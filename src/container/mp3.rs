//! MPEG audio rewriter. It drops the leading ID3v2 tags with their padding,
//! the trailing ID3v1 tag, and APE tags at either end, rewrites the
//! information frame in place as its minimal form, zeroes the ancillary
//! bytes, and copies every audio frame's referenced bytes as they are.

use super::{DropSpec, RewriteError};
use crate::detect::mp3::{ancillary_ranges, layout, InfoFrame, EXT_DELAY_PADDING, EXT_LEN};

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

/// Rewrite an information frame in place as its minimal form: the frame
/// count, byte count, and TOC it had, the delay and padding of its
/// extension, and zero everywhere else, the encoder identity included.
/// Same size, byte-for-byte deterministic.
pub fn minimal_info_frame(frame: &mut [u8], info: &InfoFrame) {
    if info.kind == "VBRI" {
        // VBRI carries a version and encoder-specific fields after its
        // table; the 26-byte head (version, delay, quality, bytes, frames,
        // table layout) stays and the rest is zeroed.
        let keep = (info.tag_at + 26).min(frame.len());
        for b in frame[keep..].iter_mut() {
            *b = 0;
        }
        return;
    }
    let mut at = info.tag_at + 8;
    if info.flags & 1 != 0 {
        at += 4;
    }
    if info.flags & 2 != 0 {
        at += 4;
    }
    if info.flags & 4 != 0 {
        at += 100;
    }
    let n = frame.len();
    if info.flags & 8 != 0 {
        // The quality indicator is an encoder setting, zeroed in place.
        for b in frame[at.min(n)..(at + 4).min(n)].iter_mut() {
            *b = 0;
        }
        at += 4;
    }
    let delay_padding: Option<[u8; 3]> = info.extension_at.and_then(|e| {
        frame
            .get(e + EXT_DELAY_PADDING.start..e + EXT_DELAY_PADDING.end)
            .map(|t| [t[0], t[1], t[2]])
    });
    for b in frame[at.min(n)..].iter_mut() {
        *b = 0;
    }
    if let (Some(e), Some(t)) = (info.extension_at, delay_padding) {
        frame[e + EXT_DELAY_PADDING.start..e + EXT_DELAY_PADDING.end].copy_from_slice(&t);
    }
}

/// Build the minimal information frame the re-encode writes: the given
/// header and side-information size, an Info tag with the frame count and
/// byte count, the extension carrying only the delay and padding, zero
/// elsewhere, padded to the frame length.
pub fn build_info_frame(
    header: &[u8; 4],
    crc: bool,
    side_info: usize,
    frame_len: usize,
    counts: (u32, u32),
    delay_padding: (u16, u16),
) -> Vec<u8> {
    let (frames, bytes) = counts;
    let (delay, padding) = delay_padding;
    let mut out = header.to_vec();
    if crc {
        out.extend_from_slice(&[0, 0]);
    }
    out.resize(out.len() + side_info, 0);
    out.extend_from_slice(b"Info");
    out.extend_from_slice(&3u32.to_be_bytes());
    out.extend_from_slice(&frames.to_be_bytes());
    out.extend_from_slice(&bytes.to_be_bytes());
    let ext_at = out.len();
    out.resize(ext_at + EXT_LEN, 0);
    let d = delay & 0x0FFF;
    let p = padding & 0x0FFF;
    out[ext_at + 21] = (d >> 4) as u8;
    out[ext_at + 22] = (((d & 0x0F) << 4) | (p >> 8)) as u8;
    out[ext_at + 23] = (p & 0xFF) as u8;
    out.resize(frame_len.max(out.len()), 0);
    out
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
    let frames_at = out.len();
    out.extend_from_slice(&bytes[l.frames_start..l.frames_end]);
    if let Some(info) = &l.info {
        if spec.info_identity && info.offset == l.frames_start {
            let frame = &mut out[frames_at..frames_at + info.len];
            minimal_info_frame(frame, info);
        }
    }
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
