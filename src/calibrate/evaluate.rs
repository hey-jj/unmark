//! Per-asset evaluation: probing a file (dimensions, duration, variance,
//! perceptual hash, observed C2PA), and scoring every calibrated plan on it.

use super::manifest::{
    format_name, identity_only_format, manifest_format, sha256_bytes, sha256_hex, verdict,
    Eligibility, ManifestAsset,
};
use crate::asset::{self, Format, Media};
use crate::budget::{self, LsdParams};
use crate::codec::Image;
use crate::policy::PolicyPackage;
use crate::scan::ScanState;
use crate::transform::pixel::{self, PixelError, PixelParams};
use serde::{Deserialize, Serialize};
use sha2::Digest as _;
use std::collections::BTreeMap;

pub fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

/// A plan's score on one asset.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum Score {
    Image { psnr_db: f64, ssim: f64 },
    Audio { lsd_db: f64 },
    Identity { identical: bool, detail: String },
    Held { reason: String },
    Error { reason: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanScore {
    pub plan: String,
    #[serde(flatten)]
    pub score: Score,
}

/// Everything the harness measured on one asset. Keyed by doc_id and sha256,
/// never by path.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetResult {
    pub doc_id: String,
    pub sha256: String,
    pub container: String,
    pub band: String,
    pub derived: String,
    pub eligibility: Eligibility,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub documented_marks: Vec<String>,
    pub generator: String,
    pub content_class: String,
    /// `api-emitted`, `local-first-save`, `native-equivalent`, `lossless`,
    /// or `unknown`.
    #[serde(default)]
    pub origin_class: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cell_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub license: String,
    /// The emitted sample format for audio: `int16`, `int24`, `float32`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sample_format: String,
    /// Whether the corpus's decoded-content hash agreed with the harness's
    /// own decode, when the manifest carried one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decoded_hash_agreement: Option<bool>,
    /// `pixel-sha256` or `dhash` when the row is a near-duplicate.
    #[serde(default)]
    pub near_duplicate_rule: String,
    pub ladder_step: String,
    /// `mcu-padded` for an image edge not divisible by 16, `short-clip` for
    /// audio under two seconds.
    pub strata: Vec<String>,
    pub fixture: bool,
    pub excluded: Option<String>,
    pub plans: Vec<PlanScore>,
}

/// The plan catalog the harness scores, read from the policy package.
#[derive(Clone, Debug)]
pub struct PlanSet {
    /// Metadata profile transforms for the identity cell.
    pub metadata: Vec<String>,
    /// The safe profile's degrade transforms.
    pub safe: Vec<String>,
    /// The aggressive profile's opt-ins, each stacked on the safe plan.
    pub opt_ins: Vec<String>,
    /// Every degrade transform scored alone.
    pub alone: Vec<String>,
    pub safe_name: String,
    pub aggressive_name: String,
    pub metadata_name: String,
}

impl PlanSet {
    pub fn from_policy(pkg: &PolicyPackage, media: Media) -> Result<PlanSet, String> {
        let (meta, safe, aggr, prefix) = match media {
            Media::Image => ("image-metadata", "image-safe", "image-aggressive", "PX"),
            Media::Audio => ("audio-metadata", "audio-safe", "audio-aggressive", "AU"),
            _ => return Err("only image and audio plans are calibrated".to_string()),
        };
        let profile = |n: &str| {
            pkg.profile(n)
                .ok_or_else(|| format!("policy has no profile {n}"))
        };
        let degrade = |ids: &[String]| -> Vec<String> {
            ids.iter()
                .filter(|i| i.starts_with(prefix))
                .cloned()
                .collect()
        };
        let mut alone: Vec<String> = pkg
            .transforms
            .iter()
            .filter(|t| t.id.starts_with(prefix))
            .map(|t| t.id.clone())
            .collect();
        alone.sort();
        Ok(PlanSet {
            metadata: profile(meta)?.transforms.clone(),
            safe: degrade(&profile(safe)?.transforms),
            opt_ins: profile(aggr)?.optional_transforms.clone(),
            alone,
            safe_name: safe.to_string(),
            aggressive_name: aggr.to_string(),
            metadata_name: meta.to_string(),
        })
    }

