//! Invisible-Unicode detection for text files. It reports zero-width and
//! bidirectional controls and unusual spaces, and classifies variation-selector
//! runs so a C2PA text wrapper is never deleted as hygiene.

use super::c2pa::{self, RunClass};
use crate::scan::{Detection, Honesty, Location, ScanState};

/// True when a code point is a variation selector.
pub fn is_variation_selector(c: char) -> bool {
    let u = c as u32;
    (0xFE00..=0xFE0F).contains(&u) || (0xE0100..=0xE01EF).contains(&u)
}

/// True when a code point is invisible hygiene: zero-width, a bidi control, or
/// an unusual space. Variation selectors are handled separately by run.
pub fn is_hygiene_invisible(c: char) -> bool {
    let u = c as u32;
    matches!(u,
        0x200B..=0x200F // zero-width space, joiners, marks
        | 0x2060..=0x2064 // word joiner, invisible operators
        | 0xFEFF // zero-width no-break space
        | 0x202A..=0x202E // bidi embeddings and overrides
        | 0x2066..=0x2069 // bidi isolates
        | 0x00AD // soft hyphen
        | 0x2000..=0x200A // en/em spaces and friends
        | 0x202F // narrow no-break space
        | 0x205F // medium mathematical space
        | 0x3000 // ideographic space
        | 0x1680 // ogham space mark
    )
}

/// A parsed span of invisible content.
#[derive(Clone, Debug)]
pub struct Span {
    pub byte_start: usize,
    pub byte_end: usize,
    pub kind: SpanKind,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpanKind {
    /// Removable invisible hygiene.
    Hygiene,
    /// A variation-selector run carrying a C2PA text wrapper.
    C2paWrapper,
    /// A wrapper-shaped variation-selector run that would not parse.
    Malformed,
}

/// Find every invisible span in the text, classifying variation-selector runs.
/// This is the shared read that both detection and the MC07 transform use.
pub fn spans(text: &str) -> Vec<Span> {
    let mut out = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((idx, c)) = chars.next() {
        if is_variation_selector(c) {
            // Gather the whole run.
            let start = idx;
            let mut run = vec![c];
            let mut end = idx + c.len_utf8();
            while let Some(&(_, nc)) = chars.peek() {
                if is_variation_selector(nc) {
                    run.push(nc);
                    end += nc.len_utf8();
                    chars.next();
                } else {
                    break;
                }
            }
            let kind = match c2pa::classify_run(&run) {
                RunClass::C2paWrapper => SpanKind::C2paWrapper,
                RunClass::Malformed => SpanKind::Malformed,
                RunClass::Hygiene => SpanKind::Hygiene,
            };
            out.push(Span {
                byte_start: start,
                byte_end: end,
                kind,
            });
        } else if is_hygiene_invisible(c) {
            out.push(Span {
                byte_start: idx,
                byte_end: idx + c.len_utf8(),
                kind: SpanKind::Hygiene,
            });
        }
    }
    out
}

pub fn scan(bytes: &[u8]) -> (Vec<Detection>, bool) {
    // A text file is read exhaustively by code point, so the scan is complete.
    // Invalid UTF-8 is not a text container this build scans.
    let Ok(text) = std::str::from_utf8(bytes) else {
        return (Vec::new(), false);
    };
    let spans = spans(text);
    if spans.is_empty() {
        return (Vec::new(), true);
    }

    let mut invis = Detection::present("invisibles", "invisible Unicode", Honesty::Confirmable);
    let mut c2pa_det = Detection::present("c2pa", "C2PA manifest", Honesty::Confirmable);
    let mut hygiene = 0usize;
    let mut malformed = 0usize;

    for s in &spans {
        match s.kind {
            SpanKind::Hygiene => {
                hygiene += 1;
                invis.locations.push(Location {
                    container: "text".to_string(),
                    offset: s.byte_start,
                    length: s.byte_end - s.byte_start,
                    detail: "invisible character".to_string(),
                });
            }
            SpanKind::C2paWrapper => {
                c2pa_det.locations.push(Location {
                    container: "text C2PA wrapper".to_string(),
                    offset: s.byte_start,
                    length: s.byte_end - s.byte_start,
                    detail: "variation-selector wrapper".to_string(),
                });
            }
            SpanKind::Malformed => {
                malformed += 1;
                invis.evidence.push(format!(
                    "wrapper-shaped variation-selector run at byte {} did not parse, left alone",
                    s.byte_start
                ));
            }
        }
    }

    let mut out = Vec::new();
    if !c2pa_det.locations.is_empty() {
        out.push(c2pa_det);
    }
    if hygiene > 0 {
        invis
            .evidence
            .insert(0, format!("{hygiene} invisible characters"));
        out.push(invis);
    } else if malformed > 0 {
        // Only unparseable wrapper-shaped runs. Presence is unknown, so the
        // class reports malformed and nothing is removed.
        invis.state = ScanState::Malformed;
        invis.locations.clear();
        out.push(invis);
    }
    (out, true)
}
