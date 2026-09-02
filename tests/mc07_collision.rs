//! The MC07 and MC01 collision. A C2PA text wrapper built from variation
//! selectors must never be deleted as invisible hygiene, because that would
//! remove a manifest silently and bypass the G2 refusal.

use unmark::detect;
use unmark::detect::invisibles::{spans, SpanKind};
use unmark::scan::ScanState;
use unmark::transform;

/// Encode bytes as a variation-selector run, the scheme a C2PA text wrapper
/// uses to ride inside plain text.
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
fn a_c2pa_wrapper_is_classified_and_not_hygiene() {
    let text = format!("caption{}\n", encode_vs(b"jumb c2pa manifest"));
    let spans = spans(&text);
    assert!(
        spans.iter().any(|s| s.kind == SpanKind::C2paWrapper),
        "the wrapper run was not classified as C2PA"
    );
    assert!(
        !spans.iter().any(|s| s.kind == SpanKind::Hygiene),
        "the wrapper run was misread as hygiene"
    );
    // Detection routes it to the C2PA class.
    let det = detect::inspect(text.as_bytes());
    assert_eq!(
        det.get("c2pa").map(|d| d.state),
        Some(ScanState::ConfirmedPresent)
    );
}

#[test]
fn mc07_alone_leaves_a_wrapper_alone() {
    let wrapper = encode_vs(b"jumb c2pa manifest");
    let text = format!("caption{wrapper}\n");
    // MC07 without MC01: hygiene only, the wrapper is routed to the manifest
    // path which is not in this plan, so it is left in place.
    let applied = transform::apply(
        text.as_bytes(),
        unmark::asset::Format::Text,
        &["MC07".to_string()],
    );
    let out = String::from_utf8(applied.unwrap().bytes).unwrap();
    assert!(out.contains(&wrapper), "MC07 alone deleted a C2PA wrapper");
}

#[test]
fn mc01_removes_the_wrapper() {
    let wrapper = encode_vs(b"jumb c2pa manifest");
    let text = format!("caption{wrapper}\n");
    let applied = transform::apply(
        text.as_bytes(),
        unmark::asset::Format::Text,
        &["MC01".to_string(), "MC07".to_string()],
    );
    let out = String::from_utf8(applied.unwrap().bytes).unwrap();
    assert!(!out.contains(&wrapper), "MC01 did not remove the wrapper");
    assert!(out.contains("caption"));
}

#[test]
fn hygiene_invisibles_are_removed_by_mc07() {
    let text = "a\u{200B}b\u{202E}c\n"; // zero-width space and a bidi override
    let applied = transform::apply(
        text.as_bytes(),
        unmark::asset::Format::Text,
        &["MC07".to_string()],
    );
    let out = String::from_utf8(applied.unwrap().bytes).unwrap();
    assert_eq!(out, "abc\n");
}

#[test]
fn a_malformed_wrapper_run_is_left_alone() {
    // A long variation-selector run that does not decode to a C2PA marker.
    let run: String = "\u{FE0F}".repeat(12);
    let text = format!("x{run}y\n");
    let applied = transform::apply(
        text.as_bytes(),
        unmark::asset::Format::Text,
        &["MC07".to_string()],
    );
    let out = String::from_utf8(applied.unwrap().bytes).unwrap();
    assert!(
        out.contains(&run),
        "a malformed wrapper-shaped run was deleted"
    );
}
