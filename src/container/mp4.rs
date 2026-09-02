//! ISOBMFF rewriter. It removes a top-level C2PA `uuid` box and corrects the
//! `stco` and `co64` sample-offset tables by the removed byte count, so the
//! media in `mdat` still resolves. The `mdat` bytes themselves are never
//! touched. Stripping an `ilst` atom from MP4 is held for a later milestone,
//! because a safe rewrite there needs the same offset accounting across nested
//! container sizes and is not yet built. That case declines rather than
//! producing a file whose sample table points at the wrong bytes.

use super::{DropSpec, RewriteError};
use crate::detect::c2pa;

struct TopBox {
    kind: [u8; 4],
    start: usize,
    total: usize,
    payload: usize,
}

/// The top-level boxes and a completeness flag. `complete` is false when a box
/// size overruns the file, so the boxes past it were never seen.
fn top_boxes(bytes: &[u8]) -> (Vec<TopBox>, bool) {
    let mut out = Vec::new();
    let mut p = 0;
    let mut complete = true;
    while p + 8 <= bytes.len() {
        let size32 = crate::detect::be_u32(bytes, p).unwrap_or(0) as usize;
        let kind = [bytes[p + 4], bytes[p + 5], bytes[p + 6], bytes[p + 7]];
        let (total, payload) = if size32 == 1 {
            let Some(s) = bytes.get(p + 8..p + 16) else {
                complete = false;
                break;
            };
            let big = u64::from_be_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]);
            (big as usize, p + 16)
        } else if size32 == 0 {
            (bytes.len() - p, p + 8)
        } else {
            (size32, p + 8)
        };
        if total < 8 || p + total > bytes.len() {
            complete = false;
            break;
        }
        out.push(TopBox {
            kind,
            start: p,
            total,
            payload,
        });
        p += total;
    }
    (out, complete)
}

/// True when the file carries an `ilst` atom anywhere in its box tree.
fn has_ilst(bytes: &[u8], start: usize, end: usize, depth: usize) -> bool {
    if depth > 8 {
        return false;
    }
    let mut p = start;
    while p + 8 <= end {
        let size32 = crate::detect::be_u32(bytes, p).unwrap_or(0) as usize;
        let kind = [bytes[p + 4], bytes[p + 5], bytes[p + 6], bytes[p + 7]];
        let total = if size32 == 0 { end - p } else { size32 };
        if total < 8 || p + total > end {
            break;
        }
        if &kind == b"ilst" {
            return true;
        }
        let is_container = matches!(
            &kind,
            b"moov" | b"udta" | b"trak" | b"mdia" | b"minf" | b"stbl"
        );
        if is_container && has_ilst(bytes, p + 8, p + total, depth + 1) {
            return true;
        }
        if &kind == b"meta" && has_ilst(bytes, p + 12, p + total, depth + 1) {
            return true;
        }
        p += total;
    }
    false
}

