//! The prepare pass: turn an inventory into the run manifest. It verifies
//! every admitted file's hash, probes each one (container, band, variance,
//! observed C2PA, perceptual hash), applies the ladder and annotation rules,
//! collapses overlay variants onto their base, writes the lossless container
//! derivations and the synthetic clips outside the corpus, and reports the
//! per-cell counts against n_min. It measures no fidelity and writes no
//! number.

use super::evaluate::{probe, Context};
use super::manifest::{
    annotate, eligibility, identity_only_format, manifest_format, resolve_path, sha256_hex,
    split_rule, verify_sha256, Eligibility, Exclusion, Filters, LoadedManifest, Manifest,
    ManifestAsset,
};
use super::{Progress, MANIFEST_SCHEMA_VERSION};
use crate::policy::PolicyPackage;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The Hamming distance at or under which two difference hashes are one
/// base image.
pub const NEAR_DUPLICATE_BITS: u32 = 10;

pub struct PrepareOptions {
    pub root: PathBuf,
    pub filters: Filters,
    /// `field=value` rules; a matching row is post-processed and joins the
    /// ladder, with its `ladder_step` field naming the step.
    pub ladder_rules: Vec<String>,
    /// Annotation CSVs merged by doc_id, in order.
    pub annotations: Vec<PathBuf>,
    /// Where lossless container derivations are written; none to skip.
    pub derive_dir: Option<PathBuf>,
    /// Where synthetic clips are written; none to skip.
    pub synth_dir: Option<PathBuf>,
    pub progress: Option<Progress>,
}

/// One floor-setting count row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CountRow {
    pub format: String,
    pub band: String,
    pub derived: String,
    pub count: usize,
    pub n_min_met: bool,
    pub generators: usize,
    pub generator_max_share: f64,
    pub generator_cap_ok: bool,
    pub near_duplicates_collapsed: usize,
    pub marks: Vec<(String, usize)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CountsReport {
    pub source: String,
    pub source_sha256: String,
    pub n_min: usize,
    pub listed: usize,
    pub admitted: usize,
    pub excluded: usize,
    pub floor_cells: Vec<CountRow>,
    pub report_only: Vec<(String, usize)>,
    pub ladder: Vec<(String, usize)>,
    pub controls: usize,
    pub identity_only: usize,
    pub c2pa_observed: usize,
    pub claim_generators: Vec<(String, usize)>,
    pub derived_written: usize,
    pub synth_written: usize,
    pub exclusion_counts: Vec<(String, usize)>,
    pub exclusions: Vec<Exclusion>,
}

pub struct Prepared {
    pub manifest: Manifest,
    pub counts: CountsReport,
}

fn say(opts: &mut PrepareOptions, line: &str) {
    if let Some(p) = opts.progress.as_mut() {
        p(line);
    }
}

