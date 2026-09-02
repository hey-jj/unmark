//! RIFF chunk walk, shared across WebP, WAV, and AVI. It reports EXIF, XMP,
//! C2PA, ID3, and LIST INFO. A RIFF rewrite lives in `container::riff` and is
//! kept separate so a read never rebuilds sizes.

use super::{c2pa, exif, le_u32, xmp};
use crate::asset::Format;
use crate::scan::{Detection, Honesty, Location, ScanState};

/// A top-level RIFF chunk borrowing its data from the input.
pub struct Chunk<'a> {
    pub id: [u8; 4],
    /// Byte offset of the chunk id, where the chunk begins.
    pub start: usize,
    /// Declared data length, before even-boundary padding.
    pub size: usize,
    /// Total bytes the chunk occupies including id, size, data, and pad byte.
    pub total: usize,
    pub data: &'a [u8],
}

/// Walk the top-level chunks inside the RIFF payload. Returns the chunks and a
/// completeness flag. `complete` is true when the walk consumed the payload to
/// its declared or actual end without a chunk size overrunning the buffer. A
/// corrupt size stops the walk with `complete` false. Never panics.
pub fn chunks(bytes: &[u8]) -> (Vec<Chunk<'_>>, bool) {
    let mut out = Vec::new();
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" {
        return (out, false);
    }
    // The RIFF header declares the payload length. The walk is complete when it
    // reaches that end (or the buffer end) without a failed read.
    let declared_end = le_u32(bytes, 4)
        .map(|s| (8 + s as usize).min(bytes.len()))
        .unwrap_or(bytes.len());
    let mut p = 12;
    let mut complete = false;
    while p + 8 <= bytes.len() {
        let Some(size) = le_u32(bytes, p + 4) else {
            break;
        };
        let size = size as usize;
        let data_start = p + 8;
        let Some(data) = bytes.get(data_start..data_start + size) else {
            break;
        };
        let id = [bytes[p], bytes[p + 1], bytes[p + 2], bytes[p + 3]];
        let padded = size + (size & 1);
        let total = 8 + padded;
        out.push(Chunk {
            id,
            start: p,
            size,
            total,
            data,
        });
        p += total;
        if p >= declared_end {
            complete = true;
            break;
        }
    }
    // A payload that ends exactly on the last chunk boundary is complete even
    // when fewer than eight trailing bytes remain.
    if !complete && p >= declared_end {
        complete = true;
    }
    // Bytes left past the declared end mean the RIFF size understates the file,
    // so chunks sit where the walk never looked. Those chunks are unknown, not
    // absent. A single trailing pad byte is the only excess allowed.
    if bytes.len() > declared_end + 1 {
        complete = false;
    }
    (out, complete)
}

pub fn scan(bytes: &[u8], format: Format) -> (Vec<Detection>, bool) {
    let mut exif_det = Detection::present("exif", "EXIF metadata", Honesty::Confirmable);
    let mut xmp_det = Detection::present("xmp", "XMP packet", Honesty::Confirmable);
    let mut c2pa_det = Detection::present("c2pa", "C2PA manifest", Honesty::Confirmable);
    let mut id3_det = Detection::present("id3", "ID3 tag", Honesty::Confirmable);
    let mut riff_det = Detection::present(
        "riff_ancillary",
        "RIFF ancillary chunk",
        Honesty::Confirmable,
    );

    let (chunks, complete) = chunks(bytes);
    for c in chunks {
        let container = format!("{} {}", format.as_str(), ascii_id(&c.id));
        match &c.id {
            b"EXIF" => {
                exif_det.locations.push(loc(&container, &c, ""));
                let payload = c.data.strip_prefix(super::jpeg::EXIF_ID).unwrap_or(c.data);
                if let Some(facts) = exif::parse_tiff(payload) {
                    if let Some(sw) = facts.software {
                        exif_det.evidence.push(format!("Software: {sw}"));
                    }
                }
            }
            b"XMP " => {
                xmp_det.locations.push(loc(&container, &c, ""));
                if let Some(tool) = xmp::creator_tool(c.data) {
                    xmp_det.evidence.push(tool);
                }
            }
            b"C2PA" => {
                c2pa_det.locations.push(loc(&container, &c, "C2PA store"));
            }
            b"id3 " => {
                id3_det
                    .locations
                    .push(loc(&container, &c, "embedded id3 chunk"));
            }
            b"LIST" => {
                if c.data.get(0..4) == Some(b"INFO") {
                    riff_det.locations.push(loc(&container, &c, "LIST INFO"));
                }
            }
            _ => {
                // A stray C2PA payload can also ride in an unusual chunk.
                if c2pa::looks_like_c2pa(c.data) && c.id != *b"data" {
                    c2pa_det.locations.push(loc(&container, &c, "C2PA store"));
                }
            }
        }
    }

    let mut out = Vec::new();
    for d in [exif_det, xmp_det, c2pa_det, id3_det, riff_det] {
        if d.state == ScanState::ConfirmedPresent && !d.locations.is_empty() {
            out.push(d);
        }
    }
    (out, complete)
}

fn loc(container: &str, c: &Chunk<'_>, detail: &str) -> Location {
    Location {
        container: container.to_string(),
        offset: c.start,
        length: c.total,
        detail: detail.to_string(),
    }
}

fn ascii_id(id: &[u8; 4]) -> String {
    id.iter()
        .map(|b| {
            if b.is_ascii_graphic() {
                *b as char
            } else {
                '.'
            }
        })
        .collect()
}
