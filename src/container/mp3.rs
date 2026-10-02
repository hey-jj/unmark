//! MPEG audio rewriter. It drops the leading ID3v2 tags with their padding,
//! the trailing ID3v1 tag, APE tags at either end, and the information
//! frame when the next audio frame draws no bits from the reservoir, and
//! copies every audio frame byte for byte.

use super::{DropSpec, RewriteError};
use crate::detect::mp3::{ancillary_ranges, layout};

/// Zero the ancillary bytes of the audio frames: everything inside the
/// main-data regions that no frame's main data covers, the bit reservoir
/// honoured. The frames decode to the same samples, since a decoder reads
/// only the covered bits. The information frame is left to its own strip.
pub fn scrub_ancillary(bytes: &[u8]) -> Result<Vec<u8>, RewriteError> {
    let l = layout(bytes);
    let Some(ranges) = ancillary_ranges(bytes, &l) else {
        return Err(RewriteError::Malformed(
            "a frame's main data and ancillary bytes could not be separated".to_string(),
        ));
    };
    let mut out = bytes.to_vec();
    for (at, len) in ranges {
        for b in out[at..at + len].iter_mut() {
            *b = 0;
        }
    }
    Ok(out)
}

pub fn rewrite(input: &[u8], spec: &DropSpec) -> Result<Vec<u8>, RewriteError> {
    let scrubbed;
    let bytes: &[u8] = if spec.ancillary {
        scrubbed = scrub_ancillary(input)?;
        &scrubbed
    } else {
        input
    };
    let l = layout(bytes);
    if !l.complete {
        return Err(RewriteError::Malformed(
            "the MPEG frame walk did not reach the end of the audio".to_string(),
        ));
    }
    let mut out = Vec::with_capacity(bytes.len());
    // Leading tags, in file order.
    let mut p = 0usize;
    while p < l.frames_start {
        if let Some((_, len)) = l.id3v2.iter().find(|(at, _)| *at == p) {
            if !spec.id3 {
                out.extend_from_slice(&bytes[p..p + len]);
            }
            p += len;
        } else if let Some((_, len)) = l.ape.iter().find(|(at, _)| *at == p) {
            if !spec.ape {
                out.extend_from_slice(&bytes[p..p + len]);
            }
            p += len;
        } else {
            // Bytes the walk did not classify stay where they are.
            out.push(bytes[p]);
            p += 1;
        }
    }
    let mut frames = &bytes[l.frames_start..l.frames_end];
    if let Some(info) = &l.info {
        if spec.xing && info.droppable() && info.offset == l.frames_start {
            frames = &bytes[info.offset + info.len..l.frames_end];
        }
    }
    out.extend_from_slice(frames);
    // Trailing tags, in file order.
    let mut p = l.frames_end;
    while p < bytes.len() {
        if let Some((_, len)) = l.ape.iter().find(|(at, _)| *at == p) {
            if !spec.ape {
                out.extend_from_slice(&bytes[p..p + len]);
            }
            p += len;
        } else if l.id3v1 == Some(p) {
            if !spec.id3 {
                out.extend_from_slice(&bytes[p..p + 128]);
            }
            p += 128;
        } else {
            out.push(bytes[p]);
            p += 1;
        }
    }
    Ok(out)
}

/// The audio frames after any information frame, with their ancillary
/// bytes zeroed: the bytes a decoder reads. These must not change across a
/// tag strip or the ancillary scrub.
pub fn signal_stream(input: &[u8]) -> Vec<u8> {
    let scrubbed = scrub_ancillary(input).unwrap_or_else(|_| input.to_vec());
    let bytes = &scrubbed;
    let l = layout(bytes);
    let start = match &l.info {
        Some(info) if info.offset == l.frames_start => info.offset + info.len,
        _ => l.frames_start,
    };
    bytes[start.min(l.frames_end)..l.frames_end].to_vec()
}