    pub fn two_opt_in_name(&self) -> String {
        format!("{}-two-opt-in", self.safe_name.trim_end_matches("-safe"))
    }
}

/// Everything an evaluation needs besides the asset.
pub struct Context<'a> {
    pub pkg: &'a PolicyPackage,
    pub lsd: LsdParams,
    pub blank_luma_stddev_min: f64,
    pub silent_rms_min: f64,
}

impl<'a> Context<'a> {
    pub fn new(pkg: &'a PolicyPackage) -> Result<Context<'a>, String> {
        let c = &pkg.calibration;
        let need_f = |k: &str| {
            c.metric_f64(k)
                .ok_or_else(|| format!("calibration.metrics.{k}"))
        };
        let need_u = |k: &str| {
            c.metric_usize(k)
                .ok_or_else(|| format!("calibration.metrics.{k}"))
        };
        let lsd = LsdParams {
            frame: need_u("lsd_frame")?,
            hop: need_u("lsd_hop")?,
            power_floor: need_f("lsd_power_floor")?,
            min_frames: need_u("lsd_min_frames")?,
        };
        if !lsd.frame.is_power_of_two() || lsd.hop == 0 || lsd.hop > lsd.frame {
            return Err("lsd_frame must be a power of two at or above lsd_hop".to_string());
        }
        if c.metric_usize("ssim_window") != Some(budget::SSIM_WINDOW) {
            return Err(format!(
                "calibration.metrics.ssim_window must be {}",
                budget::SSIM_WINDOW
            ));
        }
        Ok(Context {
            pkg,
            lsd,
            blank_luma_stddev_min: need_f("blank_luma_stddev_min")?,
            silent_rms_min: need_f("silent_rms_min")?,
        })
    }
}

/// The cell format for a container: the container name, with `-alpha` when
/// the decoded image carries alpha.
pub fn cell_format(container: &str, alpha: bool) -> String {
    if alpha {
        format!("{container}-alpha")
    } else {
        container.to_string()
    }
}

pub fn image_band(long_edge: usize) -> &'static str {
    if long_edge <= 640 {
        "small"
    } else if long_edge <= 1280 {
        "medium"
    } else {
        "large"
    }
}

pub fn audio_band(duration_s: f64) -> &'static str {
    if duration_s <= 10.0 {
        "short"
    } else if duration_s <= 60.0 {
        "medium"
    } else {
        "long"
    }
}

// --- probing --------------------------------------------------------------------------

/// What a read of the file establishes before any plan runs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Probe {
    pub container: String,
    pub band: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpha: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webp_lossy: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_s: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bits: Option<u16>,
    /// Luma standard deviation for an image, RMS for audio.
    pub level: f64,
    pub variance_ok: bool,
    /// A 64-bit difference hash of the luma, for near-duplicate grouping.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dhash: Option<u64>,
    pub c2pa_observed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim_generator: Option<String>,
    pub strata: Vec<String>,
    /// sha256 over the decoded content as pinned (RGBA8 row-major, or PCM
    /// interleaved little-endian in the emitted sample format), the key for
    /// the exact dedupe and the corpus cross-check.
    pub pixel_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_format: Option<String>,
}

/// The decoded-content hash of an image as pinned: RGBA8 row-major, alpha
/// 255 when absent.
pub fn image_pixel_sha256(img: &Image) -> String {
    let rgba = img.with_alpha();
    let mut h = sha2::Sha256::new();
    sha2::Digest::update(&mut h, &rgba.data);
    format!("{:x}", sha2::Digest::finalize(h))
}

/// The luma difference hash: the image box-averaged to 9 by 8, each bit
/// one when the left cell is brighter than its right neighbour.
pub fn dhash(img: &Image) -> u64 {
    let luma = img.luma();
    let (w, h) = (img.width, img.height);
    let cell = |gx: usize, gy: usize| -> f64 {
        let x0 = gx * w / 9;
        let x1 = ((gx + 1) * w / 9).max(x0 + 1).min(w);
        let y0 = gy * h / 8;
        let y1 = ((gy + 1) * h / 8).max(y0 + 1).min(h);
        let mut sum = 0.0;
        let mut n = 0.0;
        for y in y0..y1 {
            for x in x0..x1 {
                sum += luma[y * w + x];
                n += 1.0;
            }
        }
        sum / n
    };
    let mut bits = 0u64;
    for gy in 0..8 {
        for gx in 0..8 {
            if cell(gx, gy) > cell(gx + 1, gy) {
                bits |= 1 << (gy * 8 + gx);
            }
        }
    }
    bits
}

