//! The fidelity-ceiling calibration harness. It scores every calibrated plan
//! over a manifest of first-generation assets, derives the profile numbers by
//! a nearest-rank percentile per cell, checks the acceptance properties, and
//! produces the record `policy/calibration.json`, the owner table
//! `policy/calibration.md`, and the rewritten policy package.
//!
//! The unit of measurement is a cell: (plan, format, band). Bands stratify so
//! hard content cannot hide in an average; they never appear in the policy.
//! The calibrated plans are the safe and aggressive profiles. Each transform
//! is also scored alone, and each aggressive opt-in is scored as its own
//! stack; those rows inform the owner and set nothing. The two-opt-in stack
//! is scored only as the separation control below the aggressive tier.
//!
//! Everything reduces sequentially in manifest order on the scalar path, so
//! two runs over the same manifest produce byte-identical records.
//!
//! `examples/calibrate.rs` is the command over this module. Nothing here
//! reads the network.

use crate::asset::{self, Format, Media};
use crate::budget::{self, LsdParams};
use crate::codec::Image;
use crate::policy::PolicyPackage;
use crate::transform::pixel::{self, PixelError, PixelParams};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt::Write as _;

pub const RECORD_SCHEMA_VERSION: &str = "1.0.0";
pub const MANIFEST_SCHEMA_VERSION: &str = "1.0.0";

/// The reasoned defaults ruled before calibration, kept beside the calibrated
/// numbers in the record and the owner table.
pub const PROVISIONAL_IMAGE_SAFE: (f64, f64) = (38.0, 0.98);
pub const PROVISIONAL_IMAGE_AGGRESSIVE: (f64, f64) = (30.0, 0.90);
pub const PROVISIONAL_AUDIO_SAFE: f64 = 1.0;
pub const PROVISIONAL_AUDIO_AGGRESSIVE: f64 = 3.0;

pub const IMAGE_BANDS: [&str; 3] = ["small", "medium", "large"];
pub const AUDIO_BANDS: [&str; 3] = ["short", "medium", "long"];

// --- manifest ------------------------------------------------------------------

/// One corpus asset as the manifest describes it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManifestAsset {
    /// Relative to the manifest's directory.
    pub path: String,
    pub sha256: String,
    /// `png`, `jpeg`, `webp`, `wav`, or `flac`.
    pub format: String,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub duration_s: Option<f64>,
    #[serde(default)]
    pub rate: Option<u32>,
    #[serde(default)]
    pub channels: Option<u32>,
    #[serde(default)]
    pub generator: String,
    #[serde(default)]
    pub class: String,
    /// True when the asset was re-encoded, resized, or compressed after
    /// generation. Excluded from every cell.
    #[serde(default)]
    pub post_processed: bool,
    /// A human-made control asset: scored in its own rows, never sets a number.
    #[serde(default)]
    pub control: bool,
    /// Part of the committed fixture subset: its per-plan scores land in the
    /// record's `fixture` section.
    #[serde(default)]
    pub fixture: bool,
    /// The generation round, for manifest-driven include and exclude.
    #[serde(default)]
    pub round: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// The corpus's own identifier for the asset, when it has one.
    #[serde(default)]
    pub doc_id: String,
    /// Every other inventory column, verbatim, selectable by `field=value`.
    #[serde(default)]
    pub fields: BTreeMap<String, String>,
}

impl ManifestAsset {
    /// A selectable field by name: the typed fields first, then the extra
    /// inventory columns.
    pub fn field(&self, name: &str) -> Option<String> {
        match name {
            "path" => Some(self.path.clone()),
            "sha256" => Some(self.sha256.clone()),
            "format" => Some(self.format.clone()),
            "generator" => Some(self.generator.clone()),
            "class" => Some(self.class.clone()),
            "round" => Some(self.round.clone()),
            "doc_id" => Some(self.doc_id.clone()),
            "control" => Some(self.control.to_string()),
            "post_processed" => Some(self.post_processed.to_string()),
            other => self.fields.get(other).cloned(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: String,
    pub assets: Vec<ManifestAsset>,
}

/// Manifest-driven selection. An empty include list admits every round or
/// tag; the exclude lists always apply. `include` and `exclude` hold generic
/// `field=value` rules over any manifest field or inventory column; for a
/// field named in `include`, an asset must match one of that field's values.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Filters {
    pub include_rounds: Vec<String>,
    pub exclude_rounds: Vec<String>,
    pub include_tags: Vec<String>,
    pub exclude_tags: Vec<String>,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

fn split_rule(rule: &str) -> Result<(&str, &str), String> {
    rule.split_once('=')
        .map(|(k, v)| (k.trim(), v.trim()))
        .filter(|(k, _)| !k.is_empty())
        .ok_or_else(|| format!("selection rule {rule:?} is not field=value"))
}

impl Filters {
    /// Reject a malformed rule before any file is read.
    pub fn validate(&self) -> Result<(), String> {
        for r in self.include.iter().chain(&self.exclude) {
            split_rule(r)?;
        }
        Ok(())
    }

    /// The exclusion reason, or None when the asset is admitted.
    pub fn exclusion(&self, a: &ManifestAsset) -> Option<String> {
        if a.post_processed {
            return Some("post_processed".to_string());
        }
        // Generic rules. Include rules group by field: the asset must match
        // one value per included field.
        let mut included_fields: Vec<&str> = Vec::new();
        for r in &self.include {
            let Ok((k, _)) = split_rule(r) else { continue };
            if !included_fields.contains(&k) {
                included_fields.push(k);
            }
        }
        for k in included_fields {
            let value = a.field(k).unwrap_or_default();
            let matched = self.include.iter().any(|r| {
                split_rule(r)
                    .map(|(rk, rv)| rk == k && rv == value)
                    .unwrap_or(false)
            });
            if !matched {
                return Some(format!("{k} {value:?} not included"));
            }
        }
        for r in &self.exclude {
            let Ok((k, v)) = split_rule(r) else { continue };
            if a.field(k).as_deref() == Some(v) {
                return Some(format!("{k} {v:?} excluded"));
            }
        }
        if !self.include_rounds.is_empty() && !self.include_rounds.contains(&a.round) {
            return Some(format!("round {} not included", a.round));
        }
        if self.exclude_rounds.contains(&a.round) {
            return Some(format!("round {} excluded", a.round));
        }
        if !self.include_tags.is_empty() && !a.tags.iter().any(|t| self.include_tags.contains(t)) {
            return Some("no included tag".to_string());
        }
        if let Some(t) = a.tags.iter().find(|t| self.exclude_tags.contains(t)) {
            return Some(format!("tag {t} excluded"));
        }
        None
    }
}

pub fn parse_manifest(text: &str) -> Result<Manifest, String> {
    let m: Manifest = serde_json::from_str(text).map_err(|e| format!("manifest: {e}"))?;
    if m.schema_version != MANIFEST_SCHEMA_VERSION {
        return Err(format!(
            "manifest schema {} is not {MANIFEST_SCHEMA_VERSION}",
            m.schema_version
        ));
    }
    Ok(m)
}

/// Map an inventory's container name onto the cell format names. Unknown
/// names pass through lowercased and are excluded later as not calibrated.
pub fn normalize_format(name: &str) -> String {
    let n = name.trim().to_ascii_lowercase();
    match n.as_str() {
        "png" | "screenshot-png" => "png".to_string(),
        "jpg" | "jpeg" => "jpeg".to_string(),
        "webp" => "webp".to_string(),
        "wav" | "riff-wav" => "wav".to_string(),
        "flac" => "flac".to_string(),
        _ => n,
    }
}

/// The cell format for an inventory row: the first of `physical_variant`,
/// `format`, and the path's extension that names a calibrated container.
/// When none does, the inventory's own word stands (the variant when it has
/// one, else the format column), so the row is reported as not calibrated
/// under that name. The
/// container sniff at measurement time still has the last word.
pub fn derive_format(variant: &str, format: &str, path: &str) -> String {
    let ext = std::path::Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_default();
    for candidate in [variant, format, &ext] {
        let n = normalize_format(candidate);
        if manifest_format(&n).is_some() {
            return n;
        }
    }
    if variant.trim().is_empty() {
        normalize_format(format)
    } else {
        normalize_format(variant)
    }
}

/// A small RFC 4180 reader: quoted fields, doubled quotes, CRLF or LF.
pub fn parse_csv(text: &str) -> Result<Vec<Vec<String>>, String> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' => {
                    if chars.peek() == Some(&'"') {
                        field.push('"');
                        chars.next();
                    } else {
                        quoted = false;
                    }
                }
                _ => field.push(c),
            }
            continue;
        }
        match c {
            '"' if field.is_empty() => quoted = true,
            ',' => row.push(std::mem::take(&mut field)),
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            _ => field.push(c),
        }
    }
    if quoted {
        return Err("csv: unterminated quoted field".to_string());
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    Ok(rows)
}

