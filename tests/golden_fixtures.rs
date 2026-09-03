//! The shipped fixtures, read from disk and driven through detection, the
//! default run, and the fail-closed paths.

use std::path::PathBuf;
use unmark::asset::Format;
use unmark::container;
use unmark::detect;
use unmark::report::EXIT_OK;
use unmark::scan::ScanState;
use unmark::{clean, policy, Options, UnmarkError};

fn fixture(name: &str) -> Vec<u8> {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "fixtures", name]
        .iter()
        .collect();
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn generated_png_fixture_detects_and_cleans() {
    let png = fixture("generated.png");
    let det = detect::inspect(&png);
    assert_eq!(det.format, Format::Png);
    assert_eq!(
        det.get("png_text").map(|d| d.state),
        Some(ScanState::ConfirmedPresent)
    );
    assert_eq!(
        det.get("c2pa").map(|d| d.state),
        Some(ScanState::ConfirmedPresent)
    );
    let pkg = policy::load().unwrap();
    // With --no-degrade the pixel stream is byte-identical and the marks go.
    let opts = Options {
        no_degrade: true,
        ..Default::default()
    };
    let before = container::signal_stream(&png, Format::Png);
    let out = clean(&png, &opts, &pkg).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
    let bytes = out.output.expect("the run writes");
    assert_eq!(before, container::signal_stream(&bytes, Format::Png));
    let after = detect::inspect(&bytes);
    for class in ["png_text", "c2pa"] {
        assert_eq!(
            after.get(class).map(|d| d.state),
            Some(ScanState::ConfirmedAbsent)
        );
    }
    // The default run resizes and re-inspects; the marks are gone there too.
    let out = clean(&png, &Options::default(), &pkg).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
    let after = detect::inspect(&out.output.unwrap());
    assert_eq!(
        after.get("png_text").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
}

#[cfg(feature = "audio")]
#[test]
fn tagged_wav_fixture_strips_tags_and_keeps_samples_under_no_degrade() {
    let wav = fixture("tagged.wav");
    let det = detect::inspect(&wav);
    assert_eq!(det.format, Format::RiffWav);
    let pkg = policy::load().unwrap();
    let before = container::signal_stream(&wav, Format::RiffWav);
    let opts = Options {
        no_degrade: true,
        ..Default::default()
    };
    let out = clean(&wav, &opts, &pkg).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
    let bytes = out.output.unwrap();
    assert_eq!(before, container::signal_stream(&bytes, Format::RiffWav));
    let after = detect::inspect(&bytes);
    assert_eq!(
        after.get("riff_ancillary").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
    assert_eq!(
        after.get("id3").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
    assert!(out
        .report
        .kept
        .iter()
        .any(|k| k.item.starts_with("AU06") && k.reason.contains("--no-degrade")));
}

#[test]
fn invisible_sample_fixture_cleans_by_default() {
    let text = fixture("invisible-sample.txt");
    let pkg = policy::load().unwrap();
    let out = clean(&text, &Options::default(), &pkg).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
    let after = detect::inspect(&out.output.unwrap());
    assert_eq!(
        after.get("invisibles").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
}

#[test]
fn malformed_png_fixture_never_reports_absence_and_fails_closed() {
    let png = fixture("malformed.png");
    let det = detect::inspect(&png);
    for d in &det.items {
        assert_ne!(
            d.state,
            ScanState::ConfirmedAbsent,
            "{} reported absent",
            d.class
        );
    }
    let pkg = policy::load().unwrap();
    let err = clean(&png, &Options::default(), &pkg).unwrap_err();
    assert!(matches!(err, UnmarkError::Inspection(_)), "got {err}");
}

#[test]
fn malformed_wav_fixture_never_reports_absence_and_refuses_to_rewrite() {
    let wav = fixture("malformed.wav");
    let det = detect::inspect(&wav);
    for d in &det.items {
        assert_ne!(
            d.state,
            ScanState::ConfirmedAbsent,
            "{} reported absent",
            d.class
        );
    }
    let spec = container::DropSpec {
        riff_ancillary: true,
        id3: true,
        c2pa: true,
        ..Default::default()
    };
    let err = container::rewrite(&wav, Format::RiffWav, &spec).unwrap_err();
    assert!(matches!(err, container::RewriteError::Malformed(_)));
}