/// Best-effort read of the claim generator out of a located C2PA manifest:
/// the CBOR text after a `claim_generator` key, a JSON string after the same
/// key, or the `name` inside `claim_generator_info`. Data only, never a
/// command.
pub fn claim_generator(region: &[u8]) -> Option<String> {
    fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
        if hay.len() < needle.len() {
            return None;
        }
        (from..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
    }
    fn text_after(region: &[u8], at: usize) -> Option<String> {
        let b = *region.get(at)?;
        let (len, start) = match b {
            0x60..=0x77 => ((b & 0x1f) as usize, at + 1),
            0x78 => (*region.get(at + 1)? as usize, at + 2),
            0x79 => (
                u16::from_be_bytes([*region.get(at + 1)?, *region.get(at + 2)?]) as usize,
                at + 3,
            ),
            b'"' | b':' | b' ' => {
                // JSON form: past the key's closing quote, the colon, and
                // any space, to the opening quote of the value.
                let mut i = at;
                if region.get(i) == Some(&b'"') {
                    i += 1;
                }
                while i < region.len() && (region[i] == b':' || region[i] == b' ') {
                    i += 1;
                }
                if region.get(i) != Some(&b'"') {
                    return None;
                }
                let s = i + 1;
                let e = find(region, b"\"", s)?;
                (e - s, s)
            }
            _ => return None,
        };
        let slice = region.get(start..start + len)?;
        let text = std::str::from_utf8(slice).ok()?;
        if text.is_empty() || !text.chars().all(|c| c.is_ascii_graphic() || c == ' ') {
            return None;
        }
        Some(text.to_string())
    }
    if let Some(at) = find(region, b"claim_generator_info", 0) {
        if let Some(n) = find(region, b"name", at) {
            if let Some(t) = text_after(region, n + 4) {
                return Some(t);
            }
        }
    }
    let mut from = 0;
    while let Some(at) = find(region, b"claim_generator", from) {
        let after = at + b"claim_generator".len();
        if region.get(after) != Some(&b'_') {
            if let Some(t) = text_after(region, after) {
                return Some(t);
            }
        }
        from = after;
    }
    None
}

/// Probe a file: verify its container against the manifest, decode it, and
/// read what the record needs. The bytes are the file as read.
pub fn probe(bytes: &[u8], a: &ManifestAsset, ctx: &Context<'_>) -> Result<Probe, String> {
    let format = manifest_format(&a.container)
        .ok_or_else(|| format!("container {} not calibrated", a.container))?;
    let sniffed = asset::sniff(bytes);
    if sniffed != format {
        return Err(format!(
            "container is {}, manifest says {}",
            sniffed.as_str(),
            a.container
        ));
    }
    let det = crate::detect::inspect(bytes);
    let mut c2pa_observed = false;
    let mut claim = None;
    if let Some(c) = det.get("c2pa") {
        if c.state == ScanState::ConfirmedPresent {
            c2pa_observed = true;
            for loc in &c.locations {
                if let Some(region) = bytes.get(loc.offset..loc.offset + loc.length) {
                    claim = claim_generator(region);
                    if claim.is_some() {
                        break;
                    }
                }
            }
        }
    }
    match format.media() {
        Media::Image => {
            let img = crate::codec::decode_image(bytes, format)
                .map_err(|e| format!("decode failed: {e}"))?;
            let luma = img.luma();
            let n = luma.len() as f64;
            let mean = luma.iter().sum::<f64>() / n;
            let sd = (luma.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / n).sqrt();
            let mut strata = Vec::new();
            if img.width % 16 != 0 || img.height % 16 != 0 {
                strata.push("mcu-padded".to_string());
            }
            let webp_lossy = if format == Format::WebP {
                Some(crate::codec::webp::is_lossy(bytes).map_err(|e| e.to_string())?)
            } else {
                None
            };
            Ok(Probe {
                container: format_name(format).to_string(),
                band: image_band(img.long_edge()).to_string(),
                width: Some(img.width as u32),
                height: Some(img.height as u32),
                alpha: Some(img.has_alpha()),
                webp_lossy,
                duration_s: None,
                rate: None,
                channels: None,
                bits: None,
                level: round3(sd),
                variance_ok: sd >= ctx.blank_luma_stddev_min,
                dhash: Some(dhash(&img)),
                c2pa_observed,
                claim_generator: claim,
                strata,
                pixel_sha256: image_pixel_sha256(&img),
                sample_format: None,
            })
        }
        Media::Audio => probe_audio(bytes, format, ctx, c2pa_observed, claim),
        _ => Err("not an image or audio container".to_string()),
    }
}

