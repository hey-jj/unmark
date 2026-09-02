//! ISO base media file walk (MP4, M4A, MOV). It reports a C2PA `uuid` box and
//! an `ilst` metadata atom. It reads box headers only and never touches `mdat`.

use super::c2pa;
use crate::scan::{Detection, Honesty, Location, ScanState};

/// One parsed box.
pub struct BoxHeader {
    pub kind: [u8; 4],
    /// Byte offset of the box, where its size field begins.
    pub start: usize,
    /// Total size in bytes including the header.
    pub total: usize,
    /// Offset of the box payload relative to the file.
    pub payload: usize,
}

/// Container boxes whose children this walk descends into.
const CONTAINERS: &[&[u8; 4]] = &[b"moov", b"udta", b"trak", b"mdia", b"minf", b"stbl"];

/// Parse the boxes directly under `[start, end)`. Returns the boxes and a
/// completeness flag: `complete` is true when the walk consumed the range to its
/// end without a box size overrunning it. Never panics.
fn boxes(bytes: &[u8], start: usize, end: usize) -> (Vec<BoxHeader>, bool) {
    let mut out = Vec::new();
    let mut p = start;
    let mut complete = true;
    while p + 8 <= end {
        let size32 = super::be_u32(bytes, p).unwrap_or(0) as u64;
        let kind = [bytes[p + 4], bytes[p + 5], bytes[p + 6], bytes[p + 7]];
        let (total, payload) = if size32 == 1 {
            // 64-bit size in the eight bytes after the type.
            let s = bytes.get(p + 8..p + 16);
            let Some(s) = s else {
                complete = false;
                break;
            };
            let big = u64::from_be_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]);
            (big as usize, p + 16)
        } else if size32 == 0 {
            (end - p, p + 8)
        } else {
            (size32 as usize, p + 8)
        };
        if total < 8 || p + total > end {
            complete = false;
            break;
        }
        out.push(BoxHeader {
            kind,
            start: p,
            total,
            payload,
        });
        p += total;
    }
    (out, complete)
}

pub fn scan(bytes: &[u8]) -> (Vec<Detection>, bool) {
    let mut c2pa_det = Detection::present("c2pa", "C2PA manifest", Honesty::Confirmable);
    let mut ilst_det = Detection::present("ilst", "MP4 ilst tag", Honesty::Confirmable);
    // Completeness keys on the top-level box walk covering the whole file.
    let (_, complete) = boxes(bytes, 0, bytes.len());
    walk(bytes, 0, bytes.len(), &mut c2pa_det, &mut ilst_det, 0);

    let mut out = Vec::new();
    for d in [c2pa_det, ilst_det] {
        if d.state == ScanState::ConfirmedPresent && !d.locations.is_empty() {
            out.push(d);
        }
    }
    (out, complete)
}

fn walk(
    bytes: &[u8],
    start: usize,
    end: usize,
    c2pa_det: &mut Detection,
    ilst_det: &mut Detection,
    depth: usize,
) {
    if depth > 8 {
        return;
    }
    let (level, _) = boxes(bytes, start, end);
    for b in level {
        match &b.kind {
            b"uuid" => {
                let data = bytes.get(b.payload..b.start + b.total).unwrap_or(&[]);
                if c2pa::looks_like_c2pa(data) {
                    c2pa_det.locations.push(Location {
                        container: "ISOBMFF uuid".to_string(),
                        offset: b.start,
                        length: b.total,
                        detail: "C2PA store".to_string(),
                    });
                }
            }
            b"ilst" => {
                ilst_det.locations.push(Location {
                    container: "ISOBMFF ilst".to_string(),
                    offset: b.start,
                    length: b.total,
                    detail: String::new(),
                });
            }
            b"meta" => {
                // A FullBox: skip its four version-and-flags bytes before its
                // children.
                let child_start = (b.payload + 4).min(b.start + b.total);
                walk(
                    bytes,
                    child_start,
                    b.start + b.total,
                    c2pa_det,
                    ilst_det,
                    depth + 1,
                );
            }
            k if CONTAINERS.contains(&k) => {
                walk(
                    bytes,
                    b.payload,
                    b.start + b.total,
                    c2pa_det,
                    ilst_det,
                    depth + 1,
                );
            }
            _ => {}
        }
    }
}
