//! JPEG segment walk. It reports EXIF, XMP, C2PA, and IPTC from the APPn
//! markers and stops at the start of the entropy-coded scan.

use super::{be_u16, c2pa, exif, xmp};
use crate::scan::{Detection, Honesty, Location, ScanState};

/// A JPEG marker segment carrying a two-byte length.
pub struct Segment<'a> {
    pub marker: u8,
    /// Byte offset of the marker's `0xFF`.
    pub start: usize,
    /// Total segment length including the two marker bytes and the length field.
    pub total: usize,
    pub data: &'a [u8],
}

pub const EXIF_ID: &[u8] = b"Exif\0\0";
pub const IPTC_ID: &[u8] = b"Photoshop 3.0\0";

/// Walk APPn and other length-bearing markers up to the scan start. Returns the
/// segments and a completeness flag. `complete` is true when the walk reached
/// the start-of-scan or end-of-image marker, meaning the whole metadata region
/// was covered. A corrupt segment length stops the walk with `complete` false.
/// Handles any input without panicking. A stand-alone marker without a length is skipped.
pub fn segments(bytes: &[u8]) -> (Vec<Segment<'_>>, bool) {
    let mut out = Vec::new();
    if bytes.len() < 2 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return (out, false);
    }
    let mut p = 2;
    let mut complete = false;
    while p + 4 <= bytes.len() {
        if bytes[p] != 0xFF {
            break;
        }
        let marker = bytes[p + 1];
        // Start of scan or end of image: entropy data follows, the metadata
        // region is fully covered.
        if marker == 0xDA || marker == 0xD9 {
            complete = true;
            break;
        }
        // Standalone markers (RSTn, TEM) carry no length.
        if (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
            p += 2;
            continue;
        }
        let Some(len) = be_u16(bytes, p + 2) else {
            break;
        };
        let len = len as usize;
        if len < 2 {
            break;
        }
        let total = 2 + len;
        let data_start = p + 4;
        let Some(data) = bytes.get(data_start..p + total) else {
            break;
        };
        out.push(Segment {
            marker,
            start: p,
            total,
            data,
        });
        p += total;
    }
    (out, complete)
}

pub fn scan(bytes: &[u8]) -> (Vec<Detection>, bool) {
    let mut exif_det = Detection::present("exif", "EXIF metadata", Honesty::Confirmable);
    let mut xmp_det = Detection::present("xmp", "XMP packet", Honesty::Confirmable);
    let mut c2pa_det = Detection::present("c2pa", "C2PA manifest", Honesty::Confirmable);
    let mut iptc_det = Detection::present("iptc", "IPTC block", Honesty::Confirmable);

    let (segments, complete) = segments(bytes);
    for s in segments {
        match s.marker {
            // APP1: EXIF or XMP.
            0xE1 => {
                if s.data.starts_with(EXIF_ID) {
                    exif_det.locations.push(Location {
                        container: "JPEG APP1 Exif".to_string(),
                        offset: s.start,
                        length: s.total,
                        detail: String::new(),
                    });
                    if let Some(facts) = exif::parse_tiff(&s.data[EXIF_ID.len()..]) {
                        if let Some(sw) = facts.software {
                            exif_det.evidence.push(format!("Software: {sw}"));
                        }
                        if let Some(mk) = facts.make {
                            exif_det.evidence.push(format!("Make: {mk}"));
                        }
                    }
                } else if s.data.starts_with(xmp::XMP_APP1_ID) {
                    let packet = &s.data[xmp::XMP_APP1_ID.len()..];
                    xmp_det.locations.push(Location {
                        container: "JPEG APP1 XMP".to_string(),
                        offset: s.start,
                        length: s.total,
                        detail: String::new(),
                    });
                    if let Some(tool) = xmp::creator_tool(packet) {
                        xmp_det.evidence.push(tool);
                    }
                    if xmp::references_c2pa(packet) {
                        xmp_det
                            .evidence
                            .push("references a C2PA manifest".to_string());
                    }
                }
            }
            // APP11: JPEG box marker, C2PA carriage.
            0xEB => {
                if c2pa::looks_like_c2pa(s.data) {
                    c2pa_det.locations.push(Location {
                        container: "JPEG APP11".to_string(),
                        offset: s.start,
                        length: s.total,
                        detail: "C2PA store".to_string(),
                    });
                }
            }
            // APP13: Photoshop IRB, IPTC carriage.
            0xED if s.data.starts_with(IPTC_ID) => {
                iptc_det.locations.push(Location {
                    container: "JPEG APP13".to_string(),
                    offset: s.start,
                    length: s.total,
                    detail: "Photoshop IRB".to_string(),
                });
            }
            _ => {}
        }
    }

    let mut out = Vec::new();
    for d in [exif_det, xmp_det, c2pa_det, iptc_det] {
        if d.state == ScanState::ConfirmedPresent && !d.locations.is_empty() {
            out.push(d);
        }
    }
    (out, complete)
}

/// Whether the frame header declares chroma subsampling: a component whose
/// sampling factors differ from the first component's. None when no frame
/// header was read.
pub fn chroma_subsampled(bytes: &[u8]) -> Option<bool> {
    let (segs, _) = segments(bytes);
    for s in segs {
        if matches!(
            s.marker,
            0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF
        ) {
            let n = *s.data.get(5)? as usize;
            let mut factors = Vec::with_capacity(n);
            for i in 0..n {
                factors.push(*s.data.get(6 + i * 3 + 1)?);
            }
            let first = *factors.first()?;
            return Some(factors.iter().any(|f| *f != first));
        }
    }
    None
}