#[cfg(feature = "audio")]
fn probe_audio(
    bytes: &[u8],
    format: Format,
    ctx: &Context<'_>,
    c2pa_observed: bool,
    claim: Option<String>,
) -> Result<Probe, String> {
    let audio =
        crate::codec::decode_audio(bytes, format).map_err(|e| format!("decode failed: {e}"))?;
    let mono = audio.downmix();
    if mono.is_empty() {
        return Err("empty clip".to_string());
    }
    let rms = (mono.iter().map(|v| v * v).sum::<f64>() / mono.len() as f64).sqrt();
    let mut strata = Vec::new();
    if audio.duration_s() < 2.0 {
        strata.push("short-clip".to_string());
    }
    Ok(Probe {
        container: format_name(format).to_string(),
        band: audio_band(audio.duration_s()).to_string(),
        width: None,
        height: None,
        alpha: None,
        webp_lossy: None,
        duration_s: Some(round3(audio.duration_s())),
        rate: Some(audio.rate),
        channels: Some(audio.channels.len() as u32),
        bits: Some(audio.bits),
        level: (rms * 1e6).round() / 1e6,
        variance_ok: rms >= ctx.silent_rms_min,
        dhash: None,
        c2pa_observed,
        claim_generator: claim,
        strata,
        pixel_sha256: {
            let mut h = sha2::Sha256::new();
            h.update(audio.interleaved_le_bytes());
            format!("{:x}", h.finalize())
        },
        sample_format: Some(audio.sample_format()),
    })
}

#[cfg(not(feature = "audio"))]
fn probe_audio(
    _bytes: &[u8],
    _format: Format,
    _ctx: &Context<'_>,
    _c2pa_observed: bool,
    _claim: Option<String>,
) -> Result<Probe, String> {
    Err("audio feature not built".to_string())
}

// --- scoring --------------------------------------------------------------------------

fn base_result(a: &ManifestAsset) -> AssetResult {
    let v = verdict(a);
    let (e, reason) = (v.eligibility, v.reason);
    AssetResult {
        doc_id: a.key(),
        sha256: a.sha256.clone(),
        container: a.container.clone(),
        band: a.band.clone().unwrap_or_default(),
        derived: a.derived.clone(),
        eligibility: e,
        reason,
        documented_marks: a.documented_marks.clone(),
        generator: a.generator.clone(),
        content_class: a.content_class.clone(),
        origin_class: v.origin_class,
        cell_id: a.cell_id.clone(),
        license: a.license.clone(),
        sample_format: a.fields.get("sample_format").cloned().unwrap_or_default(),
        decoded_hash_agreement: a.fields.get("decoded_hash_agreement").map(|s| s == "true"),
        near_duplicate_rule: a
            .fields
            .get("near_duplicate_rule")
            .cloned()
            .unwrap_or_default(),
        ladder_step: a.ladder_step.clone(),
        strata: Vec::new(),
        fixture: a.fixture,
        excluded: None,
        plans: Vec::new(),
    }
}

fn excluded(a: &ManifestAsset, reason: String) -> AssetResult {
    let mut r = base_result(a);
    r.excluded = Some(reason);
    r
}

