//! The shipped fixtures, read from disk and driven through detection, the
//! container round trip, and the clean verb. The malformed fixtures prove the
//! fail-closed paths against committed bytes rather than in-memory builders.

use std::path::PathBuf;
use unmark::asset::Format;
use unmark::container;
use unmark::detect;
use unmark::report::EXIT_OK;
use unmark::scan::ScanState;
use unmark::{clean, policy, Options};

fn fixture(name: &str) -> Vec<u8> {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "fixtures", name]
        .iter()
        .collect();
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn ack_opts() -> Options {
    Options {
        i_generated_this: true,
        acknowledge_residual: true,
        ..Default::default()
    }
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
    assert!(det
        .get("png_text")
        .unwrap()
        .evidence
        .iter()
        .any(|e| e.contains("parameters")));

    // Round trip: the IDAT stream is byte-identical and the marks are gone.
    let before = container::signal_stream(&png, Format::Png);
    let pkg = policy::load().unwrap();
    let out = clean(&png, "image-metadata", &ack_opts(), &pkg).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK);
    let bytes = out.output.expect("exit 0 writes output");
    assert_eq!(before, container::signal_stream(&bytes, Format::Png));
    let after = detect::inspect(&bytes);
    assert_eq!(
        after.get("png_text").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
    assert_eq!(
        after.get("c2pa").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
}

#[test]
fn tagged_wav_fixture_detects_and_round_trips() {
    let wav = fixture("tagged.wav");
    let det = detect::inspect(&wav);
    assert_eq!(det.format, Format::RiffWav);
    assert_eq!(
        det.get("riff_ancillary").map(|d| d.state),
        Some(ScanState::ConfirmedPresent)
    );
    assert_eq!(
        det.get("c2pa").map(|d| d.state),
        Some(ScanState::ConfirmedPresent)
    );

    let before = container::signal_stream(&wav, Format::RiffWav);
    let spec = container::DropSpec {
        riff_ancillary: true,
        c2pa: true,
        id3: true,
        ..Default::default()
    };
    let out = container::rewrite(&wav, Format::RiffWav, &spec).unwrap();
    assert_eq!(before, container::signal_stream(&out, Format::RiffWav));
    let declared = u32::from_le_bytes([out[4], out[5], out[6], out[7]]) as usize;
    assert_eq!(declared, out.len() - 8, "RIFF size rebuilt");
    let after = detect::inspect(&out);
    assert_eq!(
        after.get("riff_ancillary").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
    assert_eq!(
        after.get("c2pa").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
}

#[cfg(feature = "audio")]
#[test]
fn tagged_wav_fixture_cleans_under_audio_metadata() {
    let wav = fixture("tagged.wav");
    let pkg = policy::load().unwrap();
    let out = clean(&wav, "audio-metadata", &ack_opts(), &pkg).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK);
    assert!(out.output.is_some());
}

#[test]
fn invisible_sample_fixture_cleans_under_repo_files() {
    let text = fixture("invisible-sample.txt");
    let det = detect::inspect(&text);
    assert_eq!(
        det.get("invisibles").map(|d| d.state),
        Some(ScanState::ConfirmedPresent)
    );
    let pkg = policy::load().unwrap();
    let out = clean(&text, "repo-files", &ack_opts(), &pkg).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
    let bytes = out.output.expect("exit 0 writes output");
    let s = String::from_utf8(bytes).unwrap();
    assert!(!s.contains('\u{200B}'));
    assert!(!s.contains("Generator:"));
    assert!(s.contains("Ordinary line."));
}

#[test]
fn malformed_png_fixture_never_reports_absence_and_refuses_to_clean() {
    let png = fixture("malformed.png");
    let det = detect::inspect(&png);
    assert_eq!(det.format, Format::Png);
    assert_eq!(det.get("c2pa").map(|d| d.state), Some(ScanState::Malformed));
    for d in &det.items {
        assert_ne!(
            d.state,
            ScanState::ConfirmedAbsent,
            "{} reported absent",
            d.class
        );
    }
    let pkg = policy::load().unwrap();
    let out = clean(&png, "image-metadata", &ack_opts(), &pkg).unwrap();
    assert_eq!(out.report.exit_code, unmark::report::EXIT_UNSUPPORTED);
    assert!(out.output.is_none(), "nothing may be written");
}

#[test]
fn malformed_wav_fixture_never_reports_absence_and_refuses_to_rewrite() {
    let wav = fixture("malformed.wav");
    let det = detect::inspect(&wav);
    assert_eq!(det.format, Format::RiffWav);
    assert_eq!(det.get("c2pa").map(|d| d.state), Some(ScanState::Malformed));
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
        c2pa: true,
        ..Default::default()
    };
    let err = container::rewrite(&wav, Format::RiffWav, &spec).unwrap_err();
    assert!(
        matches!(err, container::RewriteError::Malformed(_)),
        "got {err}"
    );
}
