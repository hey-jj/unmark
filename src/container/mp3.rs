//! MPEG audio rewriter. It drops the leading ID3v2 tags with their padding,
//! the trailing ID3v1 tag, APE tags at either end, and the information
//! frame when the next audio frame draws no bits from the reservoir, and
//! copies every audio frame byte for byte.

use super::{DropSpec, RewriteError};
use crate::detect::mp3::layout;

pub fn rewrite(bytes: &[u8], spec: &DropSpec) -> Result<Vec<u8>, RewriteError> {
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

/// The audio frames after any information frame. These must not change
/// across a tag strip.
pub fn signal_stream(bytes: &[u8]) -> Vec<u8> {
    let l = layout(bytes);
    let start = match &l.info {
        Some(info) if info.offset == l.frames_start => info.offset + info.len,
        _ => l.frames_start,
    };
    bytes[start.min(l.frames_end)..l.frames_end].to_vec()
}