/// Score every plan on one asset. `bytes` is the file as read; its hash is
/// checked against the manifest first.
pub fn evaluate_asset(bytes: &[u8], a: &ManifestAsset, ctx: &Context<'_>) -> AssetResult {
    let hash = sha256_hex(bytes);
    if hash != a.sha256.to_ascii_lowercase() {
        return excluded(a, format!("sha256 mismatch: file is {hash}"));
    }
    let Some(format) = manifest_format(&a.container) else {
        if let Some(f) = identity_only_format(&a.container) {
            return evaluate_identity_only(bytes, a, f, ctx);
        }
        return excluded(a, format!("container {} not calibrated", a.container));
    };
    let sniffed = asset::sniff(bytes);
    if sniffed != format {
        return excluded(
            a,
            format!(
                "container is {}, manifest says {}",
                sniffed.as_str(),
                a.container
            ),
        );
    }
    match format.media() {
        Media::Image => evaluate_image(bytes, a, format, ctx),
        Media::Audio => evaluate_audio(bytes, a, format, ctx),
        _ => excluded(a, "not an image or audio container".to_string()),
    }
}

/// A container the harness only rewrites: the metadata identity cell on the
/// container's signal stream, no fidelity plan. A declined strip writes
/// nothing, so identity holds and the decline is recorded in the detail.
fn evaluate_identity_only(
    bytes: &[u8],
    a: &ManifestAsset,
    format: Format,
    ctx: &Context<'_>,
) -> AssetResult {
    let sniffed = asset::sniff(bytes);
    if sniffed != format {
        return excluded(
            a,
            format!(
                "container is {}, manifest says {}",
                sniffed.as_str(),
                a.container
            ),
        );
    }
    let plans = match PlanSet::from_policy(ctx.pkg, Media::Audio) {
        Ok(p) => p,
        Err(e) => return excluded(a, e),
    };
    let mut result = base_result(a);
    result.band = "none".to_string();
    result.eligibility = Eligibility::ReportOnly;
    result.reason = Some("identity-only container".to_string());
    let before = crate::container::signal_stream(bytes, format);
    let score = match crate::transform::apply(bytes, format, &plans.metadata) {
        Ok(applied) => {
            let after = crate::container::signal_stream(&applied.bytes, format);
            let identical = crate::budget::check_byte_identity(&before, &after).is_ok();
            Score::Identity {
                identical,
                detail: if identical {
                    "signal stream identical".to_string()
                } else {
                    "signal stream changed under a metadata profile".to_string()
                },
            }
        }
        Err(crate::container::RewriteError::Declined { class, reason }) => Score::Identity {
            identical: true,
            detail: format!("declined stripping {class}: {reason}; nothing written"),
        },
        Err(e) => Score::Identity {
            identical: false,
            detail: format!("rewrite failed: {e}"),
        },
    };
    result.plans.push(PlanScore {
        plan: plans.metadata_name.clone(),
        score,
    });
    result
}

/// The metadata-profile identity cell: rewrite, decode, compare streams.
fn identity_score<T: PartialEq>(
    bytes: &[u8],
    format: Format,
    ids: &[String],
    decode: impl Fn(&[u8]) -> Result<T, String>,
    input: &T,
) -> Score {
    match crate::transform::apply(bytes, format, ids) {
        Ok(applied) => match decode(&applied.bytes) {
            Ok(out) => {
                let identical = &out == input;
                Score::Identity {
                    identical,
                    detail: if identical {
                        "decoded stream identical".to_string()
                    } else {
                        "decoded stream changed under a metadata profile".to_string()
                    },
                }
            }
            Err(e) => Score::Identity {
                identical: false,
                detail: format!("output would not decode: {e}"),
            },
        },
        Err(e) => Score::Identity {
            identical: false,
            detail: format!("rewrite failed: {e}"),
        },
    }
}

