//! Residual blocker: a clean never reports success while a targeted class is
//! unknown. A container whose walk breaks before reaching a real mark reports
//! that mark as malformed, and clean must fail closed at exit 40 before any
//! rewrite. Covered for JPEG (safe rewriter, so the clean-level gate is the
//! only defense), FLAC, and ISOBMFF, from in-memory builders and from the
//! committed fixtures.

mod common;
use common::*;
use std::path::PathBuf;
use unmark::asset::Format;
use unmark::container;
use unmark::detect;
use unmark::scan::ScanState;
use unmark::{clean, policy, Options, UnmarkError};

fn pkg() -> policy::PolicyPackage {
    policy::load().expect("policy loads")
}

fn fixture(name: &str) -> Vec<u8> {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "fixtures", name]
        .iter()
        .collect();
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The review probe: an APP1 whose length field claims 0xFFFF bytes, followed
/// by a real APP11 C2PA segment the walk can never reach.
fn malformed_jpeg_with_c2pa_after_corruption() -> Vec<u8> {
    let mut out = vec![0xFF, 0xD8];
    out.extend_from_slice(&[0xFF, 0xE1, 0xFF, 0xFF]);
    out.extend_from_slice(b"Exif\0\0II*\0\x08\0\0\0");
    // A real manifest and the rest of a valid file past the corruption.
    let payload = b"JP\0\0jumb c2pa manifest";
    out.extend_from_slice(&[0xFF, 0xEB]);
    out.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(payload);
    out.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00]);
    out.extend_from_slice(&[0x12, 0x34, 0x56, 0x78, 0xFF, 0xD9]);
    out
}

/// A FLAC whose second metadata block declares a length past the end of the
/// file, so the Vorbis comment it would carry is never reached.
fn malformed_flac() -> Vec<u8> {
    let mut out = b"fLaC".to_vec();
    // STREAMINFO, not the last block.
    out.push(0x00);
    out.extend_from_slice(&[0x00, 0x00, 34]);
    out.extend_from_slice(&[0u8; 34]);
    // VORBIS_COMMENT header claiming 0xFFFFF0 bytes.
    out.push(0x84);
    out.extend_from_slice(&[0xFF, 0xFF, 0xF0]);
    out.extend_from_slice(b"reference libFLAC comfyui");
    out
}

/// An MP4 whose second top-level box declares an impossible size, with a real
/// C2PA uuid box and mdat past it.
fn malformed_mp4_with_c2pa_after_corruption() -> Vec<u8> {
    let (good, _) = build_mp4_with_c2pa_uuid();
    // The good file is ftyp, uuid, moov, mdat. Insert a corrupt box right after
    // ftyp so the walk breaks before the uuid.
    let ftyp_len = u32::from_be_bytes([good[0], good[1], good[2], good[3]]) as usize;
    let mut out = good[..ftyp_len].to_vec();
    out.extend_from_slice(&0xFFFF_FFF0u32.to_be_bytes());
    out.extend_from_slice(b"free");
    out.extend_from_slice(&good[ftyp_len..]);
    out
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

// --- JPEG: the probe ---------------------------------------------------------

#[test]
fn malformed_jpeg_reports_c2pa_as_malformed() {
    let jpg = malformed_jpeg_with_c2pa_after_corruption();
    let det = detect::inspect(&jpg);
    assert_eq!(det.format, Format::Jpeg);
    assert_eq!(det.get("c2pa").map(|d| d.state), Some(ScanState::Malformed));
    assert_never_absent(&det);
}

#[test]
fn clean_on_a_malformed_jpeg_fails_the_required_inspection() {
    let jpg = malformed_jpeg_with_c2pa_after_corruption();
    let err = clean(&jpg, &Options::default(), &pkg()).unwrap_err();
    assert!(matches!(err, UnmarkError::Inspection(_)), "got {err}");
}

// --- FLAC --------------------------------------------------------------------

#[test]
fn malformed_flac_reports_vorbis_as_malformed_and_refuses_to_rewrite() {
    let flac = malformed_flac();
    let det = detect::inspect(&flac);
    assert_eq!(det.format, Format::Flac);
    assert_eq!(
        det.get("vorbis").map(|d| d.state),
        Some(ScanState::Malformed)
    );
    assert_never_absent(&det);

    // The rewriter honors its completeness flag even when it would otherwise be
    // a no-op.
    for spec in [
        container::DropSpec::default(),
        container::DropSpec {
            vorbis: true,
            ..Default::default()
        },
    ] {
        let err = container::rewrite(&flac, Format::Flac, &spec).unwrap_err();
        assert!(
            matches!(err, container::RewriteError::Malformed(_)),
            "got {err}"
        );
    }
}

#[cfg(feature = "audio")]
#[test]
fn clean_on_a_malformed_flac_fails_closed() {
    let flac = malformed_flac();
    let err = clean(&flac, &Options::default(), &pkg()).unwrap_err();
    assert!(matches!(err, UnmarkError::Inspection(_)), "got {err}");
}

// --- ISOBMFF -----------------------------------------------------------------

#[test]
fn malformed_mp4_reports_c2pa_as_malformed_and_refuses_to_rewrite() {
    let mp4 = malformed_mp4_with_c2pa_after_corruption();
    let det = detect::inspect(&mp4);
    assert_eq!(det.format, Format::Isobmff);
    assert_eq!(det.get("c2pa").map(|d| d.state), Some(ScanState::Malformed));
    assert_never_absent(&det);

    let spec = container::DropSpec {
        c2pa: true,
        ..Default::default()
    };
    let err = container::rewrite(&mp4, Format::Isobmff, &spec).unwrap_err();
    assert!(
        matches!(err, container::RewriteError::Malformed(_)),
        "got {err}"
    );
}

#[cfg(feature = "audio")]
#[test]
fn clean_on_a_malformed_mp4_fails_closed_before_any_rewrite() {
    let mp4 = malformed_mp4_with_c2pa_after_corruption();
    let err = clean(&mp4, &Options::default(), &pkg()).unwrap_err();
    assert!(matches!(err, UnmarkError::Inspection(_)), "got {err}");
}

// --- Committed fixtures --------------------------------------------------------

#[test]
fn malformed_jpg_fixture_fails_closed() {
    let jpg = fixture("malformed.jpg");
    let det = detect::inspect(&jpg);
    assert_eq!(det.get("c2pa").map(|d| d.state), Some(ScanState::Malformed));
    assert_never_absent(&det);
    let err = clean(&jpg, &Options::default(), &pkg()).unwrap_err();
    assert!(matches!(err, UnmarkError::Inspection(_)), "got {err}");
}

#[test]
fn malformed_flac_fixture_refuses_to_rewrite() {
    let flac = fixture("malformed.flac");
    let det = detect::inspect(&flac);
    assert_eq!(
        det.get("vorbis").map(|d| d.state),
        Some(ScanState::Malformed)
    );
    assert_never_absent(&det);
    let err = container::rewrite(&flac, Format::Flac, &container::DropSpec::default()).unwrap_err();
    assert!(matches!(err, container::RewriteError::Malformed(_)));
}

#[test]
fn malformed_mp4_fixture_refuses_to_rewrite() {
    let mp4 = fixture("malformed.mp4");
    let det = detect::inspect(&mp4);
    assert_eq!(det.get("c2pa").map(|d| d.state), Some(ScanState::Malformed));
    assert_never_absent(&det);
    let spec = container::DropSpec {
        c2pa: true,
        ..Default::default()
    };
    let err = container::rewrite(&mp4, Format::Isobmff, &spec).unwrap_err();
    assert!(matches!(err, container::RewriteError::Malformed(_)));
}