pub fn rewrite(bytes: &[u8], spec: &DropSpec) -> Result<Vec<u8>, RewriteError> {
    // A walk that overran a box size never saw the boxes past it, so any
    // rewrite would stand on an unknown layout. Refuse and write nothing.
    let (boxes, complete) = top_boxes(bytes);
    if !complete {
        return Err(RewriteError::Malformed(
            "the ISOBMFF box stream did not parse to the end of the file".to_string(),
        ));
    }
    // Held for a later milestone: safe ilst removal from MP4.
    if spec.ilst && has_ilst(bytes, 0, bytes.len(), 0) {
        return Err(RewriteError::Declined {
            class: "ilst".to_string(),
            reason:
                "stripping an ilst tag from an ISO base media file is not supported in this build"
                    .to_string(),
        });
    }
    if !spec.c2pa {
        return Ok(bytes.to_vec());
    }

    // Collect the top-level C2PA uuid boxes to remove.
    let removed: Vec<(usize, usize)> = boxes
        .iter()
        .filter(|b| {
            &b.kind == b"uuid"
                && c2pa::looks_like_c2pa(bytes.get(b.payload..b.start + b.total).unwrap_or(&[]))
        })
        .map(|b| (b.start, b.total))
        .collect();
    if removed.is_empty() {
        return Ok(bytes.to_vec());
    }

    // Correct sample-offset tables in a mutable copy before excising the boxes,
    // so table positions stay valid while the values are edited in place.
    let mut work = bytes.to_vec();
    let mut tables = Vec::new();
    collect_offset_tables(bytes, 0, bytes.len(), &mut tables, 0);
    for t in &tables {
        correct_table(&mut work, t, &removed);
    }

    // Excise the removed ranges from the corrected copy, high offset first.
    let mut ranges = removed.clone();
    ranges.sort_by_key(|(start, _)| std::cmp::Reverse(*start));
    for (start, len) in ranges {
        if start + len <= work.len() {
            work.drain(start..start + len);
        }
    }
    Ok(work)
}

struct OffsetTable {
    /// Byte offset of the first entry.
    entries_at: usize,
    count: usize,
    wide: bool,
}

fn collect_offset_tables(
    bytes: &[u8],
    start: usize,
    end: usize,
    out: &mut Vec<OffsetTable>,
    depth: usize,
) {
    if depth > 8 {
        return;
    }
    let mut p = start;
    while p + 8 <= end {
        let size32 = crate::detect::be_u32(bytes, p).unwrap_or(0) as usize;
        let kind = [bytes[p + 4], bytes[p + 5], bytes[p + 6], bytes[p + 7]];
        let total = if size32 == 0 { end - p } else { size32 };
        if total < 8 || p + total > end {
            break;
        }
        match &kind {
            b"stco" | b"co64" => {
                // FullBox: 4 bytes version and flags, then a 4-byte count.
                let count_at = p + 12;
                if let Some(count) = crate::detect::be_u32(bytes, count_at) {
                    out.push(OffsetTable {
                        entries_at: count_at + 4,
                        count: count as usize,
                        wide: &kind == b"co64",
                    });
                }
            }
            b"moov" | b"udta" | b"trak" | b"mdia" | b"minf" | b"stbl" => {
                collect_offset_tables(bytes, p + 8, p + total, out, depth + 1);
            }
            b"meta" => {
                collect_offset_tables(bytes, p + 12, p + total, out, depth + 1);
            }
            _ => {}
        }
        p += total;
    }
}

fn shift_for(offset: u64, removed: &[(usize, usize)]) -> u64 {
    removed
        .iter()
        .filter(|(start, _)| (*start as u64) < offset)
        .map(|(_, len)| *len as u64)
        .sum()
}

fn correct_table(work: &mut [u8], t: &OffsetTable, removed: &[(usize, usize)]) {
    let step = if t.wide { 8 } else { 4 };
    for i in 0..t.count {
        let at = t.entries_at + i * step;
        if at + step > work.len() {
            break;
        }
        if t.wide {
            let v = u64::from_be_bytes(work[at..at + 8].try_into().unwrap());
            let nv = v.saturating_sub(shift_for(v, removed));
            work[at..at + 8].copy_from_slice(&nv.to_be_bytes());
        } else {
            let v = u32::from_be_bytes(work[at..at + 4].try_into().unwrap()) as u64;
            let nv = v.saturating_sub(shift_for(v, removed));
            work[at..at + 4].copy_from_slice(&(nv as u32).to_be_bytes());
        }
    }
}

/// The `mdat` payload. It must not change across a metadata rewrite.
pub fn signal_stream(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let (boxes, _) = top_boxes(bytes);
    for b in boxes {
        if &b.kind == b"mdat" {
            if let Some(data) = bytes.get(b.payload..b.start + b.total) {
                out.extend_from_slice(data);
            }
        }
    }
    out
}