fn evaluate_image(
    bytes: &[u8],
    a: &ManifestAsset,
    format: Format,
    ctx: &Context<'_>,
) -> AssetResult {
    let img = match crate::codec::decode_image(bytes, format) {
        Ok(i) => i,
        Err(e) => return excluded(a, format!("decode failed: {e}")),
    };
    let luma = img.luma();
    let n = luma.len() as f64;
    let mean = luma.iter().sum::<f64>() / n;
    let sd = (luma.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / n).sqrt();
    let plans = match PlanSet::from_policy(ctx.pkg, Media::Image) {
        Ok(p) => p,
        Err(e) => return excluded(a, e),
    };
    let mut result = base_result(a);
    result.band = image_band(img.long_edge()).to_string();
    // An alpha image is its own format cell, since its safe plan differs.
    if img.has_alpha() {
        result.container = cell_format(&a.container, true);
    }
    if img.width % 16 != 0 || img.height % 16 != 0 {
        result.strata.push("mcu-padded".to_string());
    }
    if sd < ctx.blank_luma_stddev_min && result.eligibility.sets_numbers() {
        result.eligibility = Eligibility::ReportOnly;
        result.reason = Some(format!("near-blank: luma stddev {sd:.2}"));
    }
    let seed = crate::dsp::asset_seed(ctx.pkg.calibration.base_seed, &sha256_bytes(bytes));
    let mut rows = Vec::new();

    rows.push(PlanScore {
        plan: plans.metadata_name.clone(),
        score: identity_score(
            bytes,
            format,
            &plans.metadata,
            |b| crate::codec::decode_image(b, format).map_err(|e| e.to_string()),
            &img,
        ),
    });

    // A geometry cache: stacks that share flip, crop, and ratio share the
    // reference, and the reference is the expensive stage.
    let mut geometry: BTreeMap<String, Result<Image, PixelError>> = BTreeMap::new();
    let mut score_stack = |ids: &[String]| -> Score {
        let p = match PixelParams::from_policy(ctx.pkg, ids) {
            Ok(p) => p,
            Err(e) => return Score::Error { reason: e },
        };
        let key = format!("{:?}|{:?}|{:?}", p.flip, p.crop_area, p.resample_ratio);
        let reference = geometry
            .entry(key)
            .or_insert_with(|| pixel::geometry(&img, &p));
        let reference = match reference {
            Ok(r) => r.clone(),
            Err(e) => {
                return Score::Error {
                    reason: e.to_string(),
                }
            }
        };
        let valued = pixel::signal(&reference, &p, seed);
        let emitted = match pixel::encode_output(&valued, format, &p) {
            Ok(b) => b,
            Err(PixelError::Held(r)) => return Score::Held { reason: r },
            Err(e) => {
                return Score::Error {
                    reason: e.to_string(),
                }
            }
        };
        let output = match crate::codec::decode_image(&emitted.bytes, emitted.format) {
            Ok(o) => o,
            Err(e) => {
                return Score::Error {
                    reason: format!("output decode: {e}"),
                }
            }
        };
        image_score(&reference, &output)
    };

    for id in &plans.alone {
        rows.push(PlanScore {
            plan: id.clone(),
            score: score_stack(std::slice::from_ref(id)),
        });
    }
    rows.push(PlanScore {
        plan: plans.safe_name.clone(),
        score: score_stack(&plans.safe),
    });
    let mut singles = Vec::new();
    for opt in &plans.opt_ins {
        let mut ids = plans.safe.clone();
        ids.push(opt.clone());
        let s = score_stack(&ids);
        rows.push(PlanScore {
            plan: format!("{}+{opt}", plans.aggressive_name),
            score: s.clone(),
        });
        singles.push(s);
    }
    rows.push(PlanScore {
        plan: plans.aggressive_name.clone(),
        score: worst(&singles),
    });
    let mut pairs = Vec::new();
    for (i, a1) in plans.opt_ins.iter().enumerate() {
        for a2 in &plans.opt_ins[i + 1..] {
            let mut ids = plans.safe.clone();
            ids.push(a1.clone());
            ids.push(a2.clone());
            pairs.push(score_stack(&ids));
        }
    }
    rows.push(PlanScore {
        plan: plans.two_opt_in_name(),
        score: worst(&pairs),
    });
    result.plans = rows;
    result
}

/// PSNR over RGB or RGBA and windowed SSIM on luma, both against the
/// grid-matched reference. A channel mismatch expands both to RGBA.
pub fn image_score(reference: &Image, output: &Image) -> Score {
    if reference.width != output.width || reference.height != output.height {
        return Score::Error {
            reason: format!(
                "output grid {}x{} is not the reference grid {}x{}",
                output.width, output.height, reference.width, reference.height
            ),
        };
    }
    let (r, o) = if reference.channels != output.channels {
        (reference.with_alpha(), output.with_alpha())
    } else {
        (reference.clone(), output.clone())
    };
    let psnr = budget::psnr(&r.data, &o.data);
    let ssim = budget::ssim(&r.data, &o.data, r.width, r.height, r.channels);
    match (psnr, ssim) {
        (Some(p), Some(s)) => Score::Image {
            psnr_db: round3(p),
            ssim: round3(s),
        },
        _ => Score::Error {
            reason: "unscorable: the image is smaller than the SSIM window".to_string(),
        },
    }
}

