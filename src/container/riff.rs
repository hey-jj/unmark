//! RIFF rewriter for WAV, WebP, and AVI. Dropping a chunk rebuilds the RIFF
//! size field and honors even-boundary padding, or a decoder rejects the file.
//! This is the one metadata-path spot where a bug destroys the asset, so the
//! rewrite is deliberate about both.

use super::{DropSpec, RewriteError};
use crate::detect::c2pa;
use crate::detect::riff::chunks;

/// The chunks decoding and valid output require, kept regardless of MC06.
/// Everything else is an unlisted chunk and goes under the default run.
const KEEP: &[&[u8; 4]] = &[
    b"fmt ", b"data", b"fact", // WAV core
    b"VP8 ", b"VP8L", b"VP8X", b"ALPH", b"ANIM", b"ANMF", b"ICCP", // WebP
    b"movi", b"hdrl", b"idx1", b"strh", b"strf", // AVI core
];

/// VP8X flag bits that advertise a chunk.
const VP8X_ICC: u8 = 0x20;
const VP8X_EXIF: u8 = 0x08;
const VP8X_XMP: u8 = 0x04;

fn should_drop(id: &[u8; 4], data: &[u8], spec: &DropSpec) -> bool {
    match id {
        b"C2PA" => spec.c2pa,
        b"id3 " => spec.id3,
        b"XMP " => spec.xmp,
        b"EXIF" => spec.exif,
        b"LIST" => spec.riff_ancillary && data.get(0..4) == Some(b"INFO"),
        k if KEEP.contains(&k) => false,
        _ => {
            // A stray C2PA payload in an unusual chunk still comes off when the
            // manifest strip is requested.
            (spec.c2pa && c2pa::looks_like_c2pa(data)) || spec.unlisted
        }
    }
}

pub fn rewrite(bytes: &[u8], spec: &DropSpec) -> Result<Vec<u8>, RewriteError> {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" {
        return Err(RewriteError::Unsupported(
            "not a RIFF container".to_string(),
        ));
    }
    let form = &bytes[8..12];
    let (chunks, complete) = chunks(bytes);
    // A rewrite over a walk that overran a chunk size would truncate the file at
    // the corrupt chunk and rebuild a RIFF size around the surviving prefix,
    // producing a smaller file that reads as valid. Refuse and write nothing.
    if !complete {
        return Err(RewriteError::Malformed(
            "the RIFF chunk stream did not parse to its declared end".to_string(),
        ));
    }
    let mut body: Vec<u8> = Vec::with_capacity(bytes.len());
    body.extend_from_slice(form);
    let mut saw_chunk = false;
    let mut vp8x_flags_at: Option<usize> = None;
    let (mut has_icc, mut has_exif, mut has_xmp) = (false, false, false);
    for c in chunks {
        saw_chunk = true;
        if should_drop(&c.id, c.data, spec) {
            continue;
        }
        match &c.id {
            b"VP8X" => vp8x_flags_at = Some(body.len() + 8),
            b"ICCP" => has_icc = true,
            b"EXIF" => has_exif = true,
            b"XMP " => has_xmp = true,
            _ => {}
        }
        // Copy id, size, data, and the pad byte verbatim.
        body.extend_from_slice(&bytes[c.start..c.start + c.total]);
    }
    if !saw_chunk {
        return Err(RewriteError::Malformed("no RIFF chunks parsed".to_string()));
    }
    // A WebP VP8X header advertises its metadata chunks by flag, and a
    // decoder refuses a file whose advertised chunk is missing, so a flag
    // whose chunk was dropped is cleared.
    if form == b"WEBP" {
        if let Some(at) = vp8x_flags_at {
            if let Some(flags) = body.get_mut(at) {
                if !has_icc {
                    *flags &= !VP8X_ICC;
                }
                if !has_exif {
                    *flags &= !VP8X_EXIF;
                }
                if !has_xmp {
                    *flags &= !VP8X_XMP;
                }
            }
        }
    }
    let mut out = Vec::with_capacity(body.len() + 8);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

/// The image or audio payload chunks. For WAV this is the `data` chunk, for
/// WebP the bitstream chunks. These must not change across a metadata rewrite.
pub fn signal_stream(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let (chunks, _) = chunks(bytes);
    for c in chunks {
        if matches!(
            &c.id,
            b"data" | b"VP8 " | b"VP8L" | b"VP8X" | b"ALPH" | b"ANMF" | b"movi"
        ) {
            out.extend_from_slice(c.data);
        }
    }
    out
}
