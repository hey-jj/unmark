//! Exit codes and the batch model observed from the built binary, not the
//! library.

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "fixtures", name]
        .iter()
        .collect()
}

fn run(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_unmark"))
        .args(args)
        .output()
        .expect("the unmark binary runs");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("unmark-cli-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_profile_flag_is_a_usage_error_and_so_is_a_missing_out() {
    let png = fixture("generated.png");
    let (code, _, err) = run(&[
        "clean",
        "--profile",
        "image-metadata",
        png.to_str().unwrap(),
    ]);
    assert_eq!(code, 2, "{err}");
    let (code, _, err) = run(&["clean", png.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(err.contains("--out"));
    let (code, _, err) = run(&[
        "clean",
        "--keep",
        "unknown-thing",
        "--out",
        "x.png",
        png.to_str().unwrap(),
    ]);
    assert_eq!(code, 2, "{err}");
}

#[test]
fn an_unknown_output_format_is_a_usage_error() {
    let png = fixture("generated.png");
    let (code, out, err) = run(&["inspect", "--output", "yaml", png.to_str().unwrap()]);
    assert_eq!(code, 2, "{err}");
    assert!(out.is_empty(), "nothing is emitted for a refused format");
    assert!(err.contains("--output takes json or text"));
}

#[test]
fn inspect_runs_at_exit_0_and_prints_one_json_object() {
    let png = fixture("generated.png");
    let (code, out, _) = run(&["inspect", png.to_str().unwrap()]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(v["verb"], "inspect");
    assert!(v["capture"]["line"].is_string());
}

#[test]
fn clean_writes_under_out_and_the_report_lists_the_strip() {
    let dir = scratch("single");
    let png = fixture("generated.png");
    let dest = dir.join("out.png");
    let (code, out, err) = run(&[
        "clean",
        "--out",
        dest.to_str().unwrap(),
        png.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{err}");
    assert!(dest.exists());
    let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert!(v["stripped_and_proven_gone"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s == "PNG text chunk"));
    // A second run without --overwrite is a usage error.
    let (code, _, err) = run(&[
        "clean",
        "--out",
        dest.to_str().unwrap(),
        png.to_str().unwrap(),
    ]);
    assert_eq!(code, 2);
    assert!(err.contains("--overwrite"));
}

#[test]
fn a_directory_argument_processes_every_supported_file_independently() {
    let dir = scratch("batch");
    let inputs = dir.join("in");
    std::fs::create_dir_all(&inputs).unwrap();
    std::fs::copy(fixture("generated.png"), inputs.join("a.png")).unwrap();
    std::fs::copy(fixture("invisible-sample.txt"), inputs.join("b.txt")).unwrap();
    std::fs::write(inputs.join("c.bin"), [0xC0u8; 16]).unwrap();
    let outputs = dir.join("out");
    let (code, out, err) = run(&[
        "clean",
        "--out",
        outputs.to_str().unwrap(),
        inputs.to_str().unwrap(),
    ]);
    // The unknown file is unsupported, the others complete; the worst exit
    // wins and the supported files are still written.
    assert_eq!(code, 40, "{err}");
    assert!(outputs.join("a.png").exists());
    assert!(outputs.join("b.txt").exists());
    assert!(!outputs.join("c.bin").exists());
    let reports = out.lines().count();
    assert_eq!(
        reports, 3,
        "one JSON object per input, failures included: {out}"
    );
}

#[test]
fn an_out_extension_that_contradicts_the_emitted_container_exits_2_with_the_report() {
    // A JPEG input comes back as JPEG; asking for .png is a usage error, and
    // the report still says which container the run emits.
    let dir = scratch("ext");
    let jpg = dir.join("in.jpg");
    let img = unmark::codec::Image {
        width: 64,
        height: 48,
        channels: 3,
        data: (0..64 * 48 * 3).map(|i| (i % 251) as u8).collect(),
    };
    std::fs::write(
        &jpg,
        unmark::codec::jpeg::encode(&img, 92, "4:4:4").unwrap(),
    )
    .unwrap();
    let wrong = dir.join("out.png");
    let (code, out, err) = run(&[
        "clean",
        "--out",
        wrong.to_str().unwrap(),
        jpg.to_str().unwrap(),
    ]);
    assert_eq!(code, 2, "{err}");
    assert!(!wrong.exists());
    let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(v["exit_code"], 2);
    assert_eq!(v["output_format"], "jpeg");
    let right = dir.join("out.jpg");
    let (code, _, err) = run(&[
        "clean",
        "--out",
        right.to_str().unwrap(),
        jpg.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{err}");
    assert!(Path::new(&right).exists());
}

#[test]
fn a_malformed_fixture_exits_30_and_an_unknown_blob_exits_40() {
    let dir = scratch("codes");
    let (code, _, err) = run(&[
        "clean",
        "--out",
        dir.join("m.png").to_str().unwrap(),
        fixture("malformed.png").to_str().unwrap(),
    ]);
    assert_eq!(code, 30, "{err}");
    assert!(!dir.join("m.png").exists());
    let blob = dir.join("blob.bin");
    std::fs::write(&blob, [0xC0u8; 32]).unwrap();
    let (code, _, _) = run(&[
        "clean",
        "--out",
        dir.join("o.bin").to_str().unwrap(),
        blob.to_str().unwrap(),
    ]);
    assert_eq!(code, 40);
}
