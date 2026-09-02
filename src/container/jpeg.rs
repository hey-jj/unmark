//! JPEG rewriter. It copies the start-of-image marker, every kept marker
//! segment verbatim, and the entire entropy-coded scan from the first SOS to
//! the end unchanged.

use super::DropSpec;
use crate::detect::jpeg::{segments, EXIF_ID, IPTC_ID};
use crate::detect::{c2pa, xmp};

fn should_drop(marker: u8, data: &[u8], spec: &DropSpec) -> bool {
    match marker {
        0xE1 => {
            (spec.exif && data.starts_with(EXIF_ID))
                || (spec.xmp && data.starts_with(xmp::XMP_APP1_ID))
        }
        0xEB => spec.c2pa && c2pa::looks_like_c2pa(data),
        0xED => spec.iptc && data.starts_with(IPTC_ID),
        _ => false,
    }
}

pub fn rewrite(bytes: &[u8], spec: &DropSpec) -> Vec<u8> {
    if bytes.len() < 2 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return bytes.to_vec();
    }
    let mut out = Vec::with_capacity(bytes.len());
    out.extend_from_slice(&[0xFF, 0xD8]);
    // JPEG is safe by construction: everything after the last parsed segment is
    // copied verbatim to the end, so an incomplete walk never drops bytes.
    let (segs, _complete) = segments(bytes);
    for s in &segs {
        if should_drop(s.marker, s.data, spec) {
            continue;
        }
        out.extend_from_slice(&bytes[s.start..s.start + s.total]);
    }
    // Everything after the last parsed segment is the scan and the trailer,
    // copied whole so the entropy-coded data is byte-identical.
    let after = segs.last().map(|s| s.start + s.total).unwrap_or(2);
    out.extend_from_slice(&bytes[after..]);
    out
}

/// The entropy-coded scan: from the first SOS marker to the end. This is the
/// pixel signal and must not change across a metadata rewrite.
pub fn signal_stream(bytes: &[u8]) -> Vec<u8> {
    let mut p = 2;
    while p + 4 <= bytes.len() {
        if bytes[p] != 0xFF {
            break;
        }
        let marker = bytes[p + 1];
        if marker == 0xDA {
            return bytes[p..].to_vec();
        }
        if (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
            p += 2;
            continue;
        }
        let Some(len) = crate::detect::be_u16(bytes, p + 2) else {
            break;
        };
        p += 2 + len as usize;
    }
    Vec::new()
}