/// Run the prepare pass.
pub fn prepare(
    pkg: &PolicyPackage,
    loaded: &LoadedManifest,
    mut opts: PrepareOptions,
) -> Result<Prepared, String> {
    opts.filters.validate()?;
    for r in &opts.ladder_rules {
        split_rule(r)?;
    }
    let ctx = Context::new(pkg)?;
    let mut manifest = loaded.manifest.clone();
    let annotations = opts.annotations.clone();
    for path in &annotations {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let unmatched = annotate(&mut manifest, &text)?;
        say(
            &mut opts,
            &format!(
                "annotated from {} ({unmatched} rows without a manifest match)",
                path.display()
            ),
        );
    }

    let mut out_assets: Vec<ManifestAsset> = Vec::new();
    let mut exclusions: Vec<Exclusion> = Vec::new();
    let mut hashes: Vec<(usize, String, u64)> = Vec::new();
    let mut c2pa_observed = 0usize;
    let mut identity_only = 0usize;
    let mut claim_generators: BTreeMap<String, usize> = BTreeMap::new();
    let total = manifest.assets.len();
    for (i, a) in manifest.assets.iter().enumerate() {
        say(&mut opts, &format!("[{}/{total}] {}", i + 1, a.key()));
        if let Some(reason) = opts.filters.exclusion(a) {
            exclusions.push(Exclusion {
                doc_id: a.key(),
                reason,
            });
            continue;
        }
        let mut row = a.clone();
        for r in &opts.ladder_rules {
            let (k, v) = split_rule(r)?;
            if row.field(k).as_deref() == Some(v) {
                row.post_processed = true;
                if row.ladder_step.is_empty() {
                    row.ladder_step = row
                        .fields
                        .get("ladder_step")
                        .cloned()
                        .unwrap_or_else(|| format!("{k}={v}"));
                }
            }
        }
        if manifest_format(&row.container).is_none() {
            if let Some(f) = identity_only_format(&row.container) {
                // Rewritable but not decodable: the identity cell only.
                let bytes = verify_sha256(&opts.root, &row)?;
                let sniffed = crate::asset::sniff(&bytes);
                if sniffed != f {
                    exclusions.push(Exclusion {
                        doc_id: row.key(),
                        reason: format!(
                            "container is {}, manifest says {}",
                            sniffed.as_str(),
                            row.container
                        ),
                    });
                    continue;
                }
                row.band = Some("none".to_string());
                row.fields
                    .insert("identity_only".to_string(), "true".to_string());
                identity_only += 1;
                out_assets.push(row);
                continue;
            }
            exclusions.push(Exclusion {
                doc_id: row.key(),
                reason: format!("container {} not calibrated", row.container),
            });
            continue;
        }
        let bytes = verify_sha256(&opts.root, &row)?;
        let p = match probe(&bytes, &row, &ctx) {
            Ok(p) => p,
            Err(e) => {
                exclusions.push(Exclusion {
                    doc_id: row.key(),
                    reason: e,
                });
                continue;
            }
        };
        row.container = p.container.clone();
        row.band = Some(p.band.clone());
        row.width = p.width;
        row.height = p.height;
        row.duration_s = p.duration_s;
        row.rate = p.rate;
        row.channels = p.channels;
        if row.variance_ok.is_none() {
            row.variance_ok = Some(p.variance_ok);
        }
        if let Some(alpha) = p.alpha {
            row.fields.insert("alpha".to_string(), alpha.to_string());
        }
        if let Some(lossy) = p.webp_lossy {
            row.fields
                .insert("webp_lossy".to_string(), lossy.to_string());
            if !lossy && row.native_origin.is_none() {
                // A lossless WebP has no transcode quality to state.
                row.native_origin = Some(true);
            }
        }
        if let Some(bits) = p.bits {
            row.fields.insert("bits".to_string(), bits.to_string());
        }
        row.fields
            .insert("level".to_string(), format!("{}", p.level));
        if p.c2pa_observed {
            c2pa_observed += 1;
            if !row.documented_marks.iter().any(|m| m == "c2pa:observed") {
                row.documented_marks.push("c2pa:observed".to_string());
            }
            let cg = p
                .claim_generator
                .clone()
                .unwrap_or_else(|| "unparsed".to_string());
            *claim_generators.entry(cg.clone()).or_default() += 1;
            row.fields.insert("claim_generator".to_string(), cg);
        }
        if let Some(h) = p.dhash {
            if !row.post_processed && !row.control {
                hashes.push((out_assets.len(), row.container.clone(), h));
            }
        }
        out_assets.push(row);
    }

    // Near-duplicate collapse: the first row of a hash cluster is the base.
    let mut bases: Vec<(String, u64, String)> = Vec::new();
    for (idx, container, h) in &hashes {
        if out_assets[*idx].near_duplicate_of.is_some() {
            continue;
        }
        let hit = bases
            .iter()
            .find(|(c, bh, _)| c == container && (bh ^ h).count_ones() <= NEAR_DUPLICATE_BITS);
        match hit {
            Some((_, _, base)) => out_assets[*idx].near_duplicate_of = Some(base.clone()),
            None => bases.push((container.clone(), *h, out_assets[*idx].key())),
        }
    }

    // Lossless container derivations from floor-eligible native rows.
    let mut derived_written = 0usize;
    if let Some(dir) = opts.derive_dir.clone() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let sources: Vec<ManifestAsset> = out_assets
            .iter()
            .filter(|a| eligibility(a).0 == Eligibility::Floor && a.derived == "none")
            .cloned()
            .collect();
        for src in sources {
            let target = match src.container.as_str() {
                "png" => "webp",
                "wav" => "flac",
                _ => continue,
            };
            say(&mut opts, &format!("deriving {target} from {}", src.key()));
            let bytes = verify_sha256(&opts.root, &src)?;
            let out = derive_container(&bytes, &src.container, target)?;
            let name = format!("{}.{target}", src.key());
            let path = dir.join(&name);
            std::fs::write(&path, &out).map_err(|e| format!("{}: {e}", path.display()))?;
            let mut row = src.clone();
            row.path = path.to_string_lossy().to_string();
            row.sha256 = sha256_hex(&out);
            row.container = target.to_string();
            row.doc_id = format!("{}~{target}", src.key());
            row.derived = "container".to_string();
            row.derived_from = src.key();
            row.native_origin = Some(true);
            row.fields.remove("webp_lossy");
            if target == "webp" {
                row.fields
                    .insert("webp_lossy".to_string(), "false".to_string());
            }
            out_assets.push(row);
            derived_written += 1;
        }
    }

    // Synthetic clips from the policy base seed.
    let mut synth_written = 0usize;
    if let Some(dir) = opts.synth_dir.clone() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for (i, (kind, seconds, band)) in synth_plan().iter().enumerate() {
            say(&mut opts, &format!("synthesizing {kind} {band}"));
            let seed =
                pkg.calibration.base_seed ^ (0x9E37_79B9_7F4A_7C15u64.wrapping_mul(i as u64 + 1));
            let bytes = synth_clip(kind, *seconds, seed)?;
            let name = format!("synth-{kind}-{band}.wav");
            let path = dir.join(&name);
            std::fs::write(&path, &bytes).map_err(|e| format!("{}: {e}", path.display()))?;
            let mut fields = BTreeMap::new();
            fields.insert("bits".to_string(), "24".to_string());
            fields.insert("synth_seed".to_string(), format!("0x{seed:016x}"));
            out_assets.push(ManifestAsset {
                path: path.to_string_lossy().to_string(),
                sha256: sha256_hex(&bytes),
                container: "wav".to_string(),
                doc_id: format!("synth-{kind}-{band}"),
                width: None,
                height: None,
                duration_s: Some(*seconds),
                rate: Some(48000),
                channels: Some(2),
                generator: "unmark synth (policy base seed)".to_string(),
                content_class: "tonal-synthetic".to_string(),
                post_processed: false,
                ladder_step: String::new(),
                control: false,
                fixture: false,
                round: "synth".to_string(),
                tags: vec!["synthetic".to_string()],
                derived: "synth".to_string(),
                derived_from: String::new(),
                documented_marks: Vec::new(),
                near_duplicate_of: None,
                native_rate: Some(true),
                native_origin: Some(true),
                source_quality: None,
                variance_ok: Some(true),
                band: Some(band.to_string()),
                fields,
            });
            synth_written += 1;
        }
    }

    // Counts.
    let n_min = pkg.calibration.n_min;
    let mut floor: BTreeMap<(String, String, String), Vec<&ManifestAsset>> = BTreeMap::new();
    let mut report_only: BTreeMap<String, usize> = BTreeMap::new();
    let mut ladder: BTreeMap<String, usize> = BTreeMap::new();
    let mut controls = 0usize;
    let mut collapsed: BTreeMap<(String, String), usize> = BTreeMap::new();
    for a in &out_assets {
        let (e, reason) = eligibility(a);
        let band = a.band.clone().unwrap_or_default();
        match e {
            Eligibility::Floor | Eligibility::Container => {
                let kind = if e == Eligibility::Floor {
                    "none"
                } else {
                    "container"
                };
                floor
                    .entry((a.container.clone(), band, kind.to_string()))
                    .or_default()
                    .push(a);
            }
            Eligibility::ReportOnly if a.fields.contains_key("identity_only") => {}
            Eligibility::ReportOnly => {
                let r = reason.unwrap_or_default();
                let class = super::record::reason_class(&r);
                *report_only
                    .entry(format!("{}/{band}/{class}", a.container))
                    .or_default() += 1;
                if class == "near-duplicate" {
                    *collapsed.entry((a.container.clone(), band)).or_default() += 1;
                }
            }
            Eligibility::Ladder => {
                *ladder
                    .entry(format!("{}/{}", a.container, a.ladder_step))
                    .or_default() += 1;
            }
            Eligibility::Control => controls += 1,
        }
    }
    let mut floor_cells = Vec::new();
    for ((format, band, derived), assets) in floor {
        let mut gens: BTreeMap<&str, usize> = BTreeMap::new();
        let mut marks: BTreeMap<String, usize> = BTreeMap::new();
        for a in &assets {
            *gens.entry(a.generator.as_str()).or_default() += 1;
            *marks.entry(a.marks_key()).or_default() += 1;
        }
        let count = assets.len();
        let max_share = gens.values().copied().max().unwrap_or(0) as f64 / count.max(1) as f64;
        let max_share = (max_share * 1000.0).round() / 1000.0;
        floor_cells.push(CountRow {
            format: format.clone(),
            band: band.clone(),
            derived,
            count,
            n_min_met: count >= n_min,
            generators: gens.len(),
            generator_max_share: max_share,
            generator_cap_ok: max_share <= 0.5,
            near_duplicates_collapsed: collapsed.get(&(format, band)).copied().unwrap_or(0),
            marks: marks.into_iter().collect(),
        });
    }
    let mut exclusion_counts: BTreeMap<String, usize> = BTreeMap::new();
    for e in &exclusions {
        *exclusion_counts
            .entry(super::record::reason_class(&e.reason))
            .or_default() += 1;
    }
    let counts = CountsReport {
        source: loaded.name.clone(),
        source_sha256: loaded.sha256.clone(),
        n_min,
        listed: total,
        admitted: out_assets.len(),
        excluded: exclusions.len(),
        floor_cells,
        report_only: report_only.into_iter().collect(),
        ladder: ladder.into_iter().collect(),
        controls,
        identity_only,
        c2pa_observed,
        claim_generators: claim_generators.into_iter().collect(),
        derived_written,
        synth_written,
        exclusion_counts: exclusion_counts.into_iter().collect(),
        exclusions,
    };
    let source_inventory = loaded.manifest.source_inventory.clone().or_else(|| {
        Some(super::manifest::SourceInventory {
            name: loaded.name.clone(),
            sha256: loaded.sha256.clone(),
        })
    });
    Ok(Prepared {
        manifest: Manifest {
            schema_version: MANIFEST_SCHEMA_VERSION.to_string(),
            source_inventory,
            assets: out_assets,
        },
        counts,
    })
}

