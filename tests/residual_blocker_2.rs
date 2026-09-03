//! Residual blocker 2 and the 0.1.0 follow-up rulings. A RIFF size field that
//! understates the file hides every chunk past the declared end, and an empty
//! signal stream is no proof of identity. Plus: the text formats belong to the
//! supported constant, and the policy strings agree with the rewriter about
//! which transform owns the RIFF C2PA chunk.

mod common;
use common::*;
use std::path::PathBuf;
use unmark::asset::{Format, SUPPORTED_CONTAINERS};
use unmark::budget;
use unmark::container;
use unmark::detect;
use unmark::policy;
use unmark::scan::ScanState;
#[cfg(feature = "audio")]
use unmark::{clean, Options, UnmarkError};

fn fixture(name: &str) -> Vec<u8> {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "fixtures", name]
        .iter()
        .collect();
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The review probe: a WAV whose RIFF size covers only the form tag and the
/// `fmt ` chunk, with a real C2PA chunk and the `data` chunk past that end.
fn understated_size_wav() -> Vec<u8> {
    let mut wav = build_wav(&WavOpts {
        list_info: true,
        c2pa: true,
        ..Default::default()
    });
    // "WAVE" (4) plus the fmt chunk (8 header + 16 data).
    wav[4..8].copy_from_slice(&28u32.to_le_bytes());
    wav
}

fn assert_never_absent(det: &unmark::scan::Detections) {
    for d in &det.items {
        assert_ne!(
            d.state,
            ScanState::ConfirmedAbsent,
            "{} reported confirmed_absent over an incomplete walk",
            d.class
        );
    }
}

// --- Part 1: bytes past the declared end make the walk incomplete -----------

#[test]
fn understated_riff_size_is_an_incomplete_walk() {
    let wav = understated_size_wav();
    let (chunks, complete) = detect::riff::chunks(&wav);
    assert!(
        !complete,
        "chunks past the declared end were never looked at"
    );
    assert_eq!(
        chunks.len(),
        1,
        "only the fmt chunk sits inside the declared size"
    );
}

#[test]
fn understated_riff_size_reports_unknown_not_absent() {
    let wav = understated_size_wav();
    let det = detect::inspect(&wav);
    assert_eq!(det.format, Format::RiffWav);
    for class in ["c2pa", "id3", "riff_ancillary"] {
        assert_eq!(
            det.get(class).map(|d| d.state),
            Some(ScanState::Malformed),
            "{class} must read as malformed while the C2PA chunk sits past the declared end"
        );
    }
    assert_never_absent(&det);
}

#[test]
fn understated_riff_size_refuses_to_rewrite() {
    let wav = understated_size_wav();
    let spec = container::DropSpec {
        riff_ancillary: true,
        c2pa: true,
        id3: true,
        ..Default::default()
    };
    let err = container::rewrite(&wav, Format::RiffWav, &spec).unwrap_err();
    assert!(
        matches!(err, container::RewriteError::Malformed(_)),
        "got {err}"
    );
}

#[cfg(feature = "audio")]
#[test]
fn clean_on_an_understated_riff_size_fails_closed_and_writes_nothing() {
    let wav = understated_size_wav();
    let pkg = policy::load().unwrap();
    let err = clean(&wav, &Options::default(), &pkg).unwrap_err();
    assert!(matches!(err, UnmarkError::Inspection(_)), "got {err}");
}

#[test]
fn a_single_trailing_pad_byte_is_still_a_complete_walk() {
    let mut wav = build_wav(&WavOpts {
        c2pa: true,
        ..Default::default()
    });
    wav.push(0);
    let (_, complete) = detect::riff::chunks(&wav);
    assert!(complete, "one pad byte past the declared end is allowed");
    let det = detect::inspect(&wav);
    assert_eq!(
        det.get("c2pa").map(|d| d.state),
        Some(ScanState::ConfirmedPresent)
    );
    assert_eq!(
        det.get("id3").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
}

#[test]
fn two_trailing_bytes_are_not_allowed() {
    let mut wav = build_wav(&WavOpts::default());
    wav.extend_from_slice(&[0, 0]);
    let (_, complete) = detect::riff::chunks(&wav);
    assert!(!complete);
}

// --- Part 2: an empty signal stream proves nothing --------------------------

#[test]
fn understated_riff_size_yields_an_empty_signal_stream() {
    // This is the trap: the data chunk sits past the declared end, so the
    // walker sees no payload and the stream is empty on both sides.
    let wav = understated_size_wav();
    assert!(container::signal_stream(&wav, Format::RiffWav).is_empty());
}

#[test]
fn an_empty_signal_stream_never_passes_the_byte_identity_gate() {
    assert!(budget::check_byte_identity(b"", b"").is_err());
    assert!(budget::check_byte_identity(b"", b"abc").is_err());
    assert!(budget::check_byte_identity(b"abc", b"").is_err());
    assert!(budget::check_byte_identity(b"abc", b"abc").is_ok());
}

// --- The committed fixture ---------------------------------------------------

#[test]
fn understated_size_wav_fixture_reports_unknown_and_refuses_to_rewrite() {
    let wav = fixture("understated-size.wav");
    let det = detect::inspect(&wav);
    assert_eq!(det.format, Format::RiffWav);
    assert_eq!(det.get("c2pa").map(|d| d.state), Some(ScanState::Malformed));
    assert_never_absent(&det);
    let spec = container::DropSpec {
        riff_ancillary: true,
        c2pa: true,
        ..Default::default()
    };
    let err = container::rewrite(&wav, Format::RiffWav, &spec).unwrap_err();
    assert!(matches!(err, container::RewriteError::Malformed(_)));
}

// --- Follow-ups ruled into 0.1.0 -----------------------------------------------

#[test]
fn the_text_formats_are_in_the_supported_constant() {
    for f in [Format::Svg, Format::Html, Format::Text] {
        assert!(
            SUPPORTED_CONTAINERS.contains(&f),
            "{} must be in the constant so absence over it is licensed there",
            f.as_str()
        );
        assert!(f.is_supported_container());
    }
    // Absence over a text file goes through the constant, not a side channel.
    let det = detect::inspect(b"plain text with nothing hidden\n");
    assert_eq!(det.format, Format::Text);
    assert_eq!(
        det.get("invisibles").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
    let pkg = policy::load().unwrap();
    for name in ["svg", "html", "text"] {
        assert!(pkg.supported_containers.iter().any(|c| c == name));
    }
}

#[test]
fn the_policy_strings_route_the_riff_c2pa_chunk_to_mc01() {
    let pkg = policy::load().unwrap();
    let mc09 = pkg.transform("MC09").unwrap();
    assert!(
        !mc09.target.contains("C2PA"),
        "MC09 must not claim the RIFF C2PA chunk: {}",
        mc09.target
    );
    let riff = pkg.mark_class("riff_ancillary").unwrap();
    assert!(
        !riff.guard.contains("or a RIFF C2PA chunk"),
        "riff_ancillary must not claim the RIFF C2PA chunk as ancillary"
    );
    assert!(riff.guard.contains("MC01"));
    let mc01 = pkg.transform("MC01").unwrap();
    assert!(mc01.target.contains("RIFF C2PA chunk"));
}
