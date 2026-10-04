//! FLAC metadata block walk. It reports a Vorbis comment block. Vorbis comments
//! routinely hold legitimate credits and chapter data, so they sit on the
//! preservation allowlist and come off only under an explicit opt-in.

use crate::scan::{Detection, Honesty, Location, ScanState};

pub const VORBIS_COMMENT: u8 = 4;
pub const STREAMINFO: u8 = 0;
pub const PICTURE: u8 = 6;

/// One FLAC metadata block.
pub struct Block {
    pub last: bool,
    pub kind: u8,
    /// Byte offset of the block header.
    pub start: usize,
    /// Total bytes including the four-byte header.
    pub total: usize,
}

/// Walk the metadata blocks after the `fLaC` marker. Returns the blocks, the
/// byte offset where the audio frames begin, and a completeness flag.
/// `complete` is true when the walk reached the block whose last-block flag is
/// set. A block length that overruns the buffer stops the walk with `complete`
/// false. Handles any input without panicking.
pub fn blocks(bytes: &[u8]) -> (Vec<Block>, usize, bool) {
    let mut out = Vec::new();
    if bytes.len() < 4 || &bytes[..4] != b"fLaC" {
        return (out, bytes.len(), false);
    }
    let mut p = 4;
    let mut complete = false;
    while let Some(header) = bytes.get(p..p + 4) {
        let last = header[0] & 0x80 != 0;
        let kind = header[0] & 0x7F;
        let len = ((header[1] as usize) << 16) | ((header[2] as usize) << 8) | header[3] as usize;
        let total = 4 + len;
        if p + total > bytes.len() {
            break;
        }
        out.push(Block {
            last,
            kind,
            start: p,
            total,
        });
        p += total;
        if last {
            complete = true;
            break;
        }
    }
    (out, p, complete)
}

pub fn scan(bytes: &[u8]) -> (Vec<Detection>, bool) {
    let mut det = Detection::present("vorbis", "Vorbis comment", Honesty::Confirmable);
    let (blocks, _, complete) = blocks(bytes);
    for b in blocks {
        if b.kind == VORBIS_COMMENT {
            det.locations.push(Location {
                container: "FLAC VORBIS_COMMENT".to_string(),
                offset: b.start,
                length: b.total,
                detail: String::new(),
            });
        }
    }
    let found = if det.state == ScanState::ConfirmedPresent && !det.locations.is_empty() {
        vec![det]
    } else {
        Vec::new()
    };
    (found, complete)
}
