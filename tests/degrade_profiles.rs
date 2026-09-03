//! The image degrade profiles end to end, under the owner's PX01 decision: a
//! baseline JPEG when the decoded image is opaque, a reported skip when it
//! carries alpha, lossy WebP the same after decode, lossless WebP as PNG, and
//! a usage error when the --out extension contradicts the emitted container.

use std::path::PathBuf;
use std::process::Command;
use unmark::codec::{self, Image};
use unmark::{clean, plan, policy, Options};

/// A smooth gradient, which a Q92 encode reproduces well above the safe floor.
fn gradient(w: usize, h: usize, channels: usize) -> Image {
    let mut data = Vec::with_capacity(w * h * channels);
    for y in 0..h {
        for x in 0..w {
            data.push((x * 255 / (w - 1)) as u8);
            data.push((y * 255 / (h - 1)) as u8);
            data.push(((x + y) * 255 / (w + h - 2)) as u8);
            if channels == 4 {
                data.push(200);
            }
        }
    }
    Image {
        width: w,
        height: h,
        channels,
        data,
    }
}

fn opts() -> Options {
    Options {
        i_generated_this: true,
        acknowledge_residual: true,
        ..Options::default()
    }
}

fn action_outcome(report: &unmark::report::Report, id: &str) -> String {
    report
        .actions
        .iter()
        .find(|a| a.transform == id)
        .map(|a| a.outcome.clone())
        .unwrap_or_default()
}

#[test]
fn an_opaque_png_comes_out_as_a_baseline_jpeg_within_budget() {
    let pkg = policy::load().unwrap();
    let png = codec::png::encode(&gradient(160, 120, 3)).unwrap();
    let out = clean(&png, "image-safe", &opts(), &pkg).unwrap();
    assert_eq!(out.report.exit_code, 0, "{:?}", out.report.actions);
    assert_eq!(out.report.output_format, "jpeg");
    let bytes = out.output.expect("output written");
    assert_eq!(&bytes[..3], &[0xFF, 0xD8, 0xFF]);
    assert_eq!(action_outcome(&out.report, "PX01"), "applied");
    let f = out.report.fidelity.as_ref().unwrap();
    assert!(f.passed && f.psnr_db.unwrap() >= 38.0 && f.ssim.unwrap() >= 0.98);
    assert!((f.resample_ratio - 0.9).abs() < 1e-9);
    // Weakened blind marks land in the middle part, never in "removed".
    assert!(out
        .report
        .degraded_without_proof
        .iter()
        .any(|d| d.contains("SynthID")));
    // The output decodes at the resampled size.
    let img = codec::jpeg::decode(&bytes).unwrap();
    assert_eq!((img.width, img.height), (144, 108));
}

#[test]
fn an_alpha_png_skips_the_reencode_and_says_so() {
    let pkg = policy::load().unwrap();
    let png = codec::png::encode(&gradient(96, 64, 4)).unwrap();
    let planned = plan(&png, "image-safe", &Options::default(), &pkg).unwrap();
    assert_eq!(planned.output_format, "png");
    assert!(
        action_outcome(&planned, "PX01")
            .starts_with("skipped: re-encode skipped: the image carries alpha"),
        "{}",
        action_outcome(&planned, "PX01")
    );
    let out = clean(&png, "image-safe", &opts(), &pkg).unwrap();
    assert_eq!(out.report.exit_code, 0);
    assert_eq!(out.report.output_format, "png");
    assert!(action_outcome(&out.report, "PX01").starts_with("skipped:"));
    let img = codec::png::decode(&out.output.unwrap()).unwrap();
    assert_eq!(img.channels, 4, "alpha is never flattened");
    assert_eq!((img.width, img.height), (86, 58));
}

#[test]
fn a_lossy_webp_comes_out_as_a_jpeg_after_decode() {
    let pkg = policy::load().unwrap();
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "fixtures", "lossy-pattern.webp"]
        .iter()
        .collect();
    let webp = std::fs::read(path).unwrap();
    assert!(codec::webp::is_lossy(&webp).unwrap());
    // The pattern is broadband, so the aggressive floor is the one it clears.
    let out = clean(&webp, "image-aggressive", &opts(), &pkg).unwrap();
    assert_eq!(out.report.exit_code, 0, "{:?}", out.report.fidelity);
    assert_eq!(out.report.output_format, "jpeg");
    assert_eq!(&out.output.unwrap()[..3], &[0xFF, 0xD8, 0xFF]);
}

#[test]
fn a_lossless_webp_follows_the_png_rule() {
    let pkg = policy::load().unwrap();
    let opaque = codec::webp::encode_lossless(&gradient(128, 96, 3)).unwrap();
    let out = clean(&opaque, "image-safe", &opts(), &pkg).unwrap();
    assert_eq!(out.report.exit_code, 0);
    assert_eq!(out.report.output_format, "jpeg");
    let alpha = codec::webp::encode_lossless(&gradient(128, 96, 4)).unwrap();
    let out = clean(&alpha, "image-safe", &opts(), &pkg).unwrap();
    assert_eq!(out.report.exit_code, 0);
    assert_eq!(out.report.output_format, "webp");
    assert!(action_outcome(&out.report, "PX01").starts_with("skipped:"));
    assert!(!codec::webp::is_lossy(&out.output.unwrap()).unwrap());
}

#[test]
fn a_plan_over_budget_refuses_at_exit_30_and_writes_nothing() {
    let pkg = policy::load().unwrap();
    // A busy pattern misses the safe floor at one encode.
    let mut img = gradient(128, 128, 3);
    let mut rng = unmark::dsp::Rng::seed(3);
    for v in img.data.iter_mut() {
        *v = (*v as f64 + rng.gaussian() * 40.0)
            .round()
            .clamp(0.0, 255.0) as u8;
    }
    let png = codec::png::encode(&img).unwrap();
    let out = clean(&png, "image-safe", &opts(), &pkg).unwrap();
    assert_eq!(out.report.exit_code, 30);
    assert!(out.output.is_none());
    let f = out.report.fidelity.as_ref().unwrap();
    assert!(!f.passed);
    assert!(action_outcome(&out.report, "PX01").starts_with("refused:"));
}

#[test]
fn the_cli_exits_2_when_the_out_extension_contradicts_the_emitted_container() {
    let dir = std::env::temp_dir().join(format!("unmark-degrade-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("in.png");
    std::fs::write(&input, codec::png::encode(&gradient(160, 120, 3)).unwrap()).unwrap();
    let wrong = dir.join("out.png");
    let out = Command::new(env!("CARGO_BIN_EXE_unmark"))
        .args([
            "clean",
            "--profile",
            "image-safe",
            "--i-generated-this",
            "--acknowledge-residual",
            "--out",
        ])
        .arg(&wrong)
        .arg(&input)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(!wrong.exists(), "nothing is written on the contradiction");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("emits jpeg"), "{stderr}");
    // The report is still emitted, carrying the usage exit and the container.
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["exit_code"], 2);
    assert_eq!(report["output_format"], "jpeg");
    let right = dir.join("out.jpg");
    let out = Command::new(env!("CARGO_BIN_EXE_unmark"))
        .args([
            "clean",
            "--profile",
            "image-safe",
            "--i-generated-this",
            "--acknowledge-residual",
            "--out",
        ])
        .arg(&right)
        .arg(&input)
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(right.exists());
    let _ = std::fs::remove_dir_all(&dir);
}
