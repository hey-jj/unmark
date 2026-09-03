//! The review blockers. A partial scan never reports absence, a malformed
//! container is never rewritten, the text run can succeed, and a capture claim
//! inside a variation-selector wrapper is read like any other manifest.

mod common;
use common::*;
use unmark::asset::Format;
use unmark::container;
use unmark::detect;
use unmark::report::EXIT_OK;
use unmark::scan::ScanState;
use unmark::{clean, inspect, policy, Options, UnmarkError};

fn pkg() -> policy::PolicyPackage {
    policy::load().expect("policy loads")
}

/// A PNG whose second chunk declares an impossible length, followed by a real
/// C2PA chunk the walk can never reach. The review probe.
fn malformed_png_with_c2pa_after_corruption() -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']);
    out.extend_from_slice(&png_chunk(
        b"IHDR",
        &[0, 0, 0, 1, 0, 0, 0, 1, 8, 2, 0, 0, 0],
    ));
    // Corrupt: a tEXt header claiming 0xFFFFFFF0 bytes of data.
    out.extend_from_slice(&0xFFFF_FFF0u32.to_be_bytes());
    out.extend_from_slice(b"tEXt");
    out.extend_from_slice(b"parameters\0prompt");
    out.extend_from_slice(&[0, 0, 0, 0]);
    // A real manifest and the rest of a valid file sit past the corruption.
    out.extend_from_slice(&png_chunk(b"caBX", b"jumb\0\0\0\0c2pa manifest store"));
    out.extend_from_slice(&png_chunk(b"IDAT", &[1, 2, 3, 4]));
    out.extend_from_slice(&png_chunk(b"IEND", &[]));
    out
}

/// A WAV whose LIST chunk declares a size past the end of the file, with a real
/// C2PA chunk after it.
fn malformed_wav_with_c2pa_after_corruption() -> Vec<u8> {
    let mut wav = build_wav(&WavOpts {
        list_info: true,
        c2pa: true,
        ..Default::default()
    });
    // Find the LIST chunk id and overwrite its size field.
    let at = wav
        .windows(4)
        .position(|w| w == b"LIST")
        .expect("LIST chunk present");
    wav[at + 4..at + 8].copy_from_slice(&0xFFFF_FFF0u32.to_le_bytes());
    wav
}

// --- BLOCKER 1: a partial scan never reports confirmed_absent ---------------

#[test]
fn malformed_png_reports_unreached_classes_as_malformed_not_absent() {
    let png = malformed_png_with_c2pa_after_corruption();
    let det = detect::inspect(&png);
    assert_eq!(det.format, Format::Png);
    let c2pa = det.get("c2pa").expect("c2pa class scanned");
    assert_eq!(
        c2pa.state,
        ScanState::Malformed,
        "a manifest past a corrupt chunk must read as malformed, never absent"
    );
    for d in &det.items {
        assert_ne!(
            d.state,
            ScanState::ConfirmedAbsent,
            "{} reported confirmed_absent over an incomplete walk",
            d.class
        );
    }
}

#[test]
fn malformed_riff_reports_unreached_classes_as_malformed_not_absent() {
    let wav = malformed_wav_with_c2pa_after_corruption();
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
}

