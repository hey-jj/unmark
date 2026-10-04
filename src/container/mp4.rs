//! ISOBMFF rewriter. It removes a top-level C2PA `uuid` box and the `ilst`
//! metadata atom wherever it sits in the box tree, corrects every enclosing
//! box size on the path to a removed atom, and corrects the `stco` and `co64`
//! sample-offset tables by the bytes removed before each chunk, so the media
//! in `mdat` still resolves. The `mdat` bytes themselves are never touched.

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

/// Every `ilst` atom in the box tree, as (start, total) ranges, with the
/// chain of enclosing boxes (start offsets) whose sizes shrink when it goes.
fn find_ilst(
    bytes: &[u8],
    start: usize,
    end: usize,
    depth: usize,
    chain: &mut Vec<usize>,
    out: &mut Vec<(usize, usize, Vec<usize>)>,
) {
    if depth > 8 {
        return;
    }
    let mut p = start;
    while p + 8 <= end {
        let size32 = crate::detect::be_u32(bytes, p).unwrap_or(0) as usize;
        let kind = [bytes[p + 4], bytes[p + 5], bytes[p + 6], bytes[p + 7]];
        // A 64-bit or to-end size at this depth is a layout this rewriter
        // skips whole.
        let total = if size32 == 0 { end - p } else { size32 };
        if total < 8 || p + total > end {
            break;
        }
        if &kind == b"ilst" {
            out.push((p, total, chain.clone()));
        } else if matches!(
            &kind,
            b"moov" | b"udta" | b"trak" | b"mdia" | b"minf" | b"stbl"
        ) && size32 > 1
        {
            chain.push(p);
            find_ilst(bytes, p + 8, p + total, depth + 1, chain, out);
            chain.pop();
        } else if &kind == b"meta" && size32 > 1 {
            chain.push(p);
            find_ilst(bytes, p + 12, p + total, depth + 1, chain, out);
            chain.pop();
        }
        p += total;
    }
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
    // Collect the ranges to remove: top-level C2PA uuid boxes and ilst atoms.
    let mut removed: Vec<(usize, usize)> = Vec::new();
    let mut chains: Vec<Vec<usize>> = Vec::new();
    if spec.c2pa {
        for b in &boxes {
            if &b.kind == b"uuid"
                && c2pa::looks_like_c2pa(bytes.get(b.payload..b.start + b.total).unwrap_or(&[]))
            {
                removed.push((b.start, b.total));
                chains.push(Vec::new());
            }
        }
    }
    if spec.ilst {
        let mut found = Vec::new();
        find_ilst(bytes, 0, bytes.len(), 0, &mut Vec::new(), &mut found);
        for (start, total, chain) in found {
            removed.push((start, total));
            chains.push(chain);
        }
    }
    if removed.is_empty() {
        return Ok(bytes.to_vec());
    }

    // Correct sample-offset tables and enclosing box sizes in a mutable copy
    // before excising the ranges, so positions stay valid while the values
    // are edited in place.
    let mut work = bytes.to_vec();
    let mut tables = Vec::new();
    collect_offset_tables(bytes, 0, bytes.len(), &mut tables, 0);
    for t in &tables {
        correct_table(&mut work, t, &removed);
    }
    for ((_, total), chain) in removed.iter().zip(&chains) {
        for &parent in chain {
            let size = u32::from_be_bytes(work[parent..parent + 4].try_into().unwrap());
            let shrunk = size.saturating_sub(*total as u32);
            work[parent..parent + 4].copy_from_slice(&shrunk.to_be_bytes());
        }
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
