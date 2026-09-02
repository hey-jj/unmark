//! ID3 rewriter for standalone audio. It removes a leading ID3v2 tag and a
//! trailing ID3v1 tag and copies the audio frames unchanged.

use super::DropSpec;
use crate::detect::id3::{has_v1, v2_len};

/// The byte range of the audio frames, excluding a leading ID3v2 tag and a
/// trailing ID3v1 tag.
fn frame_range(bytes: &[u8]) -> (usize, usize) {
    let start = v2_len(bytes).unwrap_or(0).min(bytes.len());
    let end = if has_v1(bytes) {
        bytes.len() - 128
    } else {
        bytes.len()
    };
    (start, end.max(start))
}

pub fn rewrite(bytes: &[u8], spec: &DropSpec) -> Vec<u8> {
    if !spec.id3 {
        return bytes.to_vec();
    }
    let (start, end) = frame_range(bytes);
    bytes[start..end].to_vec()
}

/// The audio frames. These must not change across a tag strip.
pub fn signal_stream(bytes: &[u8]) -> Vec<u8> {
    let (start, end) = frame_range(bytes);
    bytes[start..end].to_vec()
}