#[test]
fn well_formed_containers_still_reach_absence() {
    // The completeness flag must not make every scan malformed.
    let png = build_png(&PngOpts::default());
    assert_eq!(
        detect::inspect(&png).get("c2pa").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
    let wav = build_wav(&WavOpts::default());
    assert_eq!(
        detect::inspect(&wav).get("c2pa").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
    let jpg = build_jpeg(&JpegOpts::default());
    assert_eq!(
        detect::inspect(&jpg).get("c2pa").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
}

// --- BLOCKER 2: a malformed container is never rewritten -------------------

#[test]
fn png_rewriter_refuses_an_incomplete_walk() {
    let png = malformed_png_with_c2pa_after_corruption();
    let spec = container::DropSpec {
        c2pa: true,
        png_text: true,
        ..Default::default()
    };
    let err = container::rewrite(&png, Format::Png, &spec).unwrap_err();
    assert!(
        matches!(err, container::RewriteError::Malformed(_)),
        "got {err}"
    );
}

#[test]
fn riff_rewriter_refuses_an_incomplete_walk() {
    let wav = malformed_wav_with_c2pa_after_corruption();
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

#[test]
fn clean_on_a_malformed_png_fails_the_required_inspection_and_writes_nothing() {
    // The targeted classes past the corrupt chunk scan as malformed, so the
    // required inspection failed and the run refuses before any rewrite.
    let png = malformed_png_with_c2pa_after_corruption();
    let err = clean(&png, &Options::default(), &pkg()).unwrap_err();
    assert!(matches!(err, UnmarkError::Inspection(_)), "got {err}");
    let spec = container::DropSpec {
        c2pa: true,
        png_text: true,
        ..Default::default()
    };
    let err = container::rewrite(&png, Format::Png, &spec).unwrap_err();
    assert!(matches!(err, container::RewriteError::Malformed(_)));
}

#[cfg(feature = "audio")]
#[test]
fn clean_on_a_malformed_wav_fails_closed_and_writes_nothing() {
    let wav = malformed_wav_with_c2pa_after_corruption();
    let err = clean(&wav, &Options::default(), &pkg()).unwrap_err();
    assert!(matches!(err, UnmarkError::Inspection(_)), "got {err}");
}

// --- BLOCKER 3: the text run succeeds ------------------------------------

#[test]
fn a_text_run_removes_a_zero_width_space_and_reaches_exit_0() {
    let text = "hello\u{200B} world\n";
    let out = clean(text.as_bytes(), &Options::default(), &pkg()).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
    let bytes = out.output.expect("the run writes");
    assert_eq!(String::from_utf8(bytes).unwrap(), "hello world\n");
    assert!(out
        .report
        .stripped_and_proven_gone
        .iter()
        .any(|l| l.contains("invisible")));
}

#[test]
fn a_text_run_removes_a_generator_header_and_keeps_visible_text() {
    let text = "<!-- Generator: ComfyUI 1.0 -->\nA visible line.\n";
    let out = clean(text.as_bytes(), &Options::default(), &pkg()).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK);
    let bytes = out.output.expect("the run writes");
    assert_eq!(String::from_utf8(bytes).unwrap(), "A visible line.\n");
}

#[test]
fn text_signal_ignores_removable_spans_and_catches_visible_edits() {
    let before = "a\u{200B}b\n<!-- Generator: x -->\nc\n";
    let after_ok = "ab\nc\n";
    assert_eq!(
        container::signal_stream(before.as_bytes(), Format::Text),
        container::signal_stream(after_ok.as_bytes(), Format::Text),
        "removing only invisibles and a generator line must not move the signal"
    );
    let after_bad = "ab\nX\n";
    assert_ne!(
        container::signal_stream(before.as_bytes(), Format::Text),
        container::signal_stream(after_bad.as_bytes(), Format::Text),
        "a change to visible content must move the signal"
    );
}

// --- The capture rule reads a wrapper ----------------------------------------

fn encode_vs(bytes: &[u8]) -> String {
    let mut s = String::new();
    for &b in bytes {
        let cp = if b < 16 {
            0xFE00 + b as u32
        } else {
            0xE0100 + (b as u32 - 16)
        };
        s.push(char::from_u32(cp).unwrap());
    }
    s
}

#[test]
fn a_wrapper_carrying_a_capture_claim_is_certified_capture_and_kept() {
    let wrapper = encode_vs(b"jumb c2pa.captured signer Leica Camera AG");
    let text = format!("caption{wrapper}\n");
    let out = clean(text.as_bytes(), &Options::default(), &pkg()).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK);
    assert_eq!(out.report.capture.status, "certified");
    assert_eq!(
        out.output.as_deref(),
        Some(text.as_bytes()),
        "byte-identical"
    );
    let strip = Options {
        strip_capture: true,
        ..Default::default()
    };
    let out = clean(text.as_bytes(), &strip, &pkg()).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK);
    assert_eq!(String::from_utf8(out.output.unwrap()).unwrap(), "caption\n");
}

#[test]
fn a_publisher_wrapper_without_a_capture_action_is_stripped_by_default() {
    let wrapper = encode_vs(b"jumb c2pa credit: Reuters");
    let text = format!("caption{wrapper}\n");
    let out = clean(text.as_bytes(), &Options::default(), &pkg()).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK);
    assert_eq!(out.report.capture.status, "none");
    assert_eq!(String::from_utf8(out.output.unwrap()).unwrap(), "caption\n");
}

#[test]
fn a_generative_wrapper_is_stripped_by_default() {
    let wrapper = encode_vs(b"jumb c2pa claim_generator: ComfyUI");
    let text = format!("caption{wrapper}\n");
    let out = clean(text.as_bytes(), &Options::default(), &pkg()).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK);
    assert_eq!(String::from_utf8(out.output.unwrap()).unwrap(), "caption\n");
}

#[test]
fn inspect_on_a_malformed_container_is_still_read_only_and_reports() {
    let png = malformed_png_with_c2pa_after_corruption();
    let r = inspect(&png, &Options::default(), &pkg()).unwrap();
    assert!(r
        .scan_states
        .iter()
        .any(|s| s.class == "c2pa" && s.state == ScanState::Malformed));
}
