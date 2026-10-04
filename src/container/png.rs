//! PNG rewriter. It copies every kept chunk verbatim, including its CRC, so the
//! IDAT stream is byte-identical and no CRC is ever recomputed.

use super::{DropSpec, RewriteError};
use crate::detect::png_text::{chunks, SIGNATURE};

/// Keep these ancillary chunks regardless of MC06 because rendering needs
/// their color, gamma, chromaticity, physical dimensions, and background data.
const KEEP_ANCILLARY: &[&[u8; 4]] = &[
    b"iCCP", b"gAMA", b"cHRM", b"sRGB", b"sBIT", b"pHYs", b"bKGD", b"tRNS", b"PLTE", b"acTL",
    b"fcTL", b"fdAT", b"cICP", b"mDCv", b"cLLi",
];

fn is_critical(kind: &[u8; 4]) -> bool {
    matches!(kind, b"IHDR" | b"IDAT" | b"IEND" | b"PLTE")
}

fn should_drop(kind: &[u8; 4], spec: &DropSpec, is_xmp: bool) -> bool {
    match kind {
        b"tEXt" | b"zTXt" => spec.png_text,
        b"iTXt" => (is_xmp && spec.xmp) || (!is_xmp && spec.png_text),
        b"eXIf" => spec.exif,
        b"caBX" => spec.c2pa,
        k if is_critical(k) => false,
        k if KEEP_ANCILLARY.contains(&k) => false,
        _ => spec.unlisted,
    }
}

pub fn rewrite(bytes: &[u8], spec: &DropSpec) -> Result<Vec<u8>, RewriteError> {
    if bytes.len() < 8 || bytes[..8] != SIGNATURE {
        return Err(RewriteError::Unsupported("not a PNG".to_string()));
    }
    let (chunks, complete) = chunks(bytes);
    // A rewrite over a walk that did not reach IEND would silently truncate the
    // file at the corrupt chunk, dropping every chunk past it. Refuse and write
    // nothing instead. This is the design's named worst failure.
    if !complete {
        return Err(RewriteError::Malformed(
            "the PNG chunk stream did not reach IEND cleanly".to_string(),
        ));
    }
    let mut out = Vec::with_capacity(bytes.len());
    out.extend_from_slice(&SIGNATURE);
    for c in chunks {
        let is_xmp = c.kind == *b"iTXt"
            && String::from_utf8_lossy(c.data).starts_with(crate::detect::xmp::XMP_PNG_KEYWORD);
        if should_drop(&c.kind, spec, is_xmp) {
            continue;
        }
        // Copy the whole chunk verbatim: length, type, data, and CRC.
        out.extend_from_slice(&bytes[c.start..c.start + c.total]);
    }
    Ok(out)
}

/// The concatenated IDAT and fdAT payloads. This must not change across a
/// metadata rewrite.
pub fn signal_stream(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let (chunks, _) = chunks(bytes);
    for c in chunks {
        if c.kind == *b"IDAT" || c.kind == *b"fdAT" {
            out.extend_from_slice(c.data);
        }
    }
    out
}
