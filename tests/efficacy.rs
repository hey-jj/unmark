//! The efficacy properties: every builder-rendered fixture watermarked through
//! the oracle reads present under the declared rule before the default run
//! and absent after it, with the oracle's recovered bits agreeing; the
//! false-positive fraction of the rule on unmarked builder-rendered images is
//! measured and pinned; the pinned resize ratio is the measured one.
//!
//! The oracle is the invisible-watermark Python package, run as a subprocess
//! through `tools/oracle.py`, never linked. CI installs it and sets
//! `UNMARK_ORACLE_REQUIRED=1`, so the oracle checks fail there when it is
//! absent; a local run without it skips those checks and says so.

use std::path::PathBuf;
use std::process::Command;
use unmark::codec::{self, Image};
use unmark::mark::dwtdct;
use unmark::{clean, policy, Options};

fn fixture_dir() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "fixtures", "efficacy"]
        .iter()
        .collect()
}

/// The committed marked fixtures that the declared rule reads present, with
/// the payload each carries.
const MARKED: &[(&str, &str)] = &[
    ("flat-illustration-sdxl.png", "sdxl-48"),
    ("flat-illustration-compvis.png", "compvis-136"),
    ("dense-texture-sdxl.png", "sdxl-48"),
    ("dense-texture-compvis.png", "compvis-136"),
    ("text-ui-sdxl.png", "sdxl-48"),
    ("text-ui-compvis.png", "compvis-136"),
    ("photo-like-sdxl.png", "sdxl-48"),
    ("small-sdxl.png", "sdxl-48"),
];

/// Marked fixtures the rule reads below its threshold even before cleaning:
/// the oracle's own decode of them sits below 0.80 too. They exercise the
/// rule's direction, never the removal claim.
const WEAK: &[&str] = &[
    "photo-like-compvis.png",
    "small-compvis.png",
    "photo-like-sdxl.jpg",
    "small-sdxl.jpg",
];

