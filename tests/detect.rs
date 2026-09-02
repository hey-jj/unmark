//! Detection and the scan-state honesty property.

mod common;
use common::*;
use unmark::asset::{self, Format};
use unmark::detect;
use unmark::scan::ScanState;

fn state(bytes: &[u8], class: &str) -> ScanState {
    detect::inspect(bytes)
        .get(class)
        .map(|d| d.state)
        .expect("class present in detections")
}

#[test]
fn png_text_and_c2pa_are_confirmed_present() {
    let png = build_png(&PngOpts {
        text: true,
        c2pa: true,
        ..Default::default()
    });
    assert_eq!(state(&png, "png_text"), ScanState::ConfirmedPresent);
    assert_eq!(state(&png, "c2pa"), ScanState::ConfirmedPresent);
    // A class scanned in a supported container and not found is confirmed absent.
    assert_eq!(state(&png, "exif"), ScanState::ConfirmedAbsent);
}

#[test]
fn jpeg_exif_xmp_c2pa() {
    let jpg = build_jpeg(&JpegOpts {
        exif: true,
        xmp: true,
        c2pa: true,
        ..Default::default()
    });
    assert_eq!(asset::sniff(&jpg), Format::Jpeg);
    assert_eq!(state(&jpg, "exif"), ScanState::ConfirmedPresent);
    assert_eq!(state(&jpg, "xmp"), ScanState::ConfirmedPresent);
    assert_eq!(state(&jpg, "c2pa"), ScanState::ConfirmedPresent);
}

#[test]
fn wav_list_id3_c2pa() {
    let wav = build_wav(&WavOpts {
        list_info: true,
        id3: true,
        c2pa: true,
        ..Default::default()
    });
    assert_eq!(asset::sniff(&wav), Format::RiffWav);
    assert_eq!(state(&wav, "riff_ancillary"), ScanState::ConfirmedPresent);
    assert_eq!(state(&wav, "id3"), ScanState::ConfirmedPresent);
    assert_eq!(state(&wav, "c2pa"), ScanState::ConfirmedPresent);
}

#[test]
fn webp_metadata() {
    let webp = build_webp(&WebpOpts {
        exif: true,
        xmp: true,
        c2pa: true,
    });
    assert_eq!(asset::sniff(&webp), Format::WebP);
    assert_eq!(state(&webp, "exif"), ScanState::ConfirmedPresent);
    assert_eq!(state(&webp, "c2pa"), ScanState::ConfirmedPresent);
}

#[test]
fn flac_vorbis_comment() {
    let flac = build_flac(true);
    assert_eq!(asset::sniff(&flac), Format::Flac);
    assert_eq!(state(&flac, "vorbis"), ScanState::ConfirmedPresent);
    // FLAC is a supported container, so absence is confirmable.
    let clean = build_flac(false);
    assert_eq!(state(&clean, "vorbis"), ScanState::ConfirmedAbsent);
}

#[test]
fn blind_classes_are_not_attempted() {
    let png = build_png(&PngOpts::default());
    assert_eq!(state(&png, "synthid_image"), ScanState::NotAttempted);
    assert_eq!(state(&png, "tree_ring"), ScanState::NotAttempted);
    assert_eq!(state(&png, "stable_signature"), ScanState::NotAttempted);
}

/// The honesty property: an unsupported container can never report
/// confirmed_absent for any class. A partial scan must never say absent.
#[test]
fn unsupported_container_never_reports_absent() {
    // A binary blob that is not any supported container. RAW-style files land
    // here: invalid UTF-8 and matching no sniff, so they never reach absence.
    let blob = vec![0xC0u8; 64];
    assert_eq!(asset::sniff(&blob), Format::Unknown);
    let det = detect::inspect(&blob);
    for d in &det.items {
        assert_ne!(
            d.state,
            ScanState::ConfirmedAbsent,
            "class {} reported confirmed_absent over an unsupported container",
            d.class
        );
    }
}

/// The supported-container constant is the single authority. Every format in it
/// can reach confirmed_absent, and none outside it can.
#[test]
fn confirmed_absent_only_over_the_supported_constant() {
    for f in asset::SUPPORTED_CONTAINERS {
        assert!(f.is_supported_container());
    }
    // RAW-style and unknown formats are not in the set.
    assert!(!Format::Unknown.is_supported_container());
    assert!(!Format::Mp3.is_supported_container());
    assert!(!Format::RiffAvi.is_supported_container());
}

/// A malformed structure reports malformed, never confirmed_absent.
#[test]
fn malformed_variation_run_is_not_absent() {
    // A long variation-selector run that does not decode to a C2PA wrapper.
    let mut s = String::from("text ");
    for _ in 0..12 {
        s.push('\u{FE0F}');
    }
    let det = detect::inspect(s.as_bytes());
    let invis = det.get("invisibles").expect("invisibles scanned");
    assert_eq!(invis.state, ScanState::Malformed);
}