/// Read an inventory CSV as a manifest. Required columns: `path`, `sha256`,
/// and `format`. The cell format is derived by `derive_format`. `round`
/// maps to the round, `generator_model` to the generator, and `archetype`
/// to the class. Every column is also kept verbatim in `fields`.
pub fn parse_manifest_csv(text: &str) -> Result<Manifest, String> {
    let rows = parse_csv(text)?;
    let mut it = rows.into_iter();
    let header = it.next().ok_or("csv: empty inventory")?;
    let col = |name: &str| header.iter().position(|h| h.trim() == name);
    let path_i = col("path").ok_or("csv: no path column")?;
    let sha_i = col("sha256").ok_or("csv: no sha256 column")?;
    let format_i = col("format").ok_or("csv: no format column")?;
    let mut assets = Vec::new();
    for (n, row) in it.enumerate() {
        if row.iter().all(|f| f.trim().is_empty()) {
            continue;
        }
        if row.len() != header.len() {
            return Err(format!(
                "csv: row {} has {} fields, header has {}",
                n + 2,
                row.len(),
                header.len()
            ));
        }
        let get = |name: &str| {
            col(name)
                .map(|i| row[i].trim().to_string())
                .unwrap_or_default()
        };
        let mut fields = BTreeMap::new();
        for (h, v) in header.iter().zip(&row) {
            fields.insert(h.trim().to_string(), v.trim().to_string());
        }
        let path = row[path_i].trim().to_string();
        let format = derive_format(&get("physical_variant"), row[format_i].trim(), &path);
        assets.push(ManifestAsset {
            path,
            sha256: row[sha_i].trim().to_ascii_lowercase(),
            format,
            width: None,
            height: None,
            duration_s: None,
            rate: None,
            channels: None,
            generator: get("generator_model"),
            class: get("archetype"),
            post_processed: matches!(get("post_processed").as_str(), "true" | "1" | "yes"),
            control: matches!(get("control").as_str(), "true" | "1" | "yes"),
            fixture: false,
            round: get("round"),
            tags: Vec::new(),
            doc_id: get("doc_id"),
            fields,
        });
    }
    Ok(Manifest {
        schema_version: MANIFEST_SCHEMA_VERSION.to_string(),
        assets,
    })
}

/// A loaded manifest with its identity: the file's sha256 and basename.
pub struct LoadedManifest {
    pub manifest: Manifest,
    pub sha256: String,
    pub name: String,
}

/// Load a JSON or CSV manifest by extension. The digest is over the file's
/// bytes as read, so it is the corpus identity whichever form was given.
pub fn load_manifest(path: &std::path::Path) -> Result<LoadedManifest, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let text = std::str::from_utf8(&bytes).map_err(|e| format!("manifest utf-8: {e}"))?;
    let is_csv = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("csv"));
    let manifest = if is_csv {
        parse_manifest_csv(text)?
    } else {
        parse_manifest(text)?
    };
    Ok(LoadedManifest {
        manifest,
        sha256: sha256_hex(&bytes),
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
    })
}

/// The selection a filter set makes over a manifest, before any measurement.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Selection {
    pub manifest: String,
    pub manifest_sha256: String,
    pub filters: Filters,
    pub listed: usize,
    pub admitted: usize,
    pub excluded: usize,
    /// Admitted counts keyed `format/round`.
    pub admitted_by_format_round: Vec<(String, usize)>,
    /// Admitted counts by format alone.
    pub admitted_by_format: Vec<(String, usize)>,
    /// Admitted assets whose format is not one the harness calibrates; they
    /// will be listed as excluded by the run.
    pub admitted_uncalibrated: Vec<(String, usize)>,
    pub exclusions: Vec<Exclusion>,
}

/// Apply the filters and, when `verify` is set, check every admitted file's
/// sha256 against the manifest, failing on the first mismatch or unreadable
/// file. Nothing is measured.
pub fn select(
    loaded: &LoadedManifest,
    root: &std::path::Path,
    filters: &Filters,
    verify: bool,
) -> Result<Selection, String> {
    filters.validate()?;
    let mut by_fr: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_f: BTreeMap<String, usize> = BTreeMap::new();
    let mut uncal: BTreeMap<String, usize> = BTreeMap::new();
    let mut exclusions = Vec::new();
    for a in &loaded.manifest.assets {
        if let Some(reason) = filters.exclusion(a) {
            exclusions.push(Exclusion {
                path: a.path.clone(),
                reason,
            });
            continue;
        }
        if verify {
            verify_sha256(root, a)?;
        }
        *by_fr
            .entry(format!("{}/{}", a.format, a.round))
            .or_default() += 1;
        *by_f.entry(a.format.clone()).or_default() += 1;
        if manifest_format(&a.format).is_none() {
            *uncal.entry(a.format.clone()).or_default() += 1;
        }
    }
    let listed = loaded.manifest.assets.len();
    Ok(Selection {
        manifest: loaded.name.clone(),
        manifest_sha256: loaded.sha256.clone(),
        filters: filters.clone(),
        listed,
        admitted: listed - exclusions.len(),
        excluded: exclusions.len(),
        admitted_by_format_round: by_fr.into_iter().collect(),
        admitted_by_format: by_f.into_iter().collect(),
        admitted_uncalibrated: uncal.into_iter().collect(),
        exclusions,
    })
}

/// Read an admitted asset and check its hash against the manifest. A
/// mismatch or an unreadable file is a run error, never a silent exclusion.
pub fn verify_sha256(root: &std::path::Path, a: &ManifestAsset) -> Result<Vec<u8>, String> {
    let full = root.join(&a.path);
    let bytes = std::fs::read(&full).map_err(|e| format!("{}: {e}", full.display()))?;
    let hash = sha256_hex(&bytes);
    if hash != a.sha256.to_ascii_lowercase() {
        return Err(format!(
            "sha256 mismatch for {}: manifest says {}, file is {hash}",
            a.path, a.sha256
        ));
    }
    Ok(bytes)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&Sha256::digest(bytes));
    out
}

fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

// --- per-asset scores ------------------------------------------------------------

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

/// Everything the harness measured on one asset.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetResult {
    pub path: String,
    pub sha256: String,
    pub format: String,
    pub band: String,
    /// `mcu-padded` for an image edge not divisible by 16, `short-clip` for
    /// audio under two seconds.
    pub strata: Vec<String>,
    pub control: bool,
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

fn manifest_format(name: &str) -> Option<Format> {
    match name {
        "png" => Some(Format::Png),
        "jpeg" | "jpg" => Some(Format::Jpeg),
        "webp" => Some(Format::WebP),
        "wav" => Some(Format::RiffWav),
        "flac" => Some(Format::Flac),
        _ => None,
    }
}