/// Re-encode a native asset losslessly into the target container.
pub fn derive_container(bytes: &[u8], from: &str, to: &str) -> Result<Vec<u8>, String> {
    match (from, to) {
        ("png", "webp") => {
            let img = crate::codec::png::decode(bytes).map_err(|e| e.to_string())?;
            let out = crate::codec::webp::encode_lossless(&img).map_err(|e| e.to_string())?;
            let back = crate::codec::webp::decode(&out).map_err(|e| e.to_string())?;
            if back != img {
                return Err("webp derivation is not lossless".to_string());
            }
            Ok(out)
        }
        #[cfg(feature = "audio")]
        ("wav", "flac") => {
            let audio = crate::codec::wav::decode(bytes).map_err(|e| e.to_string())?;
            let bits = audio.bits;
            let out = crate::codec::flac::encode(&audio, bits).map_err(|e| e.to_string())?;
            let back = crate::codec::flac::decode(&out).map_err(|e| e.to_string())?;
            if back != audio {
                return Err("flac derivation is not lossless".to_string());
            }
            Ok(out)
        }
        other => Err(format!("no derivation from {} to {}", other.0, other.1)),
    }
}

/// The synthetic clip plan: (kind, seconds, band).
pub fn synth_plan() -> Vec<(&'static str, f64, &'static str)> {
    let mut v = Vec::new();
    for (band, seconds) in [("short", 8.0), ("medium", 30.0), ("long", 75.0)] {
        for kind in ["tonal", "pink", "brown", "tonal-pink"] {
            v.push((kind, seconds, band));
        }
    }
    v
}