/// The worst score across stacks: the lowest PSNR and the lowest SSIM taken
/// independently, or the highest LSD. Held and error stacks are skipped;
/// when nothing scored, the first held or error reason is reported.
pub fn worst(scores: &[Score]) -> Score {
    let mut psnr: Option<f64> = None;
    let mut ssim: Option<f64> = None;
    let mut lsd: Option<f64> = None;
    for s in scores {
        match s {
            Score::Image { psnr_db, ssim: ss } => {
                psnr = Some(psnr.map_or(*psnr_db, |p| p.min(*psnr_db)));
                ssim = Some(ssim.map_or(*ss, |p| p.min(*ss)));
            }
            Score::Audio { lsd_db } => lsd = Some(lsd.map_or(*lsd_db, |p| p.max(*lsd_db))),
            _ => {}
        }
    }
    if let (Some(p), Some(s)) = (psnr, ssim) {
        return Score::Image {
            psnr_db: p,
            ssim: s,
        };
    }
    if let Some(l) = lsd {
        return Score::Audio { lsd_db: l };
    }
    scores
        .iter()
        .find(|s| matches!(s, Score::Held { .. }))
        .or_else(|| scores.first())
        .cloned()
        .unwrap_or(Score::Error {
            reason: "no stack to score".to_string(),
        })
}

#[cfg(feature = "audio")]
fn evaluate_audio(
    bytes: &[u8],
    a: &ManifestAsset,
    format: Format,
    ctx: &Context<'_>,
) -> AssetResult {
    use crate::transform::audio::{self as au, AudioError, AudioParams};
    let audio = match crate::codec::decode_audio(bytes, format) {
        Ok(x) => x,
        Err(e) => return excluded(a, format!("decode failed: {e}")),
    };
    let mono = audio.downmix();
    if mono.is_empty() {
        return excluded(a, "empty clip".to_string());
    }
    let rms = (mono.iter().map(|v| v * v).sum::<f64>() / mono.len() as f64).sqrt();
    let plans = match PlanSet::from_policy(ctx.pkg, Media::Audio) {
        Ok(p) => p,
        Err(e) => return excluded(a, e),
    };
    let mut result = base_result(a);
    result.band = audio_band(audio.duration_s()).to_string();
    if audio.duration_s() < 2.0 {
        result.strata.push("short-clip".to_string());
    }
    if rms < ctx.silent_rms_min && result.eligibility.sets_numbers() {
        result.eligibility = Eligibility::ReportOnly;
        result.reason = Some(format!("near-silent: rms {rms:.5}"));
    }
    let seed = crate::dsp::asset_seed(ctx.pkg.calibration.base_seed, &sha256_bytes(bytes));
    let mut rows = Vec::new();
    rows.push(PlanScore {
        plan: plans.metadata_name.clone(),
        score: identity_score(
            bytes,
            format,
            &plans.metadata,
            |b| crate::codec::decode_audio(b, format).map_err(|e| e.to_string()),
            &audio,
        ),
    });
    let score_stack = |ids: &[String]| -> Score {
        let p = match AudioParams::from_policy(ctx.pkg, ids) {
            Ok(p) => p,
            Err(AudioError::Held(r)) => return Score::Held { reason: r },
            Err(e) => {
                return Score::Error {
                    reason: e.to_string(),
                }
            }
        };
        let processed = au::apply(&audio, &p, seed);
        let bits = p.output_bits(audio.bits);
        let out_bytes = match crate::codec::encode_audio(&processed, format, bits) {
            Ok(b) => b,
            Err(e) => {
                return Score::Error {
                    reason: e.to_string(),
                }
            }
        };
        let output = match crate::codec::decode_audio(&out_bytes, format) {
            Ok(o) => o,
            Err(e) => {
                return Score::Error {
                    reason: format!("output decode: {e}"),
                }
            }
        };
        let out_mono = output.downmix();
        let d = if p.stretch.is_some() {
            budget::averaged_spectrum_distance(&mono, &out_mono, &ctx.lsd)
        } else {
            budget::lsd(&mono, &out_mono, &ctx.lsd)
        };
        match d {
            Some(v) => Score::Audio { lsd_db: round3(v) },
            None => Score::Error {
                reason: format!("unscorable: fewer than {} frames", ctx.lsd.min_frames),
            },
        }
    };
    for id in &plans.alone {
        rows.push(PlanScore {
            plan: id.clone(),
            score: score_stack(std::slice::from_ref(id)),
        });
    }
    rows.push(PlanScore {
        plan: plans.safe_name.clone(),
        score: score_stack(&plans.safe),
    });
    let mut singles = Vec::new();
    for opt in &plans.opt_ins {
        let mut ids = plans.safe.clone();
        ids.push(opt.clone());
        let s = score_stack(&ids);
        rows.push(PlanScore {
            plan: format!("{}+{opt}", plans.aggressive_name),
            score: s.clone(),
        });
        singles.push(s);
    }
    rows.push(PlanScore {
        plan: plans.aggressive_name.clone(),
        score: worst(&singles),
    });
    let mut pairs = Vec::new();
    for (i, a1) in plans.opt_ins.iter().enumerate() {
        for a2 in &plans.opt_ins[i + 1..] {
            let mut ids = plans.safe.clone();
            ids.push(a1.clone());
            ids.push(a2.clone());
            pairs.push(score_stack(&ids));
        }
    }
    rows.push(PlanScore {
        plan: plans.two_opt_in_name(),
        score: worst(&pairs),
    });
    result.plans = rows;
    result
}