/// The cell format name for a container.
pub fn format_name(f: Format) -> &'static str {
    match f {
        Format::RiffWav => "wav",
        other => other.as_str(),
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

fn excluded(a: &ManifestAsset, reason: String) -> AssetResult {
    AssetResult {
        path: a.path.clone(),
        sha256: a.sha256.clone(),
        format: a.format.clone(),
        band: String::new(),
        strata: Vec::new(),
        control: a.control,
        fixture: a.fixture,
        excluded: Some(reason),
        plans: Vec::new(),
    }
}

/// Score every plan on one asset. `bytes` is the file as read; its hash is
/// checked against the manifest first.
pub fn evaluate_asset(bytes: &[u8], a: &ManifestAsset, ctx: &Context<'_>) -> AssetResult {
    let hash = sha256_hex(bytes);
    if hash != a.sha256.to_ascii_lowercase() {
        return excluded(a, format!("sha256 mismatch: file is {hash}"));
    }
    let Some(format) = manifest_format(&a.format) else {
        return excluded(a, format!("format {} not calibrated", a.format));
    };
    let sniffed = asset::sniff(bytes);
    if sniffed != format {
        return excluded(
            a,
            format!(
                "container is {}, manifest says {}",
                sniffed.as_str(),
                a.format
            ),
        );
    }
    match format.media() {
        Media::Image => evaluate_image(bytes, a, format, ctx),
        Media::Audio => evaluate_audio(bytes, a, format, ctx),
        _ => excluded(a, "not an image or audio container".to_string()),
    }
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
    if sd < ctx.blank_luma_stddev_min {
        return excluded(a, format!("near-blank: luma stddev {sd:.2}"));
    }
    let plans = match PlanSet::from_policy(ctx.pkg, Media::Image) {
        Ok(p) => p,
        Err(e) => return excluded(a, e),
    };
    let mut strata = Vec::new();
    if img.width % 16 != 0 || img.height % 16 != 0 {
        strata.push("mcu-padded".to_string());
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
        let out_bytes = match pixel::encode_output(&valued, format, &p) {
            Ok(b) => b,
            Err(PixelError::Held(r)) => return Score::Held { reason: r },
            Err(e) => {
                return Score::Error {
                    reason: e.to_string(),
                }
            }
        };
        let output = match crate::codec::decode_image(&out_bytes, format) {
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
    let safe = score_stack(&plans.safe);
    rows.push(PlanScore {
        plan: plans.safe_name.clone(),
        score: safe,
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

    AssetResult {
        path: a.path.clone(),
        sha256: a.sha256.clone(),
        format: format_name(format).to_string(),
        band: image_band(img.long_edge()).to_string(),
        strata,
        control: a.control,
        fixture: a.fixture,
        excluded: None,
        plans: rows,
    }
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
    if rms < ctx.silent_rms_min {
        return excluded(a, format!("near-silent: rms {rms:.5}"));
    }
    let plans = match PlanSet::from_policy(ctx.pkg, Media::Audio) {
        Ok(p) => p,
        Err(e) => return excluded(a, e),
    };
    let mut strata = Vec::new();
    if audio.duration_s() < 2.0 {
        strata.push("short-clip".to_string());
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
    AssetResult {
        path: a.path.clone(),
        sha256: a.sha256.clone(),
        format: format_name(format).to_string(),
        band: audio_band(audio.duration_s()).to_string(),
        strata,
        control: a.control,
        fixture: a.fixture,
        excluded: None,
        plans: rows,
    }
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

// --- cells and derivation ----------------------------------------------------------

/// A per-metric sample: the score and the manifest path that breaks ties.
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    pub score: f64,
    pub path: String,
}

/// Nearest-rank percentile over samples sorted ascending by (score, path).
/// Rank is ceil(p / 100 * n), one-based; no interpolation.
pub fn nearest_rank(samples: &[Sample], percentile: u32) -> Option<Sample> {
    if samples.is_empty() {
        return None;
    }
    let mut v = samples.to_vec();
    v.sort_by(|a, b| {
        a.score
            .partial_cmp(&b.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.path.cmp(&b.path))
    });
    let n = v.len() as f64;
    let rank = ((percentile as f64 / 100.0) * n).ceil().max(1.0) as usize;
    Some(v[rank.min(v.len()) - 1].clone())
}

/// Round down to a permissive step for a floor.
pub fn floor_to_step(x: f64, step: f64) -> f64 {
    round3((x / step + 1e-9).floor() * step)
}

/// Round up to a permissive step for a ceiling.
pub fn ceil_to_step(x: f64, step: f64) -> f64 {
    round3((x / step - 1e-9).ceil() * step)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageStats {
    pub psnr_percentile_db: f64,
    pub psnr_median_db: f64,
    pub psnr_min_db: f64,
    pub ssim_percentile: f64,
    pub ssim_median: f64,
    pub ssim_min: f64,
    /// The percentile asset for each metric.
    pub psnr_percentile_path: String,
    pub ssim_percentile_path: String,
    /// The cell's own rounded numbers.
    pub psnr_floor_db: f64,
    pub ssim_floor: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioStats {
    pub lsd_percentile_db: f64,
    pub lsd_median_db: f64,
    pub lsd_max_db: f64,
    pub lsd_percentile_path: String,
    pub lsd_ceiling_db: f64,
}

/// The next tier's medians on the same assets, for the separation property.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NextTier {
    pub plan: String,
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub psnr_median_db: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssim_median: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lsd_median_db: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CellRow {
    pub plan: String,
    pub format: String,
    pub band: String,
    pub merged_from: Vec<String>,
    /// Scored assets.
    pub count: usize,
    pub held: usize,
    pub errors: usize,
    /// `qualified`, `unqualified`, `held`, or `informational`.
    pub status: String,
    pub qualified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<ImageStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<AudioStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_tier: Option<NextTier>,
}

struct CellInput<'a> {
    plan: String,
    format: String,
    bands: Vec<String>,
    assets: Vec<&'a AssetResult>,
}

fn plan_score<'a>(asset: &'a AssetResult, plan: &str) -> Option<&'a Score> {
    asset
        .plans
        .iter()
        .find(|p| p.plan == plan)
        .map(|p| &p.score)
}

fn samples(
    assets: &[&AssetResult],
    plan: &str,
) -> (Vec<Sample>, Vec<Sample>, Vec<Sample>, usize, usize) {
    let (mut psnr, mut ssim, mut lsd) = (Vec::new(), Vec::new(), Vec::new());
    let (mut held, mut errors) = (0, 0);
    for a in assets {
        match plan_score(a, plan) {
            Some(Score::Image { psnr_db, ssim: s }) => {
                psnr.push(Sample {
                    score: *psnr_db,
                    path: a.path.clone(),
                });
                ssim.push(Sample {
                    score: *s,
                    path: a.path.clone(),
                });
            }
            Some(Score::Audio { lsd_db }) => lsd.push(Sample {
                score: *lsd_db,
                path: a.path.clone(),
            }),
            Some(Score::Held { .. }) => held += 1,
            Some(Score::Error { .. }) | Some(Score::Identity { .. }) | None => errors += 1,
        }
    }
    (psnr, ssim, lsd, held, errors)
}

fn median(samples: &[Sample]) -> Option<f64> {
    nearest_rank(samples, 50).map(|s| s.score)
}

/// Build one cell row. `informational` rows set nothing and never qualify.
fn build_cell(
    input: &CellInput<'_>,
    pkg: &PolicyPackage,
    informational: bool,
    next_tier_plan: Option<&str>,
) -> CellRow {
    let c = &pkg.calibration;
    let (psnr, ssim, lsd, held, errors) = samples(&input.assets, &input.plan);
    let count = psnr.len().max(lsd.len());
    let is_audio = input.plan.starts_with("audio") || input.plan.starts_with("AU");
    let image = if !is_audio && !psnr.is_empty() {
        let pp = nearest_rank(&psnr, c.percentile_image).unwrap();
        let sp = nearest_rank(&ssim, c.percentile_image).unwrap();
        Some(ImageStats {
            psnr_percentile_db: pp.score,
            psnr_median_db: median(&psnr).unwrap(),
            psnr_min_db: psnr.iter().map(|s| s.score).fold(f64::INFINITY, f64::min),
            ssim_percentile: sp.score,
            ssim_median: median(&ssim).unwrap(),
            ssim_min: ssim.iter().map(|s| s.score).fold(f64::INFINITY, f64::min),
            psnr_percentile_path: pp.path,
            ssim_percentile_path: sp.path,
            psnr_floor_db: floor_to_step(pp.score, c.psnr_step_db),
            ssim_floor: floor_to_step(sp.score, c.ssim_step),
        })
    } else {
        None
    };
    let audio = if is_audio && !lsd.is_empty() {
        let lp = nearest_rank(&lsd, c.percentile_audio).unwrap();
        Some(AudioStats {
            lsd_percentile_db: lp.score,
            lsd_median_db: median(&lsd).unwrap(),
            lsd_max_db: lsd
                .iter()
                .map(|s| s.score)
                .fold(f64::NEG_INFINITY, f64::max),
            lsd_percentile_path: lp.path,
            lsd_ceiling_db: ceil_to_step(lp.score, c.lsd_step_db),
        })
    } else {
        None
    };
    let next_tier = next_tier_plan.map(|np| {
        let (np_psnr, np_ssim, np_lsd, _, _) = samples(&input.assets, np);
        NextTier {
            plan: np.to_string(),
            count: np_psnr.len().max(np_lsd.len()),
            psnr_median_db: median(&np_psnr),
            ssim_median: median(&np_ssim),
            lsd_median_db: median(&np_lsd),
        }
    });
    let qualified = !informational && count >= c.n_min;
    let status = if informational {
        "informational"
    } else if qualified {
        "qualified"
    } else if count == 0 && held > 0 {
        "held"
    } else {
        "unqualified"
    };
    CellRow {
        plan: input.plan.clone(),
        format: input.format.clone(),
        band: input.bands.join("+"),
        merged_from: if input.bands.len() > 1 {
            input.bands.clone()
        } else {
            Vec::new()
        },
        count,
        held,
        errors,
        status: status.to_string(),
        qualified,
        image,
        audio,
        next_tier,
    }
}

/// Group a plan's assets of one format into bands, merging a short band
/// once with its adjacent band of larger count (ties go to the lower band).
/// A band that is still short after the merge is reported unqualified.
fn band_groups<'a>(
    assets: &[&'a AssetResult],
    plan: &str,
    bands: &[&str],
    n_min: usize,
) -> Vec<(Vec<String>, Vec<&'a AssetResult>)> {
    let mut groups: Vec<(Vec<String>, Vec<&AssetResult>)> = bands
        .iter()
        .map(|b| {
            (
                vec![b.to_string()],
                assets
                    .iter()
                    .copied()
                    .filter(|a| a.band == *b)
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    let scored = |g: &[&AssetResult]| -> usize {
        g.iter()
            .filter(|a| {
                matches!(
                    plan_score(a, plan),
                    Some(Score::Image { .. }) | Some(Score::Audio { .. })
                )
            })
            .count()
    };
    let mut merged = vec![false; groups.len()];
    let mut i = 0;
    while i < groups.len() {
        if merged[i] || scored(&groups[i].1) >= n_min || groups[i].1.is_empty() {
            i += 1;
            continue;
        }
        let prev = if i > 0 && !merged[i - 1] {
            Some(i - 1)
        } else {
            None
        };
        let next = if i + 1 < groups.len() && !merged[i + 1] {
            Some(i + 1)
        } else {
            None
        };
        let pick = match (prev, next) {
            (Some(p), Some(n)) => {
                if scored(&groups[n].1) > scored(&groups[p].1) {
                    Some(n)
                } else {
                    Some(p)
                }
            }
            (Some(p), None) => Some(p),
            (None, Some(n)) => Some(n),
            (None, None) => None,
        };
        let Some(j) = pick else {
            i += 1;
            continue;
        };
        let (lo, hi) = if j < i { (j, i) } else { (i, j) };
        let (hi_names, hi_assets) = groups.remove(hi);
        groups[lo].0.extend(hi_names);
        groups[lo].1.extend(hi_assets);
        merged.remove(hi);
        merged[lo] = true;
        i = lo + 1;
    }
    groups
        .into_iter()
        .filter(|(_, assets)| !assets.is_empty())
        .collect()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FormatOverride {
    pub format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub psnr_floor_db: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssim_floor: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lsd_ceiling_db: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DerivedImage {
    pub psnr_floor_db: f64,
    pub ssim_floor: f64,
    /// `calibrated` or `provisional`.
    pub source: String,
    pub provisional_psnr_floor_db: f64,
    pub provisional_ssim_floor: f64,
    pub overrides: Vec<FormatOverride>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DerivedAudio {
    pub lsd_ceiling_db: f64,
    pub source: String,
    pub provisional_lsd_ceiling_db: f64,
    pub overrides: Vec<FormatOverride>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Derived {
    #[serde(rename = "image-safe")]
    pub image_safe: DerivedImage,
    #[serde(rename = "image-aggressive")]
    pub image_aggressive: DerivedImage,
    #[serde(rename = "audio-safe")]
    pub audio_safe: DerivedAudio,
    #[serde(rename = "audio-aggressive")]
    pub audio_aggressive: DerivedAudio,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Property {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell: Option<String>,
    pub pass: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IdentityRow {
    pub plan: String,
    pub format: String,
    pub count: usize,
    pub identical: bool,
    pub failures: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Exclusion {
    pub path: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Corpus {
    pub manifest: String,
    pub manifest_sha256: String,
    pub filters: Filters,
    pub assets_listed: usize,
    pub assets_scored: usize,
    pub assets_excluded: usize,
    pub exclusions: Vec<Exclusion>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub key: String,
    pub value: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransformPin {
    pub id: String,
    pub params: Vec<Param>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Determinism {
    pub checked: bool,
    pub identical: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FixtureRow {
    pub path: String,
    pub sha256: String,
    pub plan: String,
    #[serde(flatten)]
    pub score: Score,
}

/// The calibration record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub schema_version: String,
    pub tool_version: String,
    pub policy_version: String,
    pub date: String,
    pub corpus: Corpus,
    pub encoder_fingerprint: String,
    pub base_seed: String,
    pub percentile_image: u32,
    pub percentile_audio: u32,
    pub n_min: usize,
    pub psnr_step_db: f64,
    pub ssim_step: f64,
    pub lsd_step_db: f64,
    pub metrics: Vec<Param>,
    pub transforms: Vec<TransformPin>,
    pub cells: Vec<CellRow>,
    pub strata: Vec<CellRow>,
    pub identity: Vec<IdentityRow>,
    pub derived: Derived,
    pub properties: Vec<Property>,
    pub determinism: Determinism,
    pub accepted: bool,
    pub fixture: Vec<FixtureRow>,
}

fn toml_to_json(v: &toml::Value) -> serde_json::Value {
    match v {
        toml::Value::String(s) => serde_json::Value::String(s.clone()),
        toml::Value::Integer(i) => serde_json::Value::from(*i),
        toml::Value::Float(f) => serde_json::Value::from(*f),
        toml::Value::Boolean(b) => serde_json::Value::Bool(*b),
        toml::Value::Array(a) => serde_json::Value::Array(a.iter().map(toml_to_json).collect()),
        toml::Value::Table(t) => serde_json::Value::Object(
            t.iter()
                .map(|(k, v)| (k.clone(), toml_to_json(v)))
                .collect(),
        ),
        toml::Value::Datetime(d) => serde_json::Value::String(d.to_string()),
    }
}

fn image_fails(psnr_med: Option<f64>, ssim_med: Option<f64>, floor: (f64, f64)) -> Option<bool> {
    match (psnr_med, ssim_med) {
        (Some(p), Some(s)) => Some(p < floor.0 || s < floor.1),
        _ => None,
    }
}

/// Build the record from per-asset results. `manifest_name` is the manifest
/// file's basename; the date is an input, never the clock.
pub fn build_record(
    pkg: &PolicyPackage,
    results: &[AssetResult],
    manifest_name: &str,
    manifest_sha256: &str,
    filters: &Filters,
    date: &str,
) -> Result<Record, String> {
    let c = &pkg.calibration;
    let image_plans = PlanSet::from_policy(pkg, Media::Image)?;
    let audio_plans = PlanSet::from_policy(pkg, Media::Audio)?;
    let scored: Vec<&AssetResult> = results.iter().filter(|r| r.excluded.is_none()).collect();
    let exclusions: Vec<Exclusion> = results
        .iter()
        .filter_map(|r| {
            r.excluded.as_ref().map(|reason| Exclusion {
                path: r.path.clone(),
                reason: reason.clone(),
            })
        })
        .collect();

    let mut cells = Vec::new();
    let mut strata = Vec::new();
    let mut identity = Vec::new();

    let formats_for = |media: Media| -> Vec<&'static str> {
        match media {
            Media::Image => vec!["png", "jpeg", "webp"],
            _ => vec!["wav", "flac"],
        }
    };

    for (media, plans, bands) in [
        (Media::Image, &image_plans, &IMAGE_BANDS[..]),
        (Media::Audio, &audio_plans, &AUDIO_BANDS[..]),
    ] {
        let two = plans.two_opt_in_name();
        // Plan order in the record: safe, aggressive, then the informational
        // rows.
        let mut plan_list: Vec<(String, bool, Option<String>)> = vec![
            (
                plans.safe_name.clone(),
                false,
                Some(plans.aggressive_name.clone()),
            ),
            (plans.aggressive_name.clone(), false, Some(two.clone())),
        ];
        for opt in &plans.opt_ins {
            plan_list.push((format!("{}+{opt}", plans.aggressive_name), true, None));
        }
        plan_list.push((two.clone(), true, None));
        for id in &plans.alone {
            plan_list.push((id.clone(), true, None));
        }
        for format in formats_for(media) {
            let corpus: Vec<&AssetResult> = scored
                .iter()
                .copied()
                .filter(|a| a.format == format && !a.control)
                .collect();
            let controls: Vec<&AssetResult> = scored
                .iter()
                .copied()
                .filter(|a| a.format == format && a.control)
                .collect();
            // Identity cell for the metadata profile.
            let failures: Vec<String> = corpus
                .iter()
                .chain(controls.iter())
                .filter(|a| {
                    !matches!(
                        plan_score(a, &plans.metadata_name),
                        Some(Score::Identity {
                            identical: true,
                            ..
                        })
                    )
                })
                .map(|a| a.path.clone())
                .collect();
            let count = corpus.len() + controls.len();
            if count > 0 {
                identity.push(IdentityRow {
                    plan: plans.metadata_name.clone(),
                    format: format.to_string(),
                    count,
                    identical: failures.is_empty(),
                    failures,
                });
            }
            for (plan, informational, next) in &plan_list {
                if corpus.is_empty() {
                    continue;
                }
                for (band_names, assets) in band_groups(&corpus, plan, bands, c.n_min) {
                    let input = CellInput {
                        plan: plan.clone(),
                        format: format.to_string(),
                        bands: band_names,
                        assets,
                    };
                    cells.push(build_cell(&input, pkg, *informational, next.as_deref()));
                }
                // Strata rows for the two calibrated plans only.
                if !*informational {
                    if !controls.is_empty() {
                        let input = CellInput {
                            plan: plan.clone(),
                            format: format.to_string(),
                            bands: vec!["control".to_string()],
                            assets: controls.clone(),
                        };
                        strata.push(build_cell(&input, pkg, true, next.as_deref()));
                    }
                    for stratum in ["mcu-padded", "short-clip"] {
                        let in_stratum: Vec<&AssetResult> = corpus
                            .iter()
                            .copied()
                            .filter(|a| a.strata.iter().any(|s| s == stratum))
                            .collect();
                        if !in_stratum.is_empty() {
                            let input = CellInput {
                                plan: plan.clone(),
                                format: format.to_string(),
                                bands: vec![stratum.to_string()],
                                assets: in_stratum,
                            };
                            strata.push(build_cell(&input, pkg, true, next.as_deref()));
                        }
                    }
                }
            }
        }
    }

    // Derived numbers: the most permissive value across qualified cells.
    let derive_image = |plan: &str, provisional: (f64, f64)| -> DerivedImage {
        let q: Vec<&ImageStats> = cells
            .iter()
            .filter(|r| r.plan == plan && r.qualified)
            .filter_map(|r| r.image.as_ref())
            .collect();
        if q.is_empty() {
            DerivedImage {
                psnr_floor_db: provisional.0,
                ssim_floor: provisional.1,
                source: "provisional".to_string(),
                provisional_psnr_floor_db: provisional.0,
                provisional_ssim_floor: provisional.1,
                overrides: Vec::new(),
            }
        } else {
            DerivedImage {
                psnr_floor_db: q
                    .iter()
                    .map(|s| s.psnr_floor_db)
                    .fold(f64::INFINITY, f64::min),
                ssim_floor: q.iter().map(|s| s.ssim_floor).fold(f64::INFINITY, f64::min),
                source: "calibrated".to_string(),
                provisional_psnr_floor_db: provisional.0,
                provisional_ssim_floor: provisional.1,
                overrides: Vec::new(),
            }
        }
    };
    let derive_audio = |plan: &str, provisional: f64| -> DerivedAudio {
        let q: Vec<&AudioStats> = cells
            .iter()
            .filter(|r| r.plan == plan && r.qualified)
            .filter_map(|r| r.audio.as_ref())
            .collect();
        if q.is_empty() {
            DerivedAudio {
                lsd_ceiling_db: provisional,
                source: "provisional".to_string(),
                provisional_lsd_ceiling_db: provisional,
                overrides: Vec::new(),
            }
        } else {
            DerivedAudio {
                lsd_ceiling_db: q
                    .iter()
                    .map(|s| s.lsd_ceiling_db)
                    .fold(f64::NEG_INFINITY, f64::max),
                source: "calibrated".to_string(),
                provisional_lsd_ceiling_db: provisional,
                overrides: Vec::new(),
            }
        }
    };
    let mut derived = Derived {
        image_safe: derive_image(&image_plans.safe_name, PROVISIONAL_IMAGE_SAFE),
        image_aggressive: derive_image(&image_plans.aggressive_name, PROVISIONAL_IMAGE_AGGRESSIVE),
        audio_safe: derive_audio(&audio_plans.safe_name, PROVISIONAL_AUDIO_SAFE),
        audio_aggressive: derive_audio(&audio_plans.aggressive_name, PROVISIONAL_AUDIO_AGGRESSIVE),
    };

    // Properties.
    let mut properties = Vec::new();
    let qualified_present = cells.iter().any(|r| r.qualified);
    properties.push(Property {
        name: "qualified-cells-present".to_string(),
        cell: None,
        pass: qualified_present,
        detail: if qualified_present {
            format!(
                "{} qualified cells",
                cells.iter().filter(|r| r.qualified).count()
            )
        } else {
            "no cell reached n_min; every number stays provisional".to_string()
        },
    });

    // Image coverage and separation, with the per-format override where
    // separation fails at the profile-wide number.
    for (plan, derived_plan) in [
        (&image_plans.safe_name, &mut derived.image_safe),
        (&image_plans.aggressive_name, &mut derived.image_aggressive),
    ] {
        let profile = (derived_plan.psnr_floor_db, derived_plan.ssim_floor);
        let rows: Vec<&CellRow> = cells
            .iter()
            .filter(|r| &r.plan == plan && r.qualified)
            .collect();
        let mut failing_formats: Vec<String> = Vec::new();
        for r in &rows {
            let name = format!("{}/{}/{}", r.plan, r.format, r.band);
            let s = r.image.as_ref().unwrap();
            let covers = s.psnr_percentile_db >= profile.0 && s.ssim_percentile >= profile.1;
            properties.push(Property {
                name: "coverage".to_string(),
                cell: Some(name.clone()),
                pass: covers,
                detail: format!(
                    "percentile asset scores {:.3} dB and {:.3} against {:.1} dB and {:.3}",
                    s.psnr_percentile_db, s.ssim_percentile, profile.0, profile.1
                ),
            });
            let nt = r.next_tier.as_ref().unwrap();
            match image_fails(nt.psnr_median_db, nt.ssim_median, profile) {
                Some(true) => properties.push(Property {
                    name: "separation".to_string(),
                    cell: Some(name),
                    pass: true,
                    detail: format!(
                        "{} median {:.3} dB and {:.3} fails {:.1} dB and {:.3}",
                        nt.plan,
                        nt.psnr_median_db.unwrap(),
                        nt.ssim_median.unwrap(),
                        profile.0,
                        profile.1
                    ),
                }),
                Some(false) => {
                    if !failing_formats.contains(&r.format) {
                        failing_formats.push(r.format.clone());
                    }
                }
                None => properties.push(Property {
                    name: "separation".to_string(),
                    cell: Some(name),
                    pass: false,
                    detail: format!("{} has no scored assets in this cell", nt.plan),
                }),
            }
        }
        // Q3: a per-format override where the profile-wide number fails.
        for format in failing_formats {
            let fmt_rows: Vec<&&CellRow> = rows.iter().filter(|r| r.format == format).collect();
            let fmt_number = (
                fmt_rows
                    .iter()
                    .map(|r| r.image.as_ref().unwrap().psnr_floor_db)
                    .fold(f64::INFINITY, f64::min),
                fmt_rows
                    .iter()
                    .map(|r| r.image.as_ref().unwrap().ssim_floor)
                    .fold(f64::INFINITY, f64::min),
            );
            let all_pass = fmt_rows.iter().all(|r| {
                let nt = r.next_tier.as_ref().unwrap();
                image_fails(nt.psnr_median_db, nt.ssim_median, fmt_number) == Some(true)
            });
            for r in &fmt_rows {
                let nt = r.next_tier.as_ref().unwrap();
                properties.push(Property {
                    name: "separation".to_string(),
                    cell: Some(format!("{}/{}/{}", r.plan, r.format, r.band)),
                    pass: all_pass,
                    detail: if all_pass {
                        format!(
                            "fails at the profile-wide number, so a per-format override {:.1} dB and {:.3} is emitted; {} median {:.3} dB and {:.3}",
                            fmt_number.0, fmt_number.1, nt.plan,
                            nt.psnr_median_db.unwrap_or(f64::NAN), nt.ssim_median.unwrap_or(f64::NAN)
                        )
                    } else {
                        format!(
                            "{} median {:.3} dB and {:.3} passes the profile-wide {:.1} dB and {:.3} and the per-format {:.1} dB and {:.3}; no override separates this format",
                            nt.plan,
                            nt.psnr_median_db.unwrap_or(f64::NAN), nt.ssim_median.unwrap_or(f64::NAN),
                            profile.0, profile.1, fmt_number.0, fmt_number.1
                        )
                    },
                });
            }
            if all_pass {
                derived_plan.overrides.push(FormatOverride {
                    format,
                    psnr_floor_db: Some(fmt_number.0),
                    ssim_floor: Some(fmt_number.1),
                    lsd_ceiling_db: None,
                });
            }
        }
    }

    // Audio coverage and separation.
    for (plan, derived_plan) in [
        (&audio_plans.safe_name, &mut derived.audio_safe),
        (&audio_plans.aggressive_name, &mut derived.audio_aggressive),
    ] {
        let profile = derived_plan.lsd_ceiling_db;
        let rows: Vec<&CellRow> = cells
            .iter()
            .filter(|r| &r.plan == plan && r.qualified)
            .collect();
        let mut failing_formats: Vec<String> = Vec::new();
        for r in &rows {
            let name = format!("{}/{}/{}", r.plan, r.format, r.band);
            let s = r.audio.as_ref().unwrap();
            properties.push(Property {
                name: "coverage".to_string(),
                cell: Some(name.clone()),
                pass: s.lsd_percentile_db <= profile,
                detail: format!(
                    "percentile asset scores {:.3} dB against the {:.2} dB ceiling",
                    s.lsd_percentile_db, profile
                ),
            });
            let nt = r.next_tier.as_ref().unwrap();
            match nt.lsd_median_db {
                Some(m) if m > profile => properties.push(Property {
                    name: "separation".to_string(),
                    cell: Some(name),
                    pass: true,
                    detail: format!("{} median {m:.3} dB exceeds {profile:.2} dB", nt.plan),
                }),
                Some(_) => {
                    if !failing_formats.contains(&r.format) {
                        failing_formats.push(r.format.clone());
                    }
                }
                None => properties.push(Property {
                    name: "separation".to_string(),
                    cell: Some(name),
                    pass: false,
                    detail: format!("{} has no scored assets in this cell", nt.plan),
                }),
            }
        }
        for format in failing_formats {
            let fmt_rows: Vec<&&CellRow> = rows.iter().filter(|r| r.format == format).collect();
            let fmt_number = fmt_rows
                .iter()
                .map(|r| r.audio.as_ref().unwrap().lsd_ceiling_db)
                .fold(f64::NEG_INFINITY, f64::max);
            let all_pass = fmt_rows.iter().all(|r| {
                r.next_tier
                    .as_ref()
                    .unwrap()
                    .lsd_median_db
                    .is_some_and(|m| m > fmt_number)
            });
            for r in &fmt_rows {
                let nt = r.next_tier.as_ref().unwrap();
                properties.push(Property {
                    name: "separation".to_string(),
                    cell: Some(format!("{}/{}/{}", r.plan, r.format, r.band)),
                    pass: all_pass,
                    detail: if all_pass {
                        format!(
                            "fails at the profile-wide number, so a per-format override {fmt_number:.2} dB is emitted; {} median {:.3} dB",
                            nt.plan, nt.lsd_median_db.unwrap_or(f64::NAN)
                        )
                    } else {
                        format!(
                            "{} median {:.3} dB is within the profile-wide {profile:.2} dB and the per-format {fmt_number:.2} dB; no override separates this format",
                            nt.plan, nt.lsd_median_db.unwrap_or(f64::NAN)
                        )
                    },
                });
            }
            if all_pass {
                derived_plan.overrides.push(FormatOverride {
                    format,
                    psnr_floor_db: None,
                    ssim_floor: None,
                    lsd_ceiling_db: Some(fmt_number),
                });
            }
        }
    }

    // The aggressive number is never stricter than the safe number, per
    // format where overrides exist.
    let img_effective = |d: &DerivedImage, format: &str| -> (f64, f64) {
        d.overrides
            .iter()
            .find(|o| o.format == format)
            .map(|o| (o.psnr_floor_db.unwrap(), o.ssim_floor.unwrap()))
            .unwrap_or((d.psnr_floor_db, d.ssim_floor))
    };
    let aud_effective = |d: &DerivedAudio, format: &str| -> f64 {
        d.overrides
            .iter()
            .find(|o| o.format == format)
            .and_then(|o| o.lsd_ceiling_db)
            .unwrap_or(d.lsd_ceiling_db)
    };
    let mut monotonic = true;
    let mut detail = Vec::new();
    for format in ["png", "jpeg", "webp"] {
        let s = img_effective(&derived.image_safe, format);
        let a = img_effective(&derived.image_aggressive, format);
        if a.0 > s.0 || a.1 > s.1 {
            monotonic = false;
            detail.push(format!(
                "{format}: aggressive {:.1} dB and {:.3} is stricter than safe {:.1} dB and {:.3}",
                a.0, a.1, s.0, s.1
            ));
        }
    }
    for format in ["wav", "flac"] {
        let s = aud_effective(&derived.audio_safe, format);
        let a = aud_effective(&derived.audio_aggressive, format);
        if a < s {
            monotonic = false;
            detail.push(format!(
                "{format}: aggressive {a:.2} dB is stricter than safe {s:.2} dB"
            ));
        }
    }
    properties.push(Property {
        name: "aggressive-never-stricter-than-safe".to_string(),
        cell: None,
        pass: monotonic,
        detail: if monotonic {
            "holds for every format".to_string()
        } else {
            detail.join("; ")
        },
    });

    let identity_ok = identity.iter().all(|r| r.identical);
    properties.push(Property {
        name: "metadata-byte-identity".to_string(),
        cell: None,
        pass: identity_ok,
        detail: if identity_ok {
            format!(
                "{} identity cells, every decoded stream identical",
                identity.len()
            )
        } else {
            identity
                .iter()
                .filter(|r| !r.identical)
                .map(|r| format!("{}/{}: {}", r.plan, r.format, r.failures.join(", ")))
                .collect::<Vec<_>>()
                .join("; ")
        },
    });
    let fingerprint = crate::encoder_fingerprint();
    properties.push(Property {
        name: "encoder-fingerprint".to_string(),
        cell: None,
        pass: true,
        detail: fingerprint.clone(),
    });
    // Determinism is filled by the runner after the second pass.
    properties.push(Property {
        name: "determinism".to_string(),
        cell: None,
        pass: false,
        detail: "not checked".to_string(),
    });

    let fixture: Vec<FixtureRow> = scored
        .iter()
        .filter(|a| a.fixture)
        .flat_map(|a| {
            a.plans.iter().map(|p| FixtureRow {
                path: a.path.clone(),
                sha256: a.sha256.clone(),
                plan: p.plan.clone(),
                score: p.score.clone(),
            })
        })
        .collect();

    let metrics = c
        .metrics
        .iter()
        .map(|(k, v)| Param {
            key: k.clone(),
            value: toml_to_json(v),
        })
        .collect();
    let transforms = pkg
        .transforms
        .iter()
        .filter(|t| t.milestone == 2)
        .map(|t| TransformPin {
            id: t.id.clone(),
            params: t
                .params
                .iter()
                .map(|(k, v)| Param {
                    key: k.clone(),
                    value: toml_to_json(v),
                })
                .collect(),
        })
        .collect();

    let mut record = Record {
        schema_version: RECORD_SCHEMA_VERSION.to_string(),
        tool_version: crate::TOOL_VERSION.to_string(),
        policy_version: pkg.version.clone(),
        date: date.to_string(),
        corpus: Corpus {
            manifest: manifest_name.to_string(),
            manifest_sha256: manifest_sha256.to_string(),
            filters: filters.clone(),
            assets_listed: results.len(),
            assets_scored: scored.len(),
            assets_excluded: exclusions.len(),
            exclusions,
        },
        encoder_fingerprint: fingerprint,
        base_seed: format!("0x{:016x}", c.base_seed),
        percentile_image: c.percentile_image,
        percentile_audio: c.percentile_audio,
        n_min: c.n_min,
        psnr_step_db: c.psnr_step_db,
        ssim_step: c.ssim_step,
        lsd_step_db: c.lsd_step_db,
        metrics,
        transforms,
        cells,
        strata,
        identity,
        derived,
        properties,
        determinism: Determinism {
            checked: false,
            identical: false,
        },
        accepted: false,
        fixture,
    };
    record.accepted = record.properties.iter().all(|p| p.pass);
    Ok(record)
}

/// Set the determinism result and recompute acceptance.
pub fn set_determinism(record: &mut Record, checked: bool, identical: bool) {
    record.determinism = Determinism { checked, identical };
    if let Some(p) = record
        .properties
        .iter_mut()
        .find(|p| p.name == "determinism")
    {
        p.pass = checked && identical;
        p.detail = match (checked, identical) {
            (true, true) => {
                "two passes over the manifest produced byte-identical records".to_string()
            }
            (true, false) => "the second pass produced a different record".to_string(),
            _ => "not checked; run without --single-pass to check".to_string(),
        };
    }
    record.accepted = record.properties.iter().all(|p| p.pass);
}

/// The record as committed: pretty JSON with a trailing newline. serde
/// serializes struct fields in declaration order, so the key order is fixed.
pub fn record_json(record: &Record) -> String {
    let mut s = serde_json::to_string_pretty(record).expect("record serializes");
    s.push('\n');
    s
}

/// Recompute the acceptance properties from a record's own rows and derived
/// numbers, ignoring the stored property list. The test suite uses this to
/// confirm the committed record is internally consistent.
pub fn recheck_properties(record: &Record) -> Vec<Property> {
    let mut out = Vec::new();
    let qualified: Vec<&CellRow> = record.cells.iter().filter(|r| r.qualified).collect();
    for r in &qualified {
        let name = format!("{}/{}/{}", r.plan, r.format, r.band);
        if let Some(s) = &r.image {
            let d = if r.plan == "image-safe" {
                &record.derived.image_safe
            } else {
                &record.derived.image_aggressive
            };
            let number = d
                .overrides
                .iter()
                .find(|o| o.format == r.format)
                .map(|o| (o.psnr_floor_db.unwrap(), o.ssim_floor.unwrap()))
                .unwrap_or((d.psnr_floor_db, d.ssim_floor));
            out.push(Property {
                name: "coverage".to_string(),
                cell: Some(name.clone()),
                pass: s.psnr_percentile_db >= number.0 && s.ssim_percentile >= number.1,
                detail: String::new(),
            });
            let nt = r.next_tier.as_ref();
            out.push(Property {
                name: "separation".to_string(),
                cell: Some(name.clone()),
                pass: nt
                    .and_then(|nt| image_fails(nt.psnr_median_db, nt.ssim_median, number))
                    .unwrap_or(false),
                detail: String::new(),
            });
        }
        if let Some(s) = &r.audio {
            let d = if r.plan == "audio-safe" {
                &record.derived.audio_safe
            } else {
                &record.derived.audio_aggressive
            };
            let number = d
                .overrides
                .iter()
                .find(|o| o.format == r.format)
                .and_then(|o| o.lsd_ceiling_db)
                .unwrap_or(d.lsd_ceiling_db);
            out.push(Property {
                name: "coverage".to_string(),
                cell: Some(name.clone()),
                pass: s.lsd_percentile_db <= number,
                detail: String::new(),
            });
            out.push(Property {
                name: "separation".to_string(),
                cell: Some(name),
                pass: r
                    .next_tier
                    .as_ref()
                    .and_then(|nt| nt.lsd_median_db)
                    .is_some_and(|m| m > number),
                detail: String::new(),
            });
        }
    }
    let d = &record.derived;
    out.push(Property {
        name: "aggressive-never-stricter-than-safe".to_string(),
        cell: None,
        pass: d.image_aggressive.psnr_floor_db <= d.image_safe.psnr_floor_db
            && d.image_aggressive.ssim_floor <= d.image_safe.ssim_floor
            && d.audio_aggressive.lsd_ceiling_db >= d.audio_safe.lsd_ceiling_db,
        detail: String::new(),
    });
    out.push(Property {
        name: "metadata-byte-identity".to_string(),
        cell: None,
        pass: record.identity.iter().all(|r| r.identical),
        detail: String::new(),
    });
    out.push(Property {
        name: "encoder-fingerprint".to_string(),
        cell: None,
        pass: record.encoder_fingerprint == crate::encoder_fingerprint(),
        detail: String::new(),
    });
    out.push(Property {
        name: "determinism".to_string(),
        cell: None,
        pass: record.determinism.checked && record.determinism.identical,
        detail: String::new(),
    });
    out.push(Property {
        name: "qualified-cells-present".to_string(),
        cell: None,
        pass: !qualified.is_empty(),
        detail: String::new(),
    });
    out
}

// --- owner table --------------------------------------------------------------------

fn opt3(v: Option<f64>) -> String {
    v.map(|x| format!("{x:.3}"))
        .unwrap_or_else(|| "n/a".to_string())
}

/// The owner's table, generated from the record. One row per cell, the
/// provisional number beside the calibrated one.
pub fn render_markdown(r: &Record) -> String {
    let mut w = String::new();
    let _ = writeln!(w, "# unmark calibration record");
    let _ = writeln!(w);
    let _ = writeln!(
        w,
        "Generated from `calibration.json` by the calibration harness. Edits here are overwritten."
    );
    let _ = writeln!(w);
    let _ = writeln!(w, "- date: {}", r.date);
    let _ = writeln!(w, "- policy version: {}", r.policy_version);
    let _ = writeln!(
        w,
        "- corpus manifest: {} (sha256 {})",
        r.corpus.manifest, r.corpus.manifest_sha256
    );
    let _ = writeln!(
        w,
        "- assets: {} listed, {} scored, {} excluded",
        r.corpus.assets_listed, r.corpus.assets_scored, r.corpus.assets_excluded
    );
    let _ = writeln!(w, "- encoder fingerprint: {}", r.encoder_fingerprint);
    let _ = writeln!(w, "- base seed: {}", r.base_seed);
    let _ = writeln!(
        w,
        "- derivation: image floors at the {}th percentile rounded down to {} dB and {}, audio ceiling at the {}th percentile rounded up to {} dB, n_min {}",
        r.percentile_image, r.psnr_step_db, r.ssim_step, r.percentile_audio, r.lsd_step_db, r.n_min
    );
    let _ = writeln!(
        w,
        "- accepted: {}",
        if r.accepted {
            "yes"
        } else {
            "no, a property failed or nothing qualified"
        }
    );
    let _ = writeln!(w);

    let _ = writeln!(w, "## Derived numbers");
    let _ = writeln!(w);
    let _ = writeln!(
        w,
        "| Budget | Provisional | Calibrated | Source | Overrides |"
    );
    let _ = writeln!(w, "|---|---|---|---|---|");
    let img = |name: &str, d: &DerivedImage| {
        let ov = if d.overrides.is_empty() {
            "none".to_string()
        } else {
            d.overrides
                .iter()
                .map(|o| {
                    format!(
                        "{}: {:.1} dB, {:.3}",
                        o.format,
                        o.psnr_floor_db.unwrap_or(f64::NAN),
                        o.ssim_floor.unwrap_or(f64::NAN)
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        };
        format!(
            "| {name} | PSNR {:.1} dB, SSIM {:.3} | PSNR {:.1} dB, SSIM {:.3} | {} | {ov} |",
            d.provisional_psnr_floor_db,
            d.provisional_ssim_floor,
            d.psnr_floor_db,
            d.ssim_floor,
            d.source
        )
    };
    let aud = |name: &str, d: &DerivedAudio| {
        let ov = if d.overrides.is_empty() {
            "none".to_string()
        } else {
            d.overrides
                .iter()
                .map(|o| {
                    format!(
                        "{}: {:.2} dB",
                        o.format,
                        o.lsd_ceiling_db.unwrap_or(f64::NAN)
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        };
        format!(
            "| {name} | LSD {:.2} dB | LSD {:.2} dB | {} | {ov} |",
            d.provisional_lsd_ceiling_db, d.lsd_ceiling_db, d.source
        )
    };
    let _ = writeln!(w, "{}", img("image-safe", &r.derived.image_safe));
    let _ = writeln!(
        w,
        "{}",
        img("image-aggressive", &r.derived.image_aggressive)
    );
    let _ = writeln!(w, "{}", aud("audio-safe", &r.derived.audio_safe));
    let _ = writeln!(
        w,
        "{}",
        aud("audio-aggressive", &r.derived.audio_aggressive)
    );
    let _ = writeln!(w);

    let image_rows = |w: &mut String, rows: &[CellRow]| {
        let _ = writeln!(
            w,
            "| Plan | Format | Band | n | Held | Status | P{} PSNR | P{} SSIM | Median PSNR | Median SSIM | Cell floor | Next tier median |",
            r.percentile_image, r.percentile_image
        );
        let _ = writeln!(w, "|---|---|---|---|---|---|---|---|---|---|---|---|");
        for c in rows {
            let s = c.image.as_ref();
            let nt = c
                .next_tier
                .as_ref()
                .map(|n| {
                    format!(
                        "{}: {} dB, {}",
                        n.plan,
                        opt3(n.psnr_median_db),
                        opt3(n.ssim_median)
                    )
                })
                .unwrap_or_else(|| "n/a".to_string());
            let _ = writeln!(
                w,
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                c.plan,
                c.format,
                c.band,
                c.count,
                c.held,
                c.status,
                opt3(s.map(|s| s.psnr_percentile_db)),
                opt3(s.map(|s| s.ssim_percentile)),
                opt3(s.map(|s| s.psnr_median_db)),
                opt3(s.map(|s| s.ssim_median)),
                s.map(|s| format!("{:.1} dB, {:.3}", s.psnr_floor_db, s.ssim_floor))
                    .unwrap_or_else(|| "n/a".to_string()),
                nt
            );
        }
    };
    let audio_rows = |w: &mut String, rows: &[CellRow]| {
        let _ = writeln!(
            w,
            "| Plan | Format | Band | n | Held | Status | P{} LSD | Median LSD | Max LSD | Cell ceiling | Next tier median |",
            r.percentile_audio
        );
        let _ = writeln!(w, "|---|---|---|---|---|---|---|---|---|---|---|");
        for c in rows {
            let s = c.audio.as_ref();
            let nt = c
                .next_tier
                .as_ref()
                .map(|n| format!("{}: {} dB", n.plan, opt3(n.lsd_median_db)))
                .unwrap_or_else(|| "n/a".to_string());
            let _ = writeln!(
                w,
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                c.plan,
                c.format,
                c.band,
                c.count,
                c.held,
                c.status,
                opt3(s.map(|s| s.lsd_percentile_db)),
                opt3(s.map(|s| s.lsd_median_db)),
                opt3(s.map(|s| s.lsd_max_db)),
                s.map(|s| format!("{:.2} dB", s.lsd_ceiling_db))
                    .unwrap_or_else(|| "n/a".to_string()),
                nt
            );
        }
    };
    let is_audio_row = |c: &CellRow| c.format == "wav" || c.format == "flac";

    let _ = writeln!(w, "## Image cells");
    let _ = writeln!(w);
    let rows: Vec<CellRow> = r
        .cells
        .iter()
        .filter(|c| !is_audio_row(c))
        .cloned()
        .collect();
    if rows.is_empty() {
        let _ = writeln!(w, "No image asset scored.");
    } else {
        image_rows(&mut w, &rows);
    }
    let _ = writeln!(w);
    let _ = writeln!(w, "## Audio cells");
    let _ = writeln!(w);
    let rows: Vec<CellRow> = r
        .cells
        .iter()
        .filter(|c| is_audio_row(c))
        .cloned()
        .collect();
    if rows.is_empty() {
        let _ = writeln!(w, "No audio asset scored.");
    } else {
        audio_rows(&mut w, &rows);
    }
    let _ = writeln!(w);

    let _ = writeln!(w, "## Strata");
    let _ = writeln!(w);
    let _ = writeln!(
        w,
        "Control assets, MCU-padded images, and short clips, reported beside the cells and never used to set a number."
    );
    let _ = writeln!(w);
    let rows: Vec<CellRow> = r
        .strata
        .iter()
        .filter(|c| !is_audio_row(c))
        .cloned()
        .collect();
    if !rows.is_empty() {
        image_rows(&mut w, &rows);
        let _ = writeln!(w);
    }
    let rows: Vec<CellRow> = r
        .strata
        .iter()
        .filter(|c| is_audio_row(c))
        .cloned()
        .collect();
    if !rows.is_empty() {
        audio_rows(&mut w, &rows);
        let _ = writeln!(w);
    }
    if r.strata.is_empty() {
        let _ = writeln!(w, "None.");
        let _ = writeln!(w);
    }

    let _ = writeln!(w, "## Metadata identity");
    let _ = writeln!(w);
    let _ = writeln!(w, "| Plan | Format | n | Identical | Failures |");
    let _ = writeln!(w, "|---|---|---|---|---|");
    for i in &r.identity {
        let _ = writeln!(
            w,
            "| {} | {} | {} | {} | {} |",
            i.plan,
            i.format,
            i.count,
            if i.identical { "yes" } else { "no" },
            if i.failures.is_empty() {
                "none".to_string()
            } else {
                i.failures.join(", ")
            }
        );
    }
    let _ = writeln!(w);

    let _ = writeln!(w, "## Properties");
    let _ = writeln!(w);
    let _ = writeln!(w, "| Property | Cell | Pass | Detail |");
    let _ = writeln!(w, "|---|---|---|---|");
    for p in &r.properties {
        let _ = writeln!(
            w,
            "| {} | {} | {} | {} |",
            p.name,
            p.cell.as_deref().unwrap_or("all"),
            if p.pass { "yes" } else { "no" },
            p.detail
        );
    }
    let _ = writeln!(w);

    let _ = writeln!(w, "## Exclusions");
    let _ = writeln!(w);
    if r.corpus.exclusions.is_empty() {
        let _ = writeln!(w, "None.");
    } else {
        let _ = writeln!(w, "| Path | Reason |");
        let _ = writeln!(w, "|---|---|");
        for e in &r.corpus.exclusions {
            let _ = writeln!(w, "| {} | {} |", e.path, e.reason);
        }
    }
    let _ = writeln!(w);

    let _ = writeln!(w, "## Fixture scores");
    let _ = writeln!(w);
    let _ = writeln!(
        w,
        "Per-asset scores for the committed subset, pinned by `tests/calibration.rs` at three decimals."
    );
    let _ = writeln!(w);
    let _ = writeln!(
        w,
        "| Path | Plan | Status | PSNR dB | SSIM | LSD dB | Note |"
    );
    let _ = writeln!(w, "|---|---|---|---|---|---|---|");
    for f in &r.fixture {
        let (status, psnr, ssim, lsd, note) = match &f.score {
            Score::Image { psnr_db, ssim } => (
                "scored",
                format!("{psnr_db:.3}"),
                format!("{ssim:.3}"),
                String::new(),
                String::new(),
            ),
            Score::Audio { lsd_db } => (
                "scored",
                String::new(),
                String::new(),
                format!("{lsd_db:.3}"),
                String::new(),
            ),
            Score::Identity { identical, detail } => (
                if *identical { "identical" } else { "changed" },
                String::new(),
                String::new(),
                String::new(),
                detail.clone(),
            ),
            Score::Held { reason } => (
                "held",
                String::new(),
                String::new(),
                String::new(),
                reason.clone(),
            ),
            Score::Error { reason } => (
                "error",
                String::new(),
                String::new(),
                String::new(),
                reason.clone(),
            ),
        };
        let _ = writeln!(
            w,
            "| {} | {} | {status} | {psnr} | {ssim} | {lsd} | {note} |",
            f.path, f.plan
        );
    }
    w
}

// --- policy rewrite ------------------------------------------------------------------

const OVERRIDE_COMMENT: &str =
    "# per-format override, emitted because separation failed at the profile-wide number";

/// Rewrite the policy text: the derived numbers into the `[budget.*]` tables,
/// the per-format override tables, and the record identity into
/// `[calibration]`. Comments and every other line are kept.
pub fn rewrite_policy(
    policy: &str,
    record: &Record,
    record_sha256: &str,
) -> Result<String, String> {
    // Split into sections on header lines.
    let mut sections: Vec<(String, Vec<String>)> = vec![(String::new(), Vec::new())];
    for line in policy.lines() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            sections.push((t.to_string(), vec![line.to_string()]));
        } else {
            sections.last_mut().unwrap().1.push(line.to_string());
        }
    }
    // Drop existing override sections; they are regenerated.
    sections.retain(|(h, _)| !(h.starts_with("[budget.") && h.contains(".override.")));

    let set_key = |lines: &mut Vec<String>, key: &str, value: &str| -> Result<(), String> {
        let mut found = false;
        for l in lines.iter_mut() {
            let t = l.trim_start();
            if t.starts_with(&format!("{key} =")) || t.starts_with(&format!("{key}=")) {
                let indent = &l[..l.len() - t.len()];
                *l = format!("{indent}{key} = {value}");
                found = true;
            }
        }
        if found {
            Ok(())
        } else {
            Err(format!("policy has no key {key} in the section"))
        }
    };
    let fmt_f = |v: f64| {
        let s = format!("{v}");
        if s.contains('.') {
            s
        } else {
            format!("{s}.0")
        }
    };

    let mut out: Vec<String> = Vec::new();
    for (header, lines) in sections.iter_mut() {
        match header.as_str() {
            "[budget.image-safe]" | "[budget.image-aggressive]" => {
                let d = if header == "[budget.image-safe]" {
                    &record.derived.image_safe
                } else {
                    &record.derived.image_aggressive
                };
                set_key(lines, "psnr_floor_db", &fmt_f(d.psnr_floor_db))?;
                set_key(lines, "ssim_floor", &fmt_f(d.ssim_floor))?;
                // Trim trailing blank lines so overrides sit under the table.
                while lines.last().is_some_and(|l| l.trim().is_empty()) {
                    lines.pop();
                }
                out.extend(lines.iter().cloned());
                for o in &d.overrides {
                    out.push(String::new());
                    out.push(OVERRIDE_COMMENT.to_string());
                    out.push(format!(
                        "{}.override.{}]",
                        header.trim_end_matches(']'),
                        o.format
                    ));
                    out.push(format!(
                        "psnr_floor_db = {}",
                        fmt_f(o.psnr_floor_db.unwrap())
                    ));
                    out.push(format!("ssim_floor = {}", fmt_f(o.ssim_floor.unwrap())));
                }
                out.push(String::new());
            }
            "[budget.audio-safe]" | "[budget.audio-aggressive]" => {
                let d = if header == "[budget.audio-safe]" {
                    &record.derived.audio_safe
                } else {
                    &record.derived.audio_aggressive
                };
                set_key(lines, "lsd_ceiling_db", &fmt_f(d.lsd_ceiling_db))?;
                while lines.last().is_some_and(|l| l.trim().is_empty()) {
                    lines.pop();
                }
                out.extend(lines.iter().cloned());
                for o in &d.overrides {
                    out.push(String::new());
                    out.push(OVERRIDE_COMMENT.to_string());
                    out.push(format!(
                        "{}.override.{}]",
                        header.trim_end_matches(']'),
                        o.format
                    ));
                    out.push(format!(
                        "lsd_ceiling_db = {}",
                        fmt_f(o.lsd_ceiling_db.unwrap())
                    ));
                }
                out.push(String::new());
            }
            "[calibration]" => {
                let status = if record.accepted {
                    "calibrated"
                } else {
                    "provisional"
                };
                set_key(lines, "status", &format!("\"{status}\""))?;
                set_key(lines, "record_sha256", &format!("\"{record_sha256}\""))?;
                set_key(
                    lines,
                    "corpus_manifest_sha256",
                    &format!("\"{}\"", record.corpus.manifest_sha256),
                )?;
                set_key(lines, "date", &format!("\"{}\"", record.date))?;
                out.extend(lines.iter().cloned());
            }
            _ => out.extend(lines.iter().cloned()),
        }
    }
    let mut text = out.join("\n");
    if !text.ends_with('\n') {
        text.push('\n');
    }
    // Collapse any run of blank lines the section surgery left behind.
    while text.contains("\n\n\n") {
        text = text.replace("\n\n\n", "\n\n");
    }
    Ok(text)
}

// --- the run --------------------------------------------------------------------------

/// A progress sink for the harness.
pub type Progress = Box<dyn FnMut(&str)>;

/// Inputs to one calibration run.
pub struct RunOptions {
    pub manifest_path: std::path::PathBuf,
    /// The directory asset paths are relative to. Defaults to the manifest's
    /// directory, which suits a manifest inside the corpus; an inventory kept
    /// outside the corpus names the corpus root here.
    pub root: Option<std::path::PathBuf>,
    pub date: String,
    pub filters: Filters,
    pub check_determinism: bool,
    /// Progress lines go here.
    pub progress: Option<Progress>,
}

pub struct RunOutput {
    pub record: Record,
    pub json: String,
    pub markdown: String,
    pub record_sha256: String,
}

fn one_pass(
    pkg: &PolicyPackage,
    manifest: &Manifest,
    manifest_dir: &std::path::Path,
    opts: &mut RunOptions,
) -> Result<Vec<AssetResult>, String> {
    let ctx = Context::new(pkg)?;
    let mut results = Vec::with_capacity(manifest.assets.len());
    for (i, a) in manifest.assets.iter().enumerate() {
        if let Some(p) = opts.progress.as_mut() {
            p(&format!("[{}/{}] {}", i + 1, manifest.assets.len(), a.path));
        }
        if let Some(reason) = opts.filters.exclusion(a) {
            results.push(excluded(a, reason));
            continue;
        }
        // An admitted asset that cannot be read or whose hash disagrees with
        // the manifest fails the run: the record would otherwise describe a
        // corpus other than the one the digest names.
        let bytes = verify_sha256(manifest_dir, a)?;
        results.push(evaluate_asset(&bytes, a, &ctx));
    }
    Ok(results)
}

/// Run the harness over a manifest. The second pass, when asked for, is a
/// full recomputation compared byte for byte against the first.
pub fn run(pkg: &PolicyPackage, mut opts: RunOptions) -> Result<RunOutput, String> {
    opts.filters.validate()?;
    let loaded = load_manifest(&opts.manifest_path)?;
    let manifest = loaded.manifest;
    let manifest_sha = loaded.sha256;
    let manifest_name = loaded.name;
    let manifest_dir = opts.root.clone().unwrap_or_else(|| {
        opts.manifest_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default()
    });
    let date = opts.date.clone();
    let filters = opts.filters.clone();

    let results = one_pass(pkg, &manifest, &manifest_dir, &mut opts)?;
    let mut record = build_record(
        pkg,
        &results,
        &manifest_name,
        &manifest_sha,
        &filters,
        &date,
    )?;
    if opts.check_determinism {
        if let Some(p) = opts.progress.as_mut() {
            p("second pass for the determinism property");
        }
        let again = one_pass(pkg, &manifest, &manifest_dir, &mut opts)?;
        let record2 = build_record(pkg, &again, &manifest_name, &manifest_sha, &filters, &date)?;
        let identical = record_json(&record) == record_json(&record2);
        set_determinism(&mut record, true, identical);
    } else {
        set_determinism(&mut record, false, false);
    }
    let json = record_json(&record);
    let record_sha256 = sha256_hex(json.as_bytes());
    let markdown = render_markdown(&record);
    Ok(RunOutput {
        record,
        json,
        markdown,
        record_sha256,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(score: f64, path: &str) -> Sample {
        Sample {
            score,
            path: path.to_string(),
        }
    }

    #[test]
    fn nearest_rank_takes_the_ceiling_rank_without_interpolation() {
        let v: Vec<Sample> = (1..=40).map(|i| s(i as f64, &format!("p{i:02}"))).collect();
        assert_eq!(nearest_rank(&v, 5).unwrap().score, 2.0);
        assert_eq!(nearest_rank(&v, 95).unwrap().score, 38.0);
        assert_eq!(nearest_rank(&v, 50).unwrap().score, 20.0);
        let one = vec![s(7.0, "a")];
        assert_eq!(nearest_rank(&one, 5).unwrap().score, 7.0);
    }

    #[test]
    fn ties_break_on_the_manifest_path() {
        let v = vec![s(1.0, "b"), s(1.0, "a"), s(2.0, "c")];
        assert_eq!(nearest_rank(&v, 5).unwrap().path, "a");
    }

    #[test]
    fn rounding_moves_toward_the_permissive_side() {
        assert_eq!(floor_to_step(41.37, 0.5), 41.0);
        assert_eq!(floor_to_step(41.5, 0.5), 41.5);
        assert_eq!(floor_to_step(0.9874, 0.005), 0.985);
        assert_eq!(floor_to_step(0.985, 0.005), 0.985);
        assert_eq!(ceil_to_step(0.731, 0.05), 0.75);
        assert_eq!(ceil_to_step(0.75, 0.05), 0.75);
    }

    #[test]
    fn the_csv_reader_handles_quotes_and_maps_the_inventory_columns() {
        let text = "doc_id,path,format,round,wave,generator_model,generator_lane,physical_variant,archetype,sha256\r\n\
                    d1,documents/a.png,screenshot,2,3,\"vendor/model, v2\",\"\",png,A3,ABCDEF\n\
                    d2,documents/b.mp3,audio,4,1,local,\"\",mp3,\"R4 \"\"x\"\"\",0011\n";
        let m = parse_manifest_csv(text).unwrap();
        assert_eq!(m.assets.len(), 2);
        let a = &m.assets[0];
        assert_eq!(a.format, "png");
        assert_eq!(a.round, "2");
        assert_eq!(a.generator, "vendor/model, v2");
        assert_eq!(a.class, "A3");
        assert_eq!(a.sha256, "abcdef");
        assert_eq!(a.field("wave").as_deref(), Some("3"));
        assert_eq!(a.field("format").as_deref(), Some("png"));
        let b = &m.assets[1];
        assert_eq!(b.format, "mp3");
        assert_eq!(derive_format("screenshot", "screenshot", "d/x.png"), "png");
        assert_eq!(
            derive_format("credentialed", "c2pa-asset", "d/x.webp"),
            "webp"
        );
        assert_eq!(derive_format("", "audio", "d/x.wav"), "wav");
        assert_eq!(derive_format("", "pdf-image", "d/x.pdf"), "pdf-image");
        assert_eq!(b.class, "R4 \"x\"");
        assert!(
            manifest_format(&b.format).is_none(),
            "mp3 is not calibrated"
        );
    }

    #[test]
    fn generic_rules_select_by_any_field() {
        let text = "doc_id,path,format,round,wave,physical_variant,sha256\n\
                    d1,a.png,png,1,0,,00\nd2,b.png,png,4,1,,00\nd3,c.jpg,jpeg,2,1,,00\n";
        let m = parse_manifest_csv(text).unwrap();
        let f = Filters {
            exclude: vec!["round=4".to_string()],
            include: vec![
                "format=png".to_string(),
                "format=jpeg".to_string(),
                "wave=1".to_string(),
            ],
            ..Filters::default()
        };
        let reasons: Vec<Option<String>> = m.assets.iter().map(|a| f.exclusion(a)).collect();
        assert!(reasons[0].as_deref().unwrap().contains("wave"));
        assert!(reasons[1].as_deref().unwrap().contains("round"));
        assert!(reasons[2].is_none());
        assert!(Filters {
            include: vec!["novalue".to_string()],
            ..Filters::default()
        }
        .validate()
        .is_err());
    }

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
}
