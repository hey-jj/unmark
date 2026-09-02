//! RIFF rewriter for WAV, WebP, and AVI. Dropping a chunk rebuilds the RIFF
//! size field and honors even-boundary padding, or a decoder rejects the file.
//! This is the one metadata-path spot where a bug destroys the asset, so the
//! rewrite is deliberate about both.

use super::{DropSpec, RewriteError};
use crate::detect::c2pa;
use crate::detect::riff::chunks;

/// Image and audio payload chunks kept regardless of MC06.
const KEEP: &[&[u8; 4]] = &[
    b"fmt ", b"data", b"fact", b"cue ", b"smpl", b"inst", // WAV core
    b"bext", b"iXML", b"aXML", b"_PMX", // production metadata on the allowlist
    b"VP8 ", b"VP8L", b"VP8X", b"ALPH", b"ANIM", b"ANMF", b"ICCP", // WebP
    b"movi", b"hdrl", b"idx1", b"strh", b"strf", // AVI core
];

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
    for c in chunks {
        saw_chunk = true;
        if should_drop(&c.id, c.data, spec) {
            continue;
        }
        // Copy id, size, data, and the pad byte verbatim.
        body.extend_from_slice(&bytes[c.start..c.start + c.total]);
    }
    if !saw_chunk {
        return Err(RewriteError::Malformed("no RIFF chunks parsed".to_string()));
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
