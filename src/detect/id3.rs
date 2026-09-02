//! ID3 tag detection for standalone audio. It reports a leading ID3v2 tag and
//! a trailing ID3v1 tag. A walker that reads only the front reports a clean
//! strip while the v1 tag is still there, so both ends are read.

use crate::scan::{Detection, Honesty, Location, ScanState};

/// The size of a synchsafe ID3v2 tag body from its four-byte header field.
pub fn synchsafe(b: &[u8]) -> Option<usize> {
    let s = b.get(0..4)?;
    // Each byte carries seven bits, high bit clear.
    if s.iter().any(|x| x & 0x80 != 0) {
        return None;
    }
    Some(((s[0] as usize) << 21) | ((s[1] as usize) << 14) | ((s[2] as usize) << 7) | s[3] as usize)
}

/// The total byte length of a leading ID3v2 tag, header included, or None.
pub fn v2_len(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < 10 || &bytes[..3] != b"ID3" {
        return None;
    }
    let body = synchsafe(&bytes[6..10])?;
    Some(10 + body)
}

/// True when the last 128 bytes are a trailing ID3v1 tag.
pub fn has_v1(bytes: &[u8]) -> bool {
    bytes.len() >= 128 && &bytes[bytes.len() - 128..bytes.len() - 125] == b"TAG"
}

pub fn scan(bytes: &[u8]) -> (Vec<Detection>, bool) {
    let mut det = Detection::present("id3", "ID3 tag", Honesty::Confirmable);
    if let Some(len) = v2_len(bytes) {
        det.locations.push(Location {
            container: "ID3v2".to_string(),
            offset: 0,
            length: len,
            detail: "leading tag".to_string(),
        });
        // Note PRIV and GEOB frames as evidence without a full frame parse.
        let body = bytes.get(10..len.min(bytes.len())).unwrap_or(&[]);
        if super::c2pa::contains(body, b"PRIV") {
            det.evidence.push("PRIV frame".to_string());
        }
        if super::c2pa::contains(body, b"GEOB") {
            det.evidence.push("GEOB frame".to_string());
        }
    }
    if has_v1(bytes) {
        det.locations.push(Location {
            container: "ID3v1".to_string(),
            offset: bytes.len() - 128,
            length: 128,
            detail: "trailing tag".to_string(),
        });
    }
    // ID3 reads fixed positions at the head and tail, so the scan is always
    // complete for a standalone stream.
    let found = if det.state == ScanState::ConfirmedPresent && !det.locations.is_empty() {
        vec![det]
    } else {
        Vec::new()
    };
    (found, true)
}
