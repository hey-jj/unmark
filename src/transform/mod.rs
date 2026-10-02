//! The transform layer. The metadata transforms turn an ordered plan into a
//! container drop spec or a text edit and apply it without decoding a pixel
//! or a sample. The pixel and audio transforms live in the sibling modules
//! `pixel` and `audio`. A container that cannot be rewritten safely returns
//! an error so the caller fails closed.

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

/// The confirmable mark classes a transform strips.
pub fn targeted_classes(id: &str) -> &'static [&'static str] {
    match id {
        "MC01" => &["c2pa"],
        "MC02" => &["xmp", "iptc"],
        "MC03" => &["exif"],
        "MC04" => &["png_text"],
        "MC05" => &["id3", "ilst"],
        "MC06" => &[],
        "MC07" => &["invisibles"],
        "MC08" => &[],
        "MC09" => &["riff_ancillary", "id3"],
        "MC10" => &["vorbis"],
        "MC11" => &[],
        "MC12" => &[],
        "MC13" => &["ape"],
        "MC14" => &["mp3_info"],
        "MC15" => &["mp3_ancillary"],
        "PX02" => &["dwtdct"],
        _ => &[],
    }
}

/// Build a container drop spec from the transform ids in a plan.
pub fn drop_spec(transforms: &[String]) -> DropSpec {
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
            "MC10" => s.vorbis = true,
            "MC11" => s.jpeg_segments = true,
            "MC12" => s.riff_production = true,
            "MC13" => s.ape = true,
            "MC14" => s.info_identity = true,
            "MC15" => s.ancillary = true,
            _ => {}
        }
    }
    s
}

/// Apply the metadata transforms of a plan to the asset. Pure: a function of
/// the bytes and the plan.
pub fn apply(bytes: &[u8], format: Format, transforms: &[String]) -> Result<Applied, RewriteError> {
    if format.is_text() {
        return Ok(apply_text(bytes, transforms));
    }
    let spec = drop_spec(transforms);
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

/// Remove generator elements from markup and generator banner lines from
/// text. A `<meta>` whose name is generator, in either quoting, and an HTML
/// comment naming a generator are cut as spans, so the content around them
/// on the same line stays. A line is dropped whole only when it is a comment
/// or banner that names a generator.
fn remove_matching_lines(text: &str) -> String {
    let s = remove_generator_elements(text);
    remove_generator_lines(&s)
}

/// The value of an attribute in a tag body, with either quoting or none.
fn attribute_value<'a>(tag_lower: &'a str, name: &str) -> Option<&'a str> {
    let mut from = 0;
    while let Some(at) = tag_lower[from..].find(name) {
        let start = from + at;
        let before_ok = start == 0 || tag_lower.as_bytes()[start - 1].is_ascii_whitespace();
        let rest = &tag_lower[start + name.len()..];
        let rest_trim = rest.trim_start();
        if before_ok && rest_trim.starts_with('=') {
            let v = rest_trim[1..].trim_start();
            let value = if let Some(q) = v.strip_prefix('"') {
                q.split('"').next().unwrap_or("")
            } else if let Some(q) = v.strip_prefix('\'') {
                q.split('\'').next().unwrap_or("")
            } else {
                v.split(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')
                    .next()
                    .unwrap_or("")
            };
            return Some(value);
        }
        from = start + name.len();
    }
    None
}

fn remove_generator_elements(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut p = 0;
    while p < text.len() {
        let rest = &lower[p..];
        let mut cut = None;
        if rest.starts_with("<meta") {
            if let Some(close) = rest.find('>') {
                let tag = &rest[..=close];
                if attribute_value(tag, "name").is_some_and(|n| n.trim() == "generator") {
                    cut = Some(close + 1);
                }
            }
        } else if rest.starts_with("<!--") {
            if let Some(close) = rest.find("-->") {
                let comment = &rest[..close + 3];
                if comment.contains("generator") || comment.contains("created with") {
                    cut = Some(close + 3);
                }
            }
        }
        if let Some(len) = cut {
            p += len;
            // An element that stood alone on its line takes the line with
            // it; one inside a line leaves its neighbours in place.
            let line_start = out.rfind('\n').map(|i| i + 1).unwrap_or(0);
            let after = &text[p..];
            let nl = after.find('\n');
            let tail = &after[..nl.unwrap_or(after.len())];
            if out[line_start..].trim().is_empty() && tail.trim().is_empty() {
                out.truncate(line_start);
                p += nl.map(|i| i + 1).unwrap_or(after.len());
            }
            continue;
        }
        let ch = text[p..].chars().next().unwrap_or('\0');
        out.push(ch);
        p += ch.len_utf8().max(1);
    }
    out
}

/// Comment markers that open a banner line in text, scripts, and configs.
const COMMENT_STARTS: &[&str] = &["#", "//", ";", "--", "%", "rem ", "'"];

fn remove_generator_lines(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let lower = trimmed.to_ascii_lowercase();
        let names_generator = lower.contains("generator:") || lower.contains("x-generator:");
        let banner = COMMENT_STARTS.iter().any(|m| lower.starts_with(m))
            || lower.starts_with("generator:")
            || lower.starts_with("x-generator:");
        if names_generator && banner {
            continue;
        }
        out.push_str(line);
    }
    out
}
