//! Exit codes observed from the built binary, not the library. Selecting a
//! degrade profile in the metadata-tier release is a usage error at exit 2,
//! with a message naming the release the profile ships in.

use std::path::PathBuf;
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "fixtures", name]
        .iter()
        .collect()
}

fn run(args: &[&str]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_unmark"))
        .args(args)
        .output()
        .expect("the unmark binary runs");
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    (out.status.code().unwrap_or(-1), stderr)
}

#[test]
fn a_degrade_profile_is_a_usage_error_at_exit_2() {
    let png = fixture("generated.png");
    for profile in [
        "image-safe",
        "image-aggressive",
        "audio-safe",
        "audio-aggressive",
    ] {
        let (code, stderr) = run(&["inspect", "--profile", profile, png.to_str().unwrap()]);
        assert_eq!(code, 2, "{profile} must be a usage error, stderr: {stderr}");
        assert!(
            stderr.contains("not available in this build"),
            "{profile} stderr names the reason: {stderr}"
        );
    }
}

#[test]
fn a_degrade_profile_on_clean_exits_2_and_writes_nothing() {
    let png = fixture("generated.png");
    let out_path = std::env::temp_dir().join("unmark-cli-exit-codes-never-written.png");
    let _ = std::fs::remove_file(&out_path);
    let (code, _) = run(&[
        "clean",
        "--profile",
        "image-safe",
        "--out",
        out_path.to_str().unwrap(),
        "--i-generated-this",
        "--acknowledge-residual",
        png.to_str().unwrap(),
    ]);
    assert_eq!(code, 2);
    assert!(!out_path.exists(), "a usage error must write nothing");
}

#[test]
fn a_metadata_profile_runs_at_exit_0_on_inspect() {
    let png = fixture("generated.png");
    let (code, _) = run(&[
        "inspect",
        "--profile",
        "image-metadata",
        png.to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
}
