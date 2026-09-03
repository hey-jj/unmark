//! FLAC rewriter. It removes the Vorbis comment block whole, the default in
//! this release with --keep vorbis as the opt-out, and rebuilds the last-block
//! flag so the metadata chain stays valid. The audio
//! frames and the kept blocks are copied verbatim. A walk that never reached
//! the last-block flag cannot place the audio frames, so the rewriter refuses
//! rather than copy an unknown layout.

use super::{DropSpec, RewriteError};
use crate::detect::flac::{blocks, VORBIS_COMMENT};

pub fn rewrite(bytes: &[u8], spec: &DropSpec) -> Result<Vec<u8>, RewriteError> {
    let (blocks, audio_start, complete) = blocks(bytes);
    if !complete {
        return Err(RewriteError::Malformed(
            "the FLAC metadata chain did not reach its last block".to_string(),
        ));
    }
    if !(spec.vorbis || spec.unlisted) || blocks.is_empty() {
        return Ok(bytes.to_vec());
    }
    let mut kept: Vec<&crate::detect::flac::Block> =
        blocks.iter().filter(|b| b.kind != VORBIS_COMMENT).collect();
    if kept.len() == blocks.len() {
        return Ok(bytes.to_vec()); // nothing to remove
    }
    let mut out = Vec::with_capacity(bytes.len());
    out.extend_from_slice(b"fLaC");
    let last_idx = kept.len().saturating_sub(1);
    for (i, b) in kept.iter_mut().enumerate() {
        let block = &bytes[b.start..b.start + b.total];
        out.extend_from_slice(block);
        // Rebuild the last-block flag on the header byte at this block's start.
        let header_at = out.len() - b.total;
        if i == last_idx {
            out[header_at] |= 0x80;
        } else {
            out[header_at] &= 0x7F;
        }
    }
    // Copy the audio frames verbatim.
    out.extend_from_slice(&bytes[audio_start..]);
    Ok(out)
}

/// The audio frames after the metadata blocks. These must not change.
pub fn signal_stream(bytes: &[u8]) -> Vec<u8> {
    let (_, audio_start, _) = blocks(bytes);
    bytes[audio_start..].to_vec()
}
