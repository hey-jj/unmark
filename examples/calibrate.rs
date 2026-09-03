//! The fidelity-ceiling calibration harness. Three modes over the
//! `unmark::calibrate` module, none of which reads the network:
//!
//! Select: apply the manifest rules, verify every admitted file's sha256,
//! print the selection as JSON, measure nothing.
//!
//!     calibrate --select-only --manifest FILE [--root DIR] [selection flags]
//!
//! Prepare: build the run manifest from an inventory: verify hashes, probe
//! each file (container, band, variance, observed C2PA), merge annotation
//! CSVs by doc_id, mark ladder rows, collapse overlay variants onto their
//! base, write lossless container derivations and synthetic clips outside
//! the corpus, and print the per-cell counts against n_min. No fidelity is
//! measured and no number is written.
//!
//!     calibrate --prepare OUT.json --manifest FILE --root DIR \
//!         [--annotate CSV]... [--ladder FIELD=VALUE]... \
//!         [--derive-dir DIR] [--synth-dir DIR] [selection flags]
//!
//! Run: score every calibrated plan over the manifest, derive the numbers,
//! check the properties, and write `calibration.json`, `calibration.md`,
//! and the rewritten policy package.
//!
//!     calibrate --manifest FILE --out DIR --date YYYY-MM-DD [--root DIR] \
//!         [--policy policy/policy.toml] [--single-pass] [selection flags]
//!
//! Selection flags: --include-round R, --exclude-round R, --include-tag T,
//! --exclude-tag T, --include FIELD=VALUE, --exclude FIELD=VALUE, all
//! repeatable. The manifest is JSON (schema 1.1.0) or an inventory CSV with
//! path, sha256, and format columns.
//!
//! After a run that changed the policy, rebuild and regenerate the skill
//! snapshot, since the package digest covers the record transitively:
//!
//!     cargo run -- policy snapshot --out skills/unmark/references/transforms.md

use std::path::PathBuf;
use std::process::ExitCode;
use unmark::calibrate::{self, Filters, PrepareOptions, RunOptions};
use unmark::policy;

fn usage() -> &'static str {
    "usage:\n  \
     calibrate --manifest FILE --out DIR --date YYYY-MM-DD [--root DIR] [--policy FILE] [--single-pass] [selection]\n  \
     calibrate --prepare OUT.json --manifest FILE [--root DIR] [--annotate CSV]... [--ladder FIELD=VALUE]...\n            \
     [--derive-dir DIR] [--synth-dir DIR] [selection]\n  \
     calibrate --select-only --manifest FILE [--root DIR] [selection]\n\
     selection: [--include-round R]... [--exclude-round R]... [--include-tag T]... [--exclude-tag T]...\n            \
     [--include FIELD=VALUE]... [--exclude FIELD=VALUE]...\n\
     exit 0 when the record was written and accepted (or a prepare or selection completed),\n\
     1 when written but not accepted, 2 on a usage or run error"
}

