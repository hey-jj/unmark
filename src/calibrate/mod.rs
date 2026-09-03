//! The fidelity-ceiling calibration harness. It scores every calibrated plan
//! over a manifest of first-generation assets, derives the profile numbers by
//! a nearest-rank percentile per cell, checks the acceptance properties, and
//! produces the record `policy/calibration.json`, the owner table
//! `policy/calibration.md`, and the rewritten policy package.
//!
//! The unit of measurement is a cell: (plan, format, band), split once more
//! by whether the assets are native or a lossless container derivation. Bands
//! stratify so hard content cannot hide in an average; they never appear in
//! the policy. The calibrated plans are the safe and aggressive profiles. Each
//! transform is also scored alone, and each aggressive opt-in as its own
//! stack; those rows inform the owner and set nothing. The two-opt-in stack
//! is scored only as the separation control below the aggressive tier.
//!
//! A row sets a number only when it is eligible: a native or
//! container-derived asset, not an overlay variant of another, at its
//! generator's native rate for audio, emitted natively or transcoded at
//! quality 90 or above for a lossy image, and not near-blank. Everything else
//! is scored and reported in its own rows. Post-processed assets feed the
//! ladder table only.
//!
//! Everything reduces sequentially in manifest order on the scalar path, so
//! two runs over the same manifest produce byte-identical records. The
//! record keys per-asset rows by doc_id and sha256, never by path.
//!
//! `examples/calibrate.rs` is the command over this module. Nothing here
//! reads the network.

pub mod cells;
pub mod evaluate;
pub mod manifest;
pub mod policy_write;
pub mod prepare;
pub mod record;
pub mod table;

pub use cells::*;
pub use evaluate::*;
pub use manifest::*;
pub use policy_write::rewrite_policy;
pub use prepare::*;
pub use record::*;
pub use table::render_markdown;

use crate::policy::PolicyPackage;

pub const RECORD_SCHEMA_VERSION: &str = "1.1.0";
pub const MANIFEST_SCHEMA_VERSION: &str = "1.1.0";

/// The reasoned defaults ruled before calibration, kept beside the calibrated
/// numbers in the record and the owner table.
pub const PROVISIONAL_IMAGE_SAFE: (f64, f64) = (38.0, 0.98);
pub const PROVISIONAL_IMAGE_AGGRESSIVE: (f64, f64) = (30.0, 0.90);
pub const PROVISIONAL_AUDIO_SAFE: f64 = 1.0;
pub const PROVISIONAL_AUDIO_AGGRESSIVE: f64 = 3.0;

pub const IMAGE_BANDS: [&str; 3] = ["small", "medium", "large"];
pub const AUDIO_BANDS: [&str; 3] = ["short", "medium", "long"];

/// A progress sink for the harness.
pub type Progress = Box<dyn FnMut(&str)>;

/// Inputs to one calibration run.
pub struct RunOptions {
    pub manifest_path: std::path::PathBuf,
    /// The directory relative asset paths resolve against. Defaults to the
    /// manifest's directory; an inventory kept outside the corpus names the
    /// corpus root here.
    pub root: Option<std::path::PathBuf>,
    pub date: String,
    pub filters: Filters,
    pub check_determinism: bool,
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
    root: &std::path::Path,
    opts: &mut RunOptions,
) -> Result<Vec<AssetResult>, String> {
    let ctx = Context::new(pkg)?;
    let mut results = Vec::with_capacity(manifest.assets.len());
    for (i, a) in manifest.assets.iter().enumerate() {
        if let Some(p) = opts.progress.as_mut() {
            p(&format!(
                "[{}/{}] {}",
                i + 1,
                manifest.assets.len(),
                a.key()
            ));
        }
        if let Some(reason) = opts.filters.exclusion(a) {
            let mut r = evaluate_asset(&[], a, &ctx);
            r.excluded = Some(reason);
            r.plans.clear();
            results.push(r);
            continue;
        }
        // An admitted asset that cannot be read or whose hash disagrees with
        // the manifest fails the run: the record would otherwise describe a
        // corpus other than the one the digest names.
        let bytes = verify_sha256(root, a)?;
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
    let source_inventory = manifest.source_inventory.clone();
    let prepare_info = manifest.prepare.clone();
    let root = opts.root.clone().unwrap_or_else(|| {
        opts.manifest_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default()
    });
    let date = opts.date.clone();
    let filters = opts.filters.clone();

    let results = one_pass(pkg, &manifest, &root, &mut opts)?;
    let mut record = build_record(
        pkg,
        &results,
        &manifest_name,
        &manifest_sha,
        source_inventory.clone(),
        prepare_info.clone(),
        &filters,
        &date,
    )?;
    if opts.check_determinism {
        if let Some(p) = opts.progress.as_mut() {
            p("second pass for the determinism property");
        }
        let again = one_pass(pkg, &manifest, &root, &mut opts)?;
        let record2 = build_record(
            pkg,
            &again,
            &manifest_name,
            &manifest_sha,
            source_inventory,
            prepare_info,
            &filters,
            &date,
        )?;
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
