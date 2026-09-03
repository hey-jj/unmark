//! The fidelity-ceiling calibration harness. It takes a corpus manifest and an
//! output directory, scores every calibrated plan over the corpus, and writes
//! `calibration.json`, `calibration.md`, and the rewritten policy package.
//! It never reads the network.
//!
//!     cargo run --release --example calibrate --all-features -- \
//!         --manifest /corpus/manifest.json --out policy --date 2026-09-02 \
//!         [--root DIR] [--policy policy/policy.toml] [--single-pass] \
//!         [--include-round R]... [--exclude-round R]... \
//!         [--include-tag T]... [--exclude-tag T]... \
//!         [--include FIELD=VALUE]... [--exclude FIELD=VALUE]...
//!
//! The manifest is JSON (schema 1.0.0) or an inventory CSV with `path`,
//! `sha256`, and `format` columns. `--select-only` prints the selection as
//! JSON after verifying every admitted file's sha256 and measures nothing.
//!
//! After a run that changed the policy, rebuild and regenerate the skill
//! snapshot, since the package digest covers the record transitively:
//!
//!     cargo run -- policy snapshot --out skills/unmark/references/transforms.md

use std::path::PathBuf;
use std::process::ExitCode;
use unmark::calibrate::{self, Filters, RunOptions};
use unmark::policy;

fn usage() -> &'static str {
    "usage: calibrate --manifest FILE --out DIR --date YYYY-MM-DD [--root DIR] [--policy FILE] [--single-pass]\n       \
     [--include-round R]... [--exclude-round R]... [--include-tag T]... [--exclude-tag T]...\n       \
     [--include FIELD=VALUE]... [--exclude FIELD=VALUE]...\n       \
     calibrate --select-only --manifest FILE [--root DIR] [selection flags]\n\
     The manifest is JSON (schema 1.0.0) or an inventory CSV with path, sha256, and format columns.\n\
     --select-only prints the selection as JSON after verifying every admitted file's sha256 and measures nothing.\n\
     exit 0 when the record was written and accepted, 1 when written but not accepted, 2 on a usage or run error"
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
        eprintln!("calibrate: {e}");
        return ExitCode::from(2);
    }

    if select_only {
        let Some(manifest) = manifest else {
            eprintln!("calibrate: --manifest is required\n{}", usage());
            return ExitCode::from(2);
        };
        let root_dir = root.unwrap_or_else(|| {
            manifest
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_default()
        });
        let loaded = match calibrate::load_manifest(&manifest) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("calibrate: {e}");
                return ExitCode::from(2);
            }
        };
        return match calibrate::select(&loaded, &root_dir, &filters, true) {
            Ok(sel) => {
                println!("{}", serde_json::to_string_pretty(&sel).unwrap());
                ExitCode::from(0)
            }
            Err(e) => {
                eprintln!("calibrate: {e}");
                ExitCode::from(2)
            }
        };
    }

    let (Some(manifest), Some(out), Some(date)) = (manifest, out, date) else {
        eprintln!(
            "calibrate: --manifest, --out, and --date are required\n{}",
            usage()
        );
        return ExitCode::from(2);
    };
    if date.len() != 10 || date.as_bytes()[4] != b'-' || date.as_bytes()[7] != b'-' {
        eprintln!("calibrate: --date must be YYYY-MM-DD");
        return ExitCode::from(2);
    }

    let pkg = match policy::load() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("calibrate: policy load failed: {e}");
            return ExitCode::from(2);
        }
    };
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
        Err(e) => {
            eprintln!("calibrate: {e}");
            return ExitCode::from(2);
        }
    };

    if let Err(e) = std::fs::create_dir_all(&out) {
        eprintln!("calibrate: {}: {e}", out.display());
        return ExitCode::from(2);
    }
    let json_path = out.join("calibration.json");
    let md_path = out.join("calibration.md");
    if let Err(e) = std::fs::write(&json_path, &result.json) {
        eprintln!("calibrate: {}: {e}", json_path.display());
        return ExitCode::from(2);
    }
    if let Err(e) = std::fs::write(&md_path, &result.markdown) {
        eprintln!("calibrate: {}: {e}", md_path.display());
        return ExitCode::from(2);
    }
    let policy_text = match std::fs::read_to_string(&policy_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("calibrate: {}: {e}", policy_path.display());
            return ExitCode::from(2);
        }
    };
    match calibrate::rewrite_policy(&policy_text, &result.record, &result.record_sha256) {
        Ok(text) => {
            if let Err(e) = std::fs::write(&policy_path, text) {
                eprintln!("calibrate: {}: {e}", policy_path.display());
                return ExitCode::from(2);
            }
        }
        Err(e) => {
            eprintln!("calibrate: policy rewrite: {e}");
            return ExitCode::from(2);
        }
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