fn fail(msg: &str) -> ExitCode {
    eprintln!("calibrate: {msg}");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut manifest: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut date: Option<String> = None;
    let mut policy_path = PathBuf::from("policy/policy.toml");
    let mut root: Option<PathBuf> = None;
    let mut single_pass = false;
    let mut select_only = false;
    let mut prepare_out: Option<PathBuf> = None;
    let mut annotations: Vec<PathBuf> = Vec::new();
    let mut ladder_rules: Vec<String> = Vec::new();
    let mut derive_dir: Option<PathBuf> = None;
    let mut synth_dir: Option<PathBuf> = None;
    let mut filters = Filters::default();
    while let Some(a) = args.next() {
        let mut value = |what: &str| -> Result<String, String> {
            args.next().ok_or_else(|| format!("{what} needs a value"))
        };
        let r: Result<(), String> = match a.as_str() {
            "--manifest" => value("--manifest").map(|v| manifest = Some(PathBuf::from(v))),
            "--out" => value("--out").map(|v| out = Some(PathBuf::from(v))),
            "--date" => value("--date").map(|v| date = Some(v)),
            "--policy" => value("--policy").map(|v| policy_path = PathBuf::from(v)),
            "--root" => value("--root").map(|v| root = Some(PathBuf::from(v))),
            "--prepare" => value("--prepare").map(|v| prepare_out = Some(PathBuf::from(v))),
            "--annotate" => value("--annotate").map(|v| annotations.push(PathBuf::from(v))),
            "--ladder" => value("--ladder").map(|v| ladder_rules.push(v)),
            "--derive-dir" => value("--derive-dir").map(|v| derive_dir = Some(PathBuf::from(v))),
            "--synth-dir" => value("--synth-dir").map(|v| synth_dir = Some(PathBuf::from(v))),
            "--single-pass" => {
                single_pass = true;
                Ok(())
            }
            "--select-only" => {
                select_only = true;
                Ok(())
            }
            "--include-round" => value("--include-round").map(|v| filters.include_rounds.push(v)),
            "--exclude-round" => value("--exclude-round").map(|v| filters.exclude_rounds.push(v)),
            "--include-tag" => value("--include-tag").map(|v| filters.include_tags.push(v)),
            "--exclude-tag" => value("--exclude-tag").map(|v| filters.exclude_tags.push(v)),
            "--include" => value("--include").map(|v| filters.include.push(v)),
            "--exclude" => value("--exclude").map(|v| filters.exclude.push(v)),
            "--help" | "-h" => {
                println!("{}", usage());
                return ExitCode::from(0);
            }
            other => Err(format!("unknown argument {other}")),
        };
        if let Err(e) = r {
            eprintln!("calibrate: {e}\n{}", usage());
            return ExitCode::from(2);
        }
    }
    if let Err(e) = filters.validate() {
        return fail(&e);
    }
    let Some(manifest) = manifest else {
        eprintln!("calibrate: --manifest is required\n{}", usage());
        return ExitCode::from(2);
    };
    let root_dir = root.clone().unwrap_or_else(|| {
        manifest
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default()
    });
    let pkg = match policy::load() {
        Ok(p) => p,
        Err(e) => return fail(&format!("policy load failed: {e}")),
    };

    if select_only {
        let loaded = match calibrate::load_manifest(&manifest) {
            Ok(l) => l,
            Err(e) => return fail(&e),
        };
        return match calibrate::select(&loaded, &root_dir, &filters, true) {
            Ok(sel) => {
                println!("{}", serde_json::to_string_pretty(&sel).unwrap());
                ExitCode::from(0)
            }
            Err(e) => fail(&e),
        };
    }

    if let Some(prepare_out) = prepare_out {
        let loaded = match calibrate::load_manifest(&manifest) {
            Ok(l) => l,
            Err(e) => return fail(&e),
        };
        let opts = PrepareOptions {
            root: root_dir,
            filters,
            ladder_rules,
            annotations,
            derive_dir,
            synth_dir,
            progress: Some(Box::new(|line: &str| eprintln!("calibrate: {line}"))),
        };
        let prepared = match calibrate::prepare(&pkg, &loaded, opts) {
            Ok(p) => p,
            Err(e) => return fail(&e),
        };
        let mut json = serde_json::to_string_pretty(&prepared.manifest).unwrap();
        json.push('\n');
        if let Err(e) = std::fs::write(&prepare_out, json) {
            return fail(&format!("{}: {e}", prepare_out.display()));
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&prepared.counts).unwrap()
        );
        eprintln!(
            "calibrate: wrote the run manifest to {} ({} rows)",
            prepare_out.display(),
            prepared.manifest.assets.len()
        );
        return ExitCode::from(0);
    }

    let (Some(out), Some(date)) = (out, date) else {
        eprintln!(
            "calibrate: --out and --date are required for a run\n{}",
            usage()
        );
        return ExitCode::from(2);
    };
    if date.len() != 10 || date.as_bytes()[4] != b'-' || date.as_bytes()[7] != b'-' {
        return fail("--date must be YYYY-MM-DD");
    }
    let opts = RunOptions {
        manifest_path: manifest,
        root,
        date,
        filters,
        check_determinism: !single_pass,
        progress: Some(Box::new(|line: &str| eprintln!("calibrate: {line}"))),
    };
    let result = match calibrate::run(&pkg, opts) {
        Ok(r) => r,
        Err(e) => return fail(&e),
    };

    if let Err(e) = std::fs::create_dir_all(&out) {
        return fail(&format!("{}: {e}", out.display()));
    }
    let json_path = out.join("calibration.json");
    let md_path = out.join("calibration.md");
    if let Err(e) = std::fs::write(&json_path, &result.json) {
        return fail(&format!("{}: {e}", json_path.display()));
    }
    if let Err(e) = std::fs::write(&md_path, &result.markdown) {
        return fail(&format!("{}: {e}", md_path.display()));
    }
    let policy_text = match std::fs::read_to_string(&policy_path) {
        Ok(t) => t,
        Err(e) => return fail(&format!("{}: {e}", policy_path.display())),
    };
    match calibrate::rewrite_policy(&policy_text, &result.record, &result.record_sha256) {
        Ok(text) => {
            if let Err(e) = std::fs::write(&policy_path, text) {
                return fail(&format!("{}: {e}", policy_path.display()));
            }
        }
        Err(e) => return fail(&format!("policy rewrite: {e}")),
    }
    eprintln!(
        "calibrate: wrote {} (sha256 {}), {}, and {}",
        json_path.display(),
        result.record_sha256,
        md_path.display(),
        policy_path.display()
    );
    eprintln!(
        "calibrate: accepted: {}. Rebuild and run `unmark policy snapshot --out skills/unmark/references/transforms.md`.",
        result.record.accepted
    );
    for p in result.record.properties.iter().filter(|p| !p.pass) {
        eprintln!(
            "calibrate: property {} failed{}: {}",
            p.name,
            p.cell
                .as_ref()
                .map(|c| format!(" for {c}"))
                .unwrap_or_default(),
            p.detail
        );
    }
    ExitCode::from(if result.record.accepted { 0 } else { 1 })
}