fn read(name: &str) -> Vec<u8> {
    std::fs::read(fixture_dir().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn decode(bytes: &[u8]) -> Image {
    let format = unmark::asset::sniff(bytes);
    codec::decode_image(bytes, format).unwrap()
}

/// The oracle, when it can run: the interpreter named by
/// `UNMARK_ORACLE_PYTHON` (default `python3`) with the package importable.
fn oracle() -> Option<String> {
    let py = std::env::var("UNMARK_ORACLE_PYTHON").unwrap_or_else(|_| "python3".to_string());
    let script: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tools", "oracle.py"]
        .iter()
        .collect();
    let ok = Command::new(&py)
        .arg(&script)
        .arg("version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if ok {
        return Some(py);
    }
    if std::env::var("UNMARK_ORACLE_REQUIRED").as_deref() == Ok("1") {
        panic!("UNMARK_ORACLE_REQUIRED is set and the oracle is not importable with {py}");
    }
    eprintln!("oracle not available with {py}; the oracle agreement checks are skipped");
    None
}

fn oracle_bits(py: &str, path: &std::path::Path, n: usize) -> Vec<u8> {
    let script: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tools", "oracle.py"]
        .iter()
        .collect();
    let out = Command::new(py)
        .arg(&script)
        .arg("decode")
        .arg(path)
        .arg(n.to_string())
        .output()
        .expect("oracle runs");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .bytes()
        .map(|c| (c == b'1') as u8)
        .collect()
}

#[test]
fn every_marked_fixture_reads_present_before_and_absent_after_the_default_run() {
    let pkg = policy::load().unwrap();
    let py = oracle();
    let scratch = std::env::temp_dir().join(format!("unmark-efficacy-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    for (name, payload) in MARKED {
        let bytes = read(name);
        let img = decode(&bytes);
        let before = dwtdct::detect(&img);
        let d = before.iter().find(|d| d.payload == *payload).unwrap();
        assert!(
            d.present,
            "{name}: {payload} reads absent before cleaning (agreement {})",
            d.agreement
        );
        let out = clean(&bytes, &Options::default(), &pkg).unwrap();
        assert_eq!(out.report.exit_code, 0, "{name}: {:?}", out.report.actions);
        assert!(
            out.report
                .stripped_and_proven_gone
                .iter()
                .any(|l| l == "dwtDct pixel mark"),
            "{name}: the mark is not listed as stripped and proven gone"
        );
        let px02 = out
            .report
            .actions
            .iter()
            .find(|a| a.transform == "PX02")
            .unwrap();
        assert!(
            px02.result.starts_with("removed and proven gone"),
            "{name}: {}",
            px02.result
        );
        let written = out.output.unwrap();
        let after = dwtdct::detect(&decode(&written));
        let d = after.iter().find(|d| d.payload == *payload).unwrap();
        assert!(
            !d.present,
            "{name}: {payload} still reads present (agreement {})",
            d.agreement
        );
        if let Some(py) = &py {
            // The oracle's recovered bits agree with the in-crate decode on
            // the same image, before and after.
            let expected = dwtdct::payload_bits(payload).unwrap();
            let (mine_before, _) = dwtdct::decode(&img, expected.len());
            let o_before = oracle_bits(py, &fixture_dir().join(name), expected.len());
            let agree_before = dwtdct::agreement(&mine_before, &o_before);
            assert!(
                agree_before >= 0.9,
                "{name}: in-crate and oracle bits disagree before cleaning ({agree_before:.3})"
            );
            let out_path = scratch.join(name);
            std::fs::write(&out_path, &written).unwrap();
            let o_after = oracle_bits(py, &out_path, expected.len());
            let o_agreement = dwtdct::agreement(&o_after, &expected);
            assert!(
                o_agreement < dwtdct::AGREEMENT_THRESHOLD,
                "{name}: the oracle still recovers the payload after cleaning ({o_agreement:.3})"
            );
        }
    }
    let _ = std::fs::remove_dir_all(&scratch);
}

#[test]
fn the_weak_fixtures_read_below_the_rule_before_cleaning_and_stay_below() {
    let pkg = policy::load().unwrap();
    for name in WEAK {
        let bytes = read(name);
        let before = dwtdct::detect(&decode(&bytes));
        assert!(before.iter().all(|d| !d.present), "{name}: {before:?}");
        let out = clean(&bytes, &Options::default(), &pkg).unwrap();
        assert_eq!(out.report.exit_code, 0, "{name}: {:?}", out.report.actions);
        let px02 = out
            .report
            .actions
            .iter()
            .find(|a| a.transform == "PX02")
            .unwrap();
        assert!(
            px02.result
                .starts_with("applied; no dwtDct payload decoded"),
            "{name}: {}",
            px02.result
        );
    }
}

/// A builder-rendered unmarked image: gradients, texture, and flat regions
/// from a seeded generator.
fn unmarked(seed: u64, w: usize, h: usize) -> Image {
    let mut rng = unmark::dsp::Rng::seed(seed);
    let kind = seed % 4;
    let (fx, fy, amp) = (
        0.01 + rng.uniform() * 0.1,
        0.01 + rng.uniform() * 0.1,
        20.0 + rng.uniform() * 80.0,
    );
    let base = [
        60.0 + rng.uniform() * 130.0,
        60.0 + rng.uniform() * 130.0,
        60.0 + rng.uniform() * 130.0,
    ];
    let noise = if kind == 2 { 12.0 } else { 2.0 };
    let mut data = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        for x in 0..w {
            let t = match kind {
                0 => amp * unmark::dsp::sin(x as f64 * fx) * unmark::dsp::cos(y as f64 * fy),
                1 => amp * (x as f64 / w as f64 - 0.5) + amp * (y as f64 / h as f64 - 0.5),
                2 => amp * unmark::dsp::sin(x as f64 * fx * 5.0 + y as f64 * fy * 3.0),
                _ => {
                    if ((x / 40) + (y / 40)) % 2 == 0 {
                        amp
                    } else {
                        -amp
                    }
                }
            };
            for b in base {
                let v = b + t + rng.gaussian() * noise;
                data.push(v.round().clamp(0.0, 255.0) as u8);
            }
        }
    }
    Image {
        width: w,
        height: h,
        channels: 3,
        data,
    }
}

#[test]
fn the_false_positive_fraction_of_the_rule_on_unmarked_images_is_measured() {
    // 200 builder-rendered unmarked images across four pattern families and
    // several sizes, plus the five unmarked fixture bases.
    let mut checked = 0usize;
    let mut fired = 0usize;
    let mut max_agreement: f64 = 0.0;
    for seed in 0..200u64 {
        let (w, h) = match seed % 5 {
            0 => (256, 256),
            1 => (320, 240),
            2 => (512, 384),
            3 => (640, 400),
            _ => (400, 300),
        };
        let img = unmarked(seed, w, h);
        for d in dwtdct::detect(&img) {
            checked += 1;
            max_agreement = max_agreement.max(d.agreement);
            if d.present {
                fired += 1;
            }
        }
    }
    for name in [
        "flat-illustration-base.png",
        "dense-texture-base.png",
        "photo-like-base.png",
        "text-ui-base.png",
        "small-base.png",
    ] {
        for d in dwtdct::detect(&decode(&read(name))) {
            checked += 1;
            max_agreement = max_agreement.max(d.agreement);
            if d.present {
                fired += 1;
            }
        }
    }
    let fraction = fired as f64 / checked as f64;
    eprintln!(
        "false-positive fraction at threshold {}: {fired}/{checked} = {fraction:.4}, max agreement {max_agreement:.3}",
        dwtdct::AGREEMENT_THRESHOLD
    );
    assert_eq!(
        fired, 0,
        "the rule fired on unmarked images: {fired} of {checked}"
    );
    assert!(max_agreement < dwtdct::AGREEMENT_THRESHOLD - 0.05);
}

#[test]
fn the_pinned_resize_ratio_is_the_measured_one_and_bounded_by_the_citation() {
    let pkg = policy::load().unwrap();
    let ratio = pkg.transform("PX02").unwrap().param_f64("ratio").unwrap();
    assert!(
        (0.5..=0.95).contains(&ratio),
        "ratio {ratio} is outside the cited bound"
    );
    assert_eq!(ratio, 0.95, "the measured pin");
    // At the pin, every marked fixture reads absent with margin.
    for (name, payload) in MARKED {
        let img = decode(&read(name));
        let small = unmark::transform::pixel::resize(&img, ratio);
        let d = dwtdct::detect(&small);
        let d = d.iter().find(|d| d.payload == *payload).unwrap();
        assert!(
            d.agreement < dwtdct::AGREEMENT_THRESHOLD - 0.1,
            "{name}: agreement {} after the pinned resize",
            d.agreement
        );
    }
}
