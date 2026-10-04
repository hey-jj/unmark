//! PNG chunk walk. One pass reports PNG text chunks, an EXIF chunk, an XMP
//! packet, and a C2PA chunk. The walk copies nothing and decodes no image data.

use super::{be_u32, exif, xmp};
use crate::scan::{Detection, Honesty, Location, ScanState};

pub const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];

/// A PNG chunk, borrowing its data from the input.
pub struct Chunk<'a> {
    pub kind: [u8; 4],
    /// Byte offset of the length field, where the whole chunk begins.
    pub start: usize,
    /// Total chunk length including length, type, and CRC.
    pub total: usize,
    pub data: &'a [u8],
}

/// Walk PNG chunks. Returns the chunks and a completeness flag. `complete` is
/// true only when the walk reached the IEND chunk cleanly. A corrupt length or a
/// truncated chunk stops the walk with `complete` false, so a caller never reads
/// an early break as an exhaustive scan. Handles any input without panicking.
pub fn chunks(bytes: &[u8]) -> (Vec<Chunk<'_>>, bool) {
    let mut out = Vec::new();
    if bytes.len() < 8 || bytes[..8] != SIGNATURE {
        return (out, false);
    }
    let mut p = 8;
    let mut complete = false;
    while p + 12 <= bytes.len() {
        let Some(len) = be_u32(bytes, p) else { break };
        let len = len as usize;
        let data_start = p + 8;
        let Some(data) = bytes.get(data_start..data_start + len) else {
            break;
        };
        let kind = [bytes[p + 4], bytes[p + 5], bytes[p + 6], bytes[p + 7]];
        let total = 12 + len;
        out.push(Chunk {
            kind,
            start: p,
            total,
            data,
        });
        if kind == *b"IEND" {
            complete = true;
            break;
        }
        p += total;
    }
    (out, complete)
}

/// PNG keyword is the ASCII before the first null in a text chunk's data.
fn keyword(data: &[u8]) -> String {
    super::ascii_preview(
        data.iter()
            .take_while(|c| **c != 0)
            .copied()
            .collect::<Vec<_>>()
            .as_slice(),
        80,
    )
}

pub fn scan(bytes: &[u8]) -> (Vec<Detection>, bool) {
    let mut png_text = Detection::present("png_text", "PNG text chunk", Honesty::Confirmable);
    let mut xmp_det = Detection::present("xmp", "XMP packet", Honesty::Confirmable);
    let mut exif_det = Detection::present("exif", "EXIF metadata", Honesty::Confirmable);
    let mut c2pa_det = Detection::present("c2pa", "C2PA manifest", Honesty::Confirmable);

    let (chunks, complete) = chunks(bytes);
    for c in chunks {
        match &c.kind {
            b"tEXt" | b"zTXt" | b"iTXt" => {
                let kw = keyword(c.data);
                if c.kind == *b"iTXt" && kw == xmp::XMP_PNG_KEYWORD {
                    xmp_det.locations.push(Location {
                        container: "PNG iTXt".to_string(),
                        offset: c.start,
                        length: c.total,
                        detail: kw,
                    });
                    if let Some(tool) = xmp::creator_tool(c.data) {
                        xmp_det.evidence.push(tool);
                    }
                    if xmp::references_c2pa(c.data) {
                        xmp_det
                            .evidence
                            .push("references a C2PA manifest".to_string());
                    }
                } else {
                    let kind = std::str::from_utf8(&c.kind).unwrap_or("text");
                    png_text.locations.push(Location {
                        container: format!("PNG {kind}"),
                        offset: c.start,
                        length: c.total,
                        detail: kw.clone(),
                    });
                    if !kw.is_empty() {
                        png_text.evidence.push(kw);
                    }
                }
            }
            b"eXIf" => {
                exif_det.locations.push(Location {
                    container: "PNG eXIf".to_string(),
                    offset: c.start,
                    length: c.total,
                    detail: String::new(),
                });
                if let Some(facts) = exif::parse_tiff(c.data) {
                    if let Some(sw) = facts.software {
                        exif_det.evidence.push(format!("Software: {sw}"));
                    }
                    if let Some(mk) = facts.make {
                        exif_det.evidence.push(format!("Make: {mk}"));
                    }
                }
            }
            b"caBX" => {
                c2pa_det.locations.push(Location {
                    container: "PNG caBX".to_string(),
                    offset: c.start,
                    length: c.total,
                    detail: "C2PA store".to_string(),
                });
            }
            _ => {}
        }
    }

    let mut out = Vec::new();
    for d in [png_text, xmp_det, exif_det, c2pa_det] {
        if d.state == ScanState::ConfirmedPresent && !d.locations.is_empty() {
            out.push(d);
        }
    }
    (out, complete)
}
