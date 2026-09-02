//! Guardrail logic. The transforms are general-purpose, so these make misuse
//! hard to reach by accident and hard-stop the strong-signal cases.
//!
//! G1 (ownership assertion) and G4 (no aggressive batch) live at the CLI. G5
//! (safety hashes), G6 (no keyed-mark scorer), and G7 (never fabricate
//! provenance) are constraints on what this build contains rather than runtime
//! checks: no code path implements them, which is the enforcement. G2 and G3
//! read the asset and live here.

use crate::asset::Format;
use crate::detect::exif::{parse_tiff, ExifFacts};
use crate::scan::Detections;

/// A signal that fires the third-party-provenance refusal (G2).
#[derive(Clone, Debug)]
pub struct G2Signal {
    pub kind: String,
    pub detail: String,
}

/// Camera vendors used by both the capture-signer read and the G3 heuristic.
fn camera_makes() -> Vec<String> {
    include_str!("../policy/signatures/camera-makes.txt")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// Third-party publisher names. A named news or stock organization in the
/// signer or the author assertions fires the refusal.
const PUBLISHERS: &[&str] = &[
    "associated press",
    "reuters",
    "getty images",
    "getty",
    "agence france-presse",
    "shutterstock",
    "adobe stock",
    "bloomberg",
    "the new york times",
    "bbc",
    "national geographic",
    "magnum photos",
];

/// C2PA capture-action and camera-signer markers. A manifest asserting a
/// capture describes a photograph, not a generated asset.
const CAPTURE_MARKERS: &[&str] = &["c2pa.captured", "c2pa.capture", "cai.capture"];

/// Scan for a capture or third-party-publisher signal in the asset's manifest
/// or descriptive metadata. A generative tool as claim generator does not fire
/// this, because that is the normal case for an asset in scope.
pub fn scan_g2(bytes: &[u8], det: &Detections) -> Option<G2Signal> {
    // C2PA manifest content: capture actions and publisher or camera signers.
    if let Some(c2pa) = det.get("c2pa") {
        for loc in &c2pa.locations {
            let region = bytes
                .get(loc.offset..loc.offset + loc.length)
                .unwrap_or(&[]);
            // A C2PA text wrapper carries its content variation-selector encoded,
            // so the capture and publisher markers are invisible in the raw
            // bytes. Decode the wrapper and match on both the raw region and the
            // decoded content.
            let lower = manifest_haystack(region);
            for m in CAPTURE_MARKERS {
                if window_contains(&lower, m.as_bytes()) {
                    return Some(G2Signal {
                        kind: "capture".to_string(),
                        detail: format!("the manifest asserts {m}"),
                    });
                }
            }
            for p in PUBLISHERS {
                if window_contains(&lower, p.as_bytes()) {
                    return Some(G2Signal {
                        kind: "third-party-publisher".to_string(),
                        detail: format!("the manifest names {p}"),
                    });
                }
            }
            for mk in camera_makes() {
                let ml = mk.to_ascii_lowercase();
                if window_contains(&lower, ml.as_bytes()) {
                    return Some(G2Signal {
                        kind: "capture".to_string(),
                        detail: format!("a camera vendor ({mk}) signs the manifest"),
                    });
                }
            }
        }
    }
    // XMP descriptive credit naming a publisher.
    if let Some(xmp) = det.get("xmp") {
        for loc in &xmp.locations {
            let region = bytes
                .get(loc.offset..loc.offset + loc.length)
                .unwrap_or(&[]);
            let lower = to_lower_ascii(region);
            for p in PUBLISHERS {
                if window_contains(&lower, p.as_bytes()) {
                    return Some(G2Signal {
                        kind: "third-party-publisher".to_string(),
                        detail: format!("XMP credit names {p}"),
                    });
                }
            }
        }
    }
    None
}

/// The camera-origin heuristic (G3). EXIF naming a real camera body with lens
/// data and a shutter, aperture, and ISO triple describes a photograph. Returns
/// a warning string. This is friction rather than proof: a generative tool can
/// write fake EXIF and a real photograph can be model-edited.
pub fn scan_g3(bytes: &[u8], format: Format) -> Option<String> {
    let facts = exif_facts(bytes, format)?;
    let make = facts.make.as_deref()?;
    let known = camera_makes().iter().any(|m| {
        make.eq_ignore_ascii_case(m) || make.to_ascii_lowercase().contains(&m.to_ascii_lowercase())
    });
    if known && facts.has_exposure_triple() && facts.has_lens {
        Some(format!(
            "EXIF names a camera body ({}) with lens and a shutter, aperture, and ISO triple, which describes a photograph",
            make
        ))
    } else {
        None
    }
}

/// Pull EXIF facts from a supported image container without decoding pixels.
pub fn exif_facts(bytes: &[u8], format: Format) -> Option<ExifFacts> {
    match format {
        Format::Jpeg => {
            let (segments, _) = crate::detect::jpeg::segments(bytes);
            for s in segments {
                if s.marker == 0xE1 && s.data.starts_with(crate::detect::jpeg::EXIF_ID) {
                    return parse_tiff(&s.data[crate::detect::jpeg::EXIF_ID.len()..]);
                }
            }
            None
        }
        Format::Png => {
            let (chunks, _) = crate::detect::png_text::chunks(bytes);
            for c in chunks {
                if c.kind == *b"eXIf" {
                    return parse_tiff(c.data);
                }
            }
            None
        }
        Format::WebP => {
            let (chunks, _) = crate::detect::riff::chunks(bytes);
            for c in chunks {
                if &c.id == b"EXIF" {
                    let payload = c
                        .data
                        .strip_prefix(crate::detect::jpeg::EXIF_ID)
                        .unwrap_or(c.data);
                    return parse_tiff(payload);
                }
            }
            None
        }
        _ => None,
    }
}

fn to_lower_ascii(b: &[u8]) -> Vec<u8> {
    b.iter().map(|c| c.to_ascii_lowercase()).collect()
}

/// The lowercased bytes to match capture and publisher markers against. This is
/// the raw region, plus the decoded content of any variation-selector run the
/// region carries, so a manifest ridden inside a C2PA text wrapper is read the
/// same as one carried in binary JUMBF.
fn manifest_haystack(region: &[u8]) -> Vec<u8> {
    let mut hay = to_lower_ascii(region);
    if let Ok(text) = std::str::from_utf8(region) {
        let mut run: Vec<char> = Vec::new();
        for c in text.chars() {
            if crate::detect::invisibles::is_variation_selector(c) {
                run.push(c);
            } else if !run.is_empty() {
                if let Some(decoded) = crate::detect::c2pa::decode_variation_run(&run) {
                    hay.push(b' ');
                    hay.extend(decoded.iter().map(|b| b.to_ascii_lowercase()));
                }
                run.clear();
            }
        }
        if !run.is_empty() {
            if let Some(decoded) = crate::detect::c2pa::decode_variation_run(&run) {
                hay.push(b' ');
                hay.extend(decoded.iter().map(|b| b.to_ascii_lowercase()));
            }
        }
    }
    hay
}

fn window_contains(hay: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || hay.len() < needle.len() {
        return false;
    }
    hay.windows(needle.len()).any(|w| w == needle)
}
