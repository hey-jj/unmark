//! Read-only detection. `inspect` sniffs the format, walks the container, and
//! reports one state per mark class. It decodes no pixel or sample data and
//! never mutates the input.

pub mod c2pa;
pub mod exif;
pub mod flac;
pub mod id3;
pub mod invisibles;
pub mod isobmff;
pub mod jpeg;
pub mod png_text;
pub mod riff;
pub mod xmp;

use crate::asset::{self, Format, Media};
use crate::scan::{absence_for, Detection, Detections, Honesty, ScanState};

/// Confirmable container classes applicable to each format. A class not listed
/// for a format is not scannable there and reports `unsupported_format`.
fn applicable_confirmable(format: Format) -> &'static [(&'static str, &'static str)] {
    match format {
        Format::Png => &[
            ("png_text", "PNG text chunk"),
            ("c2pa", "C2PA manifest"),
            ("exif", "EXIF metadata"),
            ("xmp", "XMP packet"),
        ],
        Format::Jpeg => &[
            ("exif", "EXIF metadata"),
            ("xmp", "XMP packet"),
            ("c2pa", "C2PA manifest"),
            ("iptc", "IPTC block"),
        ],
        Format::WebP => &[
            ("exif", "EXIF metadata"),
            ("xmp", "XMP packet"),
            ("c2pa", "C2PA manifest"),
            ("riff_ancillary", "RIFF ancillary chunk"),
        ],
        Format::RiffWav | Format::RiffAvi => &[
            ("id3", "ID3 tag"),
            ("riff_ancillary", "RIFF ancillary chunk"),
            ("c2pa", "C2PA manifest"),
        ],
        Format::Isobmff => &[("c2pa", "C2PA manifest"), ("ilst", "MP4 ilst tag")],
        Format::Mp3 => &[("id3", "ID3 tag")],
        Format::Flac => &[("vorbis", "Vorbis comment")],
        Format::Svg | Format::Html | Format::Text => &[("invisibles", "invisible Unicode")],
        _ => &[],
    }
}

/// Blind classes reported as `not_attempted`, keyed by media axis.
fn blind_classes(media: Media) -> &'static [(&'static str, &'static str)] {
    match media {
        Media::Image => &[
            ("synthid_image", "SynthID-Image"),
            ("tree_ring", "Tree-Ring"),
            ("stable_signature", "Stable Signature"),
        ],
        Media::Audio => &[
            ("synthid_audio", "SynthID-Audio"),
            ("audioseal", "AudioSeal"),
        ],
        _ => &[],
    }
}

/// Unaddressed classes reported as instrumentation notes.
fn unaddressed_classes(media: Media) -> &'static [(&'static str, &'static str)] {
    match media {
        Media::Image => &[
            ("visible_overlay", "visible overlay"),
            ("c2pa_soft_binding", "C2PA soft binding"),
            ("generative_fingerprint", "generative fingerprint"),
        ],
        Media::Audio => &[("c2pa_soft_binding", "C2PA soft binding")],
        _ => &[],
    }
}

/// Inspect one asset. Pure: a function of the input bytes alone.
pub fn inspect(bytes: &[u8]) -> Detections {
    let format = asset::sniff(bytes);
    // Each walker reports whether it covered the container exhaustively. An
    // incomplete walk (a corrupt length, a truncated chunk) must never let an
    // unreported class fall through to absence, so a class the walk did not
    // reach is Malformed rather than confirmed_absent.
    let (mut found, complete): (Vec<Detection>, bool) = match format {
        Format::Png => png_text::scan(bytes),
        Format::Jpeg => jpeg::scan(bytes),
        Format::WebP | Format::RiffWav | Format::RiffAvi => riff::scan(bytes, format),
        Format::Isobmff => isobmff::scan(bytes),
        Format::Mp3 => id3::scan(bytes),
        Format::Flac => flac::scan(bytes),
        Format::Svg | Format::Html | Format::Text => invisibles::scan(bytes),
        Format::Ogg | Format::Unknown => (Vec::new(), false),
    };

    // Fill applicable confirmable classes that no detector reported.
    for (class, label) in applicable_confirmable(format) {
        if found.iter().any(|d| &d.class == class) {
            continue;
        }
        // Absence is reachable only through `absence_for`, which keys on the
        // supported-format constant. A walk that broke before covering the
        // whole container leaves an unreported class unknown instead.
        let state = if !complete {
            ScanState::Malformed
        } else {
            absence_for(format)
        };
        found.push(Detection::with_state(
            class,
            label,
            Honesty::Confirmable,
            state,
        ));
    }

    // Blind classes are always not_attempted in an offline build.
    let media = format.media();
    for (class, label) in blind_classes(media) {
        found.push(Detection::with_state(
            class,
            label,
            Honesty::Blind,
            ScanState::NotAttempted,
        ));
    }
    // Unaddressed classes are named so their absence from the action list is
    // not read as their absence from the asset.
    for (class, label) in unaddressed_classes(media) {
        found.push(Detection::with_state(
            class,
            label,
            Honesty::Unaddressed,
            ScanState::NotAttempted,
        ));
    }

    Detections {
        format,
        items: found,
    }
}

// --- shared byte helpers, all bounds-checked -------------------------------

pub(crate) fn be_u32(b: &[u8], at: usize) -> Option<u32> {
    let s = b.get(at..at + 4)?;
    Some(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

pub(crate) fn le_u32(b: &[u8], at: usize) -> Option<u32> {
    let s = b.get(at..at + 4)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

pub(crate) fn be_u16(b: &[u8], at: usize) -> Option<u16> {
    let s = b.get(at..at + 2)?;
    Some(u16::from_be_bytes([s[0], s[1]]))
}

/// A printable-ASCII preview of a byte slice, for evidence. Non-printable
/// bytes are dropped. Truncated so a hostile field cannot flood the report.
pub(crate) fn ascii_preview(b: &[u8], max: usize) -> String {
    b.iter()
        .filter(|c| c.is_ascii_graphic() || **c == b' ')
        .take(max)
        .map(|c| *c as char)
        .collect()
}
