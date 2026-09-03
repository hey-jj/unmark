//! The capture reading. An asset is certified capture when its active,
//! well-formed C2PA manifest carries a `c2pa.created` action whose digital
//! source type is `digitalCapture`, or a signer naming a camera vendor, and
//! no later action naming a generative tool. A claim is read, never
//! signature-verified. Camera-style EXIF without such a claim is uncertain:
//! stripped by default, with the hint named. A publisher manifest without a
//! capture action is not a capture at all.
//!
//! The rest of the old guardrail set is a constraint on what this build
//! contains rather than a runtime check: no safety-hash targeting, no keyed
//! mark scorer, no fabricated provenance. No code path implements them, which
//! is the enforcement.

use crate::asset::Format;
use crate::detect::exif::{parse_tiff, ExifFacts};
use crate::scan::Detections;

/// What the asset's provenance says about capture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaptureStatus {
    /// A well-formed capture claim with no later generative action.
    Certified,
    /// A capture claim followed by an action naming a generative tool.
    Generative,
    /// Camera-style EXIF without a qualifying claim.
    Uncertain,
    /// No capture signal.
    None,
}

impl CaptureStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            CaptureStatus::Certified => "certified",
            CaptureStatus::Generative => "generative",
            CaptureStatus::Uncertain => "uncertain",
            CaptureStatus::None => "none",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureReading {
    pub status: CaptureStatus,
    /// The quoted claim markers for a certified or generative reading.
    pub claim: Option<String>,
    /// The named hint for an uncertain reading.
    pub hint: Option<String>,
}

pub const SIGNATURE_STATUS: &str = "not signature-verified";

/// The capture action markers. `c2pa.created` with the IPTC
/// `digitalCapture` source type is the specified form; the older
/// `c2pa.captured` and `cai.capture` spellings are read as the same claim.
const CAPTURE_ACTIONS: &[&str] = &["c2pa.captured", "c2pa.capture", "cai.capture"];
const CREATED_ACTION: &str = "c2pa.created";
const DIGITAL_CAPTURE: &str = "digitalcapture";

/// Generative digital source types and the action that names a tool.
const GENERATIVE_SOURCE_TYPES: &[&str] = &[
    "trainedalgorithmicmedia",
    "compositewithtrainedalgorithmicmedia",
    "algorithmicmedia",
];

/// Camera vendors used by both the capture-signer read and the EXIF hint.
fn camera_makes() -> Vec<String> {
    include_str!("../policy/signatures/camera-makes.txt")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

fn generator_strings() -> Vec<String> {
    include_str!("../policy/signatures/generator-strings.txt")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_ascii_lowercase)
        .collect()
}

/// Read the capture status from the asset's manifest and EXIF.
pub fn read_capture(bytes: &[u8], det: &Detections, format: Format) -> CaptureReading {
    if let Some(c2pa) = det.get("c2pa") {
        for loc in &c2pa.locations {
            let region = bytes
                .get(loc.offset..loc.offset + loc.length)
                .unwrap_or(&[]);
            let hay = manifest_haystack(region);
            if let Some(reading) = read_manifest(&hay) {
                return reading;
            }
        }
    }
    if let Some(hint) = exif_hint(bytes, format) {
        return CaptureReading {
            status: CaptureStatus::Uncertain,
            claim: None,
            hint: Some(hint),
        };
    }
    CaptureReading {
        status: CaptureStatus::None,
        claim: None,
        hint: None,
    }
}

/// The capture reading of one manifest region, lowercased. None when the
/// manifest carries no capture claim.
fn read_manifest(hay: &[u8]) -> Option<CaptureReading> {
    // The capture claim and where it sits.
    let mut capture_at: Option<(usize, String)> = None;
    if let Some(at) = find(hay, CREATED_ACTION.as_bytes(), 0) {
        if find(hay, DIGITAL_CAPTURE.as_bytes(), 0).is_some() {
            capture_at = Some((
                at,
                "c2pa.created with digitalSourceType digitalCapture".to_string(),
            ));
        }
    }
    if capture_at.is_none() {
        for m in CAPTURE_ACTIONS {
            if let Some(at) = find(hay, m.as_bytes(), 0) {
                capture_at = Some((at, format!("action {m}")));
                break;
            }
        }
    }
    if capture_at.is_none() {
        for mk in camera_makes() {
            let ml = mk.to_ascii_lowercase();
            if let Some(at) = find(hay, ml.as_bytes(), 0) {
                capture_at = Some((at, format!("signer names the camera vendor {mk}")));
                break;
            }
        }
    }
    let (at, claim) = capture_at?;
    // A later action naming a generative tool: a generative source type or a
    // generator string after the capture claim.
    let after = &hay[at..];
    let mut later: Option<String> = None;
    for st in GENERATIVE_SOURCE_TYPES {
        if find(after, st.as_bytes(), 1).is_some() {
            later = Some(format!("a later action with digitalSourceType {st}"));
            break;
        }
    }
    if later.is_none() {
        for g in generator_strings() {
            if find(after, g.as_bytes(), 1).is_some() {
                later = Some(format!("a later action naming {g}"));
                break;
            }
        }
    }
    Some(match later {
        Some(l) => CaptureReading {
            status: CaptureStatus::Generative,
            claim: Some(format!("{claim}; {l}")),
            hint: None,
        },
        None => CaptureReading {
            status: CaptureStatus::Certified,
            claim: Some(claim),
            hint: None,
        },
    })
}

/// The EXIF hint: a known camera body with lens data and a shutter,
/// aperture, and ISO triple describes a photograph, without proving one.
pub fn exif_hint(bytes: &[u8], format: Format) -> Option<String> {
    let facts = exif_facts(bytes, format)?;
    let make = facts.make.as_deref()?;
    let known = camera_makes().iter().any(|m| {
        make.eq_ignore_ascii_case(m) || make.to_ascii_lowercase().contains(&m.to_ascii_lowercase())
    });
    if known && facts.has_exposure_triple() && facts.has_lens {
        Some(format!(
            "EXIF names a camera body ({make}) with lens data and a shutter, aperture, and ISO triple"
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

/// The lowercased bytes to match markers against: the raw region plus the
/// decoded content of any variation-selector run, so a manifest ridden in a
/// C2PA text wrapper reads the same as binary JUMBF.
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

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() + from {
        return None;
    }
    (from..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_capture_claim_certifies_and_a_later_generative_action_does_not() {
        let cert = manifest_haystack(
            b"c2pa.created digitalSourceType=http://cv.iptc.org/newscodes/digitalsourcetype/digitalCapture signer=Leica Camera AG",
        );
        let r = read_manifest(&cert).unwrap();
        assert_eq!(r.status, CaptureStatus::Certified);
        assert!(r.claim.unwrap().contains("digitalCapture"));
        let gen = manifest_haystack(
            b"c2pa.created digitalCapture ... c2pa.edited softwareAgent=Adobe Firefly trainedAlgorithmicMedia",
        );
        let r = read_manifest(&gen).unwrap();
        assert_eq!(r.status, CaptureStatus::Generative);
        let pub_only = manifest_haystack(b"claim_generator=Reuters Publisher c2pa.published");
        assert!(read_manifest(&pub_only).is_none());
        let gen_only =
            manifest_haystack(b"claim_generator=ComfyUI c2pa.created trainedAlgorithmicMedia");
        assert!(
            read_manifest(&gen_only).is_none(),
            "no capture claim at all"
        );
    }
}
