//! The metadata transform layer. It turns an ordered plan into a container drop
//! spec or a text edit and applies it. It never re-encodes a pixel or a sample.
//! A container that cannot be rewritten safely returns an error so the caller
//! fails closed. The pixel and audio degrade transforms live in the sibling
//! modules `pixel` and `audio`; this module never decodes.

#[cfg(feature = "audio")]
pub mod audio;
#[cfg(feature = "image")]
pub mod pixel;

use crate::asset::Format;
use crate::container::{self, DropSpec, RewriteError};
use crate::detect::invisibles::{spans, SpanKind};

/// The result of applying a plan.
pub struct Applied {
    pub bytes: Vec<u8>,
}

/// Build a container drop spec from the transform ids in a plan.
fn drop_spec(transforms: &[String]) -> DropSpec {
    let mut s = DropSpec::default();
    for t in transforms {
        match t.as_str() {
            "MC01" => s.c2pa = true,
            "MC02" => {
                s.xmp = true;
                s.iptc = true;
            }
            "MC03" => s.exif = true,
            "MC04" => s.png_text = true,
            "MC05" => {
                s.id3 = true;
                s.ilst = true;
            }
            "MC06" => s.unlisted = true,
            "MC09" => {
                s.riff_ancillary = true;
                s.id3 = true;
            }
            _ => {}
        }
    }
    s
}

/// Apply the plan's transforms to the asset. Pure: a function of the bytes and
/// the plan.
pub fn apply(bytes: &[u8], format: Format, transforms: &[String]) -> Result<Applied, RewriteError> {
    if format.is_text() {
        return Ok(apply_text(bytes, transforms));
    }
    let spec = drop_spec(transforms);
    // A Declined error propagates so the caller fails the run closed rather
    // than claiming a removal it did not perform.
    let out = container::rewrite(bytes, format, &spec)?;
    Ok(Applied { bytes: out })
}

/// Text transforms MC07 and MC08. MC07 removes invisible hygiene and routes a
/// C2PA text wrapper to the manifest path, which here means removing it only
/// when the manifest strip MC01 is in the plan. A malformed wrapper is left
/// alone. MC08 removes generator identity headers.
fn apply_text(bytes: &[u8], transforms: &[String]) -> Applied {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Applied {
            bytes: bytes.to_vec(),
        };
    };
    let strip_invis = transforms.iter().any(|t| t == "MC07");
    let strip_wrapper = transforms.iter().any(|t| t == "MC01");
    let strip_headers = transforms.iter().any(|t| t == "MC08");

    let mut result = text.to_string();
    if strip_invis || strip_wrapper {
        result = remove_spans(&result, strip_invis, strip_wrapper);
    }
    if strip_headers {
        result = strip_generator_headers(&result);
    }
    Applied {
        bytes: result.into_bytes(),
    }
}

/// Remove invisible spans. Hygiene spans go when `hygiene` is set, C2PA wrapper
/// spans go when `wrapper` is set, and malformed spans are always kept.
fn remove_spans(text: &str, hygiene: bool, wrapper: bool) -> String {
    let spans = spans(text);
    let mut drop: Vec<(usize, usize)> = Vec::new();
    for s in spans {
        let take = match s.kind {
            SpanKind::Hygiene => hygiene,
            SpanKind::C2paWrapper => wrapper,
            SpanKind::Malformed => false,
        };
        if take {
            drop.push((s.byte_start, s.byte_end));
        }
    }
    if drop.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for (start, end) in drop {
        if start >= last {
            out.push_str(&text[last..start]);
            last = end;
        }
    }
    out.push_str(&text[last..]);
    out
}

/// Remove generator identity headers from text, SVG, and HTML. The shapes are a
/// closed list read from a keep-nothing predicate over lines and elements.
pub fn strip_generator_headers(text: &str) -> String {
    let mut s = remove_svg_metadata(text);
    s = remove_matching_lines(&s);
    s
}

fn remove_svg_metadata(text: &str) -> String {
    // Remove a <metadata>...</metadata> element, which SVG editors fill with
    // generator and provenance data.
    let mut out = text.to_string();
    while let (Some(open), Some(close)) = (out.find("<metadata"), out.find("</metadata>")) {
        if open < close {
            let end = close + "</metadata>".len();
            out.replace_range(open..end, "");
        } else {
            break;
        }
    }
    out
}

fn remove_matching_lines(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let lower = line.to_ascii_lowercase();
        let is_generator_meta = lower.contains("<meta") && lower.contains("name=\"generator\"");
        let is_generator_comment = lower.contains("generator:") || lower.contains("x-generator:");
        let is_created_with = lower.contains("<!--") && lower.contains("created with");
        if is_generator_meta || is_generator_comment || is_created_with {
            continue;
        }
        out.push_str(line);
    }
    out
}
