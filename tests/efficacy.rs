//! The efficacy properties: every builder-rendered fixture watermarked through
//! the oracle reads present under the declared rule before the default run
//! and absent after it, with the oracle's recovered bits agreeing; the
//! false-positive fraction of the rule on unmarked builder-rendered images is
//! measured and pinned; the pinned resize ratio is the measured one.
//!
//! The oracle is the reference embedding package, run as a subprocess
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

/// The committed marked fixtures, with the payload each carries. Every one
/// decodes at or above `FIXTURE_FLOOR` through the in-crate rule and the
/// oracle before cleaning. The two JPEG entries are quality-95 4:4:4 encodes.
const MARKED: &[(&str, &str)] = &[
    ("flat-illustration-sdxl.png", "sdxl-48"),
    ("flat-illustration-compvis.png", "compvis-136"),
    ("dense-texture-sdxl.png", "sdxl-48"),
    ("dense-texture-compvis.png", "compvis-136"),
    ("text-ui-sdxl.png", "sdxl-48"),
    ("text-ui-compvis.png", "compvis-136"),
    ("photo-like-sdxl.png", "sdxl-48"),
    ("photo-like-compvis.png", "compvis-136"),
    ("photo-like-sdxl.jpg", "sdxl-48"),
    ("small-sdxl.png", "sdxl-48"),
    ("small-compvis.png", "compvis-136"),
    ("small-sdxl.jpg", "sdxl-48"),
];

/// The agreement every marked fixture must reach before cleaning, through
/// the in-crate rule and through the oracle.
const FIXTURE_FLOOR: f64 = 0.9;

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
            d.present && d.agreement >= FIXTURE_FLOOR,
            "{name}: {payload} reads {} before cleaning, under the fixture floor",
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
            let o_strength = dwtdct::agreement(&o_before, &expected);
            assert!(
                o_strength >= FIXTURE_FLOOR,
                "{name}: the oracle reads {o_strength:.3} before cleaning, under the fixture floor"
            );
            let agree_before = dwtdct::agreement(&mine_before, &o_before);
            assert!(
                agree_before >= 0.9,
                "{name}: in-crate and oracle bits disagree before cleaning ({agree_before:.3})"
            );
            // The oracle refuses an image under 256 pixels on an edge, so
            // an output the crop and resize took below that is read in-crate
            // only, and that read matches the oracle's arithmetic.
            let out_img = decode(&written);
            if out_img.width >= 256 && out_img.height >= 256 {
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
    }
    let _ = std::fs::remove_dir_all(&scratch);
}

/// A chroma-subsampled JPEG halves the U channel the mark lives in. The
/// reference decoder fails on such input as well, so the row reads
/// unsupported with the measured note, and the resize claims no removal.
#[test]
fn a_chroma_subsampled_jpeg_reads_unsupported_and_the_resize_claims_no_removal() {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "fixtures",
        "subsampled-sdxl-420.jpg",
    ]
    .iter()
    .collect();
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(unmark::detect::jpeg::chroma_subsampled(&bytes), Some(true));
    let pkg = policy::load().unwrap();
    let r = unmark::inspect(&bytes, &Options::default(), &pkg).unwrap();
    let row = r.scan_states.iter().find(|s| s.class == "dwtdct").unwrap();
    assert_eq!(row.state, unmark::scan::ScanState::UnsupportedFormat);
    assert!(
        r.findings.iter().all(|f| f.class != "dwtdct"),
        "an unsupported row carries no finding sentence"
    );
    let out = clean(&bytes, &Options::default(), &pkg).unwrap();
    assert_eq!(out.report.exit_code, 0, "{:?}", out.report.actions);
    let px02 = out
        .report
        .actions
        .iter()
        .find(|a| a.transform == "PX02")
        .unwrap();
    assert!(
        px02.result.contains("no dwtDct payload decoded"),
        "{}",
        px02.result
    );
    assert!(!out
        .report
        .stripped_and_proven_gone
        .iter()
        .any(|l| l == "dwtDct pixel mark"));
    // The same mark in a 4:4:4 JPEG decodes and is removed.
    let full = read("photo-like-sdxl.jpg");
    assert_eq!(unmark::detect::jpeg::chroma_subsampled(&full), Some(false));
    let out = clean(&full, &Options::default(), &pkg).unwrap();
    assert!(out
        .report
        .stripped_and_proven_gone
        .iter()
        .any(|l| l == "dwtDct pixel mark"));
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

/// Flat content is the decoder-parity boundary: the embed lands every carrier
/// on a multiple of the step, the reference decoder reads chance, and the
/// in-crate rule reads the same chance. Both decoders say absent, and the
/// in-crate bits match the oracle's on a PNG.
#[test]
fn flat_content_reads_absent_in_both_decoders_and_the_reads_agree() {
    let Some(py) = oracle() else {
        return;
    };
    let scratch = std::env::temp_dir().join(format!("unmark-flat-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let flat = Image {
        width: 288,
        height: 256,
        channels: 3,
        data: [184u8, 146, 93]
            .iter()
            .copied()
            .cycle()
            .take(288 * 256 * 3)
            .collect(),
    };
    let base = scratch.join("flat.png");
    std::fs::write(&base, codec::png::encode(&flat).unwrap()).unwrap();
    let script: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tools", "oracle.py"]
        .iter()
        .collect();
    for (name, spec, payload) in [
        ("flat-sdxl.png", "hex:B3EC907BB19E:48", "sdxl-48"),
        ("flat-compvis.png", "text:StableDiffusionV1", "compvis-136"),
    ] {
        let marked = scratch.join(name);
        let status = Command::new(&py)
            .arg(&script)
            .arg("encode")
            .arg(&base)
            .arg(&marked)
            .arg(spec)
            .status()
            .unwrap();
        assert!(status.success());
        let img = decode(&std::fs::read(&marked).unwrap());
        let expected = dwtdct::payload_bits(payload).unwrap();
        let (mine, _) = dwtdct::decode(&img, expected.len());
        let theirs = oracle_bits(&py, &marked, expected.len());
        assert_eq!(
            mine, theirs,
            "{name}: the in-crate bits differ from the oracle's"
        );
        let d = dwtdct::detect(&img);
        let d = d.iter().find(|d| d.payload == payload).unwrap();
        assert!(
            !d.present,
            "{name}: flat content read present at {}",
            d.agreement
        );
    }
    let _ = std::fs::remove_dir_all(&scratch);
}