#[cfg(not(feature = "audio"))]
fn evaluate_audio(
    _bytes: &[u8],
    a: &ManifestAsset,
    _format: Format,
    _ctx: &Context<'_>,
) -> AssetResult {
    excluded(a, "audio feature not built".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worst_takes_each_metric_independently() {
        let w = worst(&[
            Score::Image {
                psnr_db: 40.0,
                ssim: 0.90,
            },
            Score::Image {
                psnr_db: 35.0,
                ssim: 0.99,
            },
            Score::Held {
                reason: "x".to_string(),
            },
        ]);
        assert_eq!(
            w,
            Score::Image {
                psnr_db: 35.0,
                ssim: 0.90
            }
        );
        let h = worst(&[Score::Held {
            reason: "x".to_string(),
        }]);
        assert!(matches!(h, Score::Held { .. }));
    }

    #[test]
    fn the_claim_generator_reads_cbor_and_json_forms() {
        let mut cbor = b"xxclaim_generator".to_vec();
        cbor.push(0x6b);
        cbor.extend_from_slice(b"Tool/1.2 ab");
        assert_eq!(claim_generator(&cbor).as_deref(), Some("Tool/1.2 ab"));
        let json = br#"{"claim_generator": "Other Tool 3", "x": 1}"#;
        assert_eq!(claim_generator(json).as_deref(), Some("Other Tool 3"));
        let mut info = b"..claim_generator_info..".to_vec();
        info.extend_from_slice(b"name");
        info.push(0x65);
        info.extend_from_slice(b"Named");
        assert_eq!(claim_generator(&info).as_deref(), Some("Named"));
        assert_eq!(claim_generator(b"nothing here"), None);
    }

    #[test]
    fn dhash_separates_distinct_patterns_and_matches_itself() {
        let a = Image {
            width: 32,
            height: 16,
            channels: 3,
            data: (0..32 * 16 * 3).map(|i| ((i / 3) % 32 * 8) as u8).collect(),
        };
        let b = Image {
            width: 32,
            height: 16,
            channels: 3,
            data: (0..32 * 16 * 3)
                .map(|i| (255 - (i / 3) % 32 * 8) as u8)
                .collect(),
        };
        assert_eq!(dhash(&a), dhash(&a));
        assert_ne!(dhash(&a), dhash(&b));
        assert_eq!((dhash(&a) ^ dhash(&b)).count_ones(), 64);
    }
}