/// A 48 kHz stereo 24-bit WAV of the named kind, deterministic in the seed.
#[cfg(feature = "audio")]
pub fn synth_clip(kind: &str, seconds: f64, seed: u64) -> Result<Vec<u8>, String> {
    use crate::codec::Audio;
    use crate::dsp::Rng;
    let rate = 48000u32;
    let n = (rate as f64 * seconds).round() as usize;
    let mut rng = Rng::seed(seed);
    let base_hz = 110.0 * 2f64.powf(rng.uniform() * 2.0);
    let mut channels = Vec::new();
    for ch in 0..2usize {
        let detune = 1.0 + (ch as f64 - 0.5) * 0.002;
        let (mut b0, mut b1, mut b2, mut brown) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        let mut v = Vec::with_capacity(n);
        for i in 0..n {
            let t = i as f64 / rate as f64;
            let white = rng.gaussian();
            // Kellet's economy pinking filter.
            b0 = 0.99765 * b0 + white * 0.0990460;
            b1 = 0.96300 * b1 + white * 0.2965164;
            b2 = 0.57000 * b2 + white * 1.0526913;
            let pink = (b0 + b1 + b2 + white * 0.1848) * 0.08;
            brown = 0.998 * brown + white * 0.02;
            let vib = 1.0 + 0.004 * (2.0 * std::f64::consts::PI * 0.3 * t).sin();
            let mut tone = 0.0;
            for h in 1..=8 {
                let f = base_hz * h as f64 * detune * vib;
                if f < 20000.0 {
                    tone += (2.0 * std::f64::consts::PI * f * t).sin() / h as f64;
                }
            }
            tone *= 0.25;
            let s = match kind {
                "tonal" => tone,
                "pink" => pink,
                "brown" => brown * 0.8,
                "tonal-pink" => tone * 0.7 + pink * 0.5,
                other => return Err(format!("unknown synth kind {other}")),
            };
            v.push(s.clamp(-0.999, 0.999));
        }
        channels.push(v);
    }
    let audio = Audio {
        rate,
        bits: 24,
        channels,
    };
    crate::codec::wav::encode(&audio, 24).map_err(|e| e.to_string())
}

#[cfg(not(feature = "audio"))]
pub fn synth_clip(_kind: &str, _seconds: f64, _seed: u64) -> Result<Vec<u8>, String> {
    Err("audio feature not built".to_string())
}

/// Resolve a row's path for callers outside this module.
pub fn asset_path(root: &Path, a: &ManifestAsset) -> PathBuf {
    resolve_path(root, a)
}
