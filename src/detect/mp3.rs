//! MPEG audio layer III, read from the frame headers of ISO/IEC 11172-3 and
//! 13818-3: the sync word, version, layer, bitrate and sample-rate indices,
//! padding, and channel mode give each frame's length and side-information
//! size, and the first bits of the side information give main_data_begin,
//! the bit reservoir pointer. Around the frames sit the tags: a leading
//! ID3v2 tag with its padding and optional footer, a trailing ID3v1 tag, an
//! APE tag at either end, and an information frame (Xing, Info, or VBRI)
//! that an encoder writes as the first frame. Nothing here decodes audio.

use crate::scan::{Detection, Honesty, Location, ScanState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Version {
    Mpeg1,
    Mpeg2,
    Mpeg25,
}

/// One parsed frame header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameHeader {
    pub version: Version,
    pub crc: bool,
    pub bitrate_kbps: u32,
    pub sample_rate: u32,
    pub padding: bool,
    pub mono: bool,
    /// Total frame length in bytes, header included.
    pub len: usize,
    /// Side information length in bytes.
    pub side_info: usize,
}

const BITRATE_V1: [u32; 16] = [
    0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0,
];
const BITRATE_V2: [u32; 16] = [
    0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 0,
];

/// Parse a layer III frame header at the start of `b`. None for anything
/// that is not a layer III frame with a tabled bitrate and sample rate; a
/// free-format bitrate (index zero) is None too, since its length is not in
/// the header.
pub fn parse_header(b: &[u8]) -> Option<FrameHeader> {
    let h = b.get(0..4)?;
    if h[0] != 0xFF || (h[1] & 0xE0) != 0xE0 {
        return None;
    }
    let version = match (h[1] >> 3) & 0x03 {
        0b11 => Version::Mpeg1,
        0b10 => Version::Mpeg2,
        0b00 => Version::Mpeg25,
        _ => return None,
    };
    if (h[1] >> 1) & 0x03 != 0b01 {
        return None; // layer III only
    }
    let crc = h[1] & 0x01 == 0;
    let bitrate_index = (h[2] >> 4) as usize;
    let sr_index = ((h[2] >> 2) & 0x03) as usize;
    let padding = (h[2] >> 1) & 0x01 == 1;
    let mono = (h[3] >> 6) == 0b11;
    let bitrate_kbps = match version {
        Version::Mpeg1 => BITRATE_V1[bitrate_index],
        _ => BITRATE_V2[bitrate_index],
    };
    if bitrate_kbps == 0 {
        return None;
    }
    let sample_rate = match (version, sr_index) {
        (Version::Mpeg1, 0) => 44100,
        (Version::Mpeg1, 1) => 48000,
        (Version::Mpeg1, 2) => 32000,
        (Version::Mpeg2, 0) => 22050,
        (Version::Mpeg2, 1) => 24000,
        (Version::Mpeg2, 2) => 16000,
        (Version::Mpeg25, 0) => 11025,
        (Version::Mpeg25, 1) => 12000,
        (Version::Mpeg25, 2) => 8000,
        _ => return None,
    };
    let per_frame = if version == Version::Mpeg1 {
        144_000
    } else {
        72_000
    };
    let len = (per_frame * bitrate_kbps / sample_rate) as usize + padding as usize;
    let side_info = match (version, mono) {
        (Version::Mpeg1, true) => 17,
        (Version::Mpeg1, false) => 32,
        (_, true) => 9,
        (_, false) => 17,
    };
    Some(FrameHeader {
        version,
        crc,
        bitrate_kbps,
        sample_rate,
        padding,
        mono,
        len,
        side_info,
    })
}

/// The bit reservoir pointer of a frame: nine bits in MPEG-1, eight in
/// MPEG-2 and 2.5, at the start of the side information.
pub fn main_data_begin(frame: &[u8], h: &FrameHeader) -> Option<u16> {
    let at = 4 + if h.crc { 2 } else { 0 };
    let s = frame.get(at..at + 2)?;
    let word = u16::from_be_bytes([s[0], s[1]]);
    Some(match h.version {
        Version::Mpeg1 => word >> 7,
        _ => word >> 8,
    })
}

/// The information frame an encoder writes first: a Xing or Info tag with
/// its flagged fields and the LAME-style extension after them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InfoFrame {
    pub offset: usize,
    pub len: usize,
    /// "Xing", "Info", or "VBRI".
    pub kind: String,
    /// Offset of the tag id within the frame.
    pub tag_at: usize,
    pub flags: u32,
    pub frames: Option<u32>,
    pub bytes: Option<u32>,
    pub has_toc: bool,
    pub has_quality: bool,
    /// Offset of the extension within the frame, when the frame has room
    /// for its 36 bytes.
    pub extension_at: Option<usize>,
    /// The encoder string the extension carries, when printable.
    pub encoder: Option<String>,
    /// Encoder delay and padding from the extension, in samples.
    pub delay_padding: Option<(u16, u16)>,
    /// True when the extension carries an identity: a nonzero byte in the
    /// encoder string, the revision, the lowpass, the replay gain, the
    /// encoding flags, the ABR rate, the misc, gain, or preset fields.
    pub identity: bool,
}

/// Field offsets inside the 36-byte extension.
pub const EXT_ENCODER: std::ops::Range<usize> = 0..9;
pub const EXT_DELAY_PADDING: std::ops::Range<usize> = 21..24;
pub const EXT_LEN: usize = 36;

/// Where everything sits in a standalone MPEG audio file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Layout {
    /// Leading ID3v2 tags as (offset, length), padding and footer included.
    pub id3v2: Vec<(usize, usize)>,
    /// A trailing ID3v1 tag.
    pub id3v1: Option<usize>,
    /// APE tags as (offset, length), leading or trailing.
    pub ape: Vec<(usize, usize)>,
    pub info: Option<InfoFrame>,
    /// The audio frames, information frame included.
    pub frames_start: usize,
    pub frames_end: usize,
    /// The number of frames the walk parsed.
    pub frames: usize,
    /// True when the frame walk reached frames_end exactly.
    pub complete: bool,
}

fn id3v2_len(b: &[u8]) -> Option<usize> {
    if b.len() < 10 || &b[..3] != b"ID3" {
        return None;
    }
    let body = super::id3::synchsafe(&b[6..10])?;
    let footer = if b[5] & 0x10 != 0 { 10 } else { 0 };
    Some(10 + body + footer)
}

const APE_PREAMBLE: &[u8] = b"APETAGEX";
const APE_HAS_HEADER: u32 = 0x8000_0000;
const APE_IS_HEADER: u32 = 0x2000_0000;

/// An APE tag whose header or footer starts at `at`: (tag start, length).
fn ape_at(b: &[u8], at: usize) -> Option<(usize, usize)> {
    let block = b.get(at..at + 32)?;
    if &block[..8] != APE_PREAMBLE {
        return None;
    }
    let size = u32::from_le_bytes([block[12], block[13], block[14], block[15]]) as usize;
    let flags = u32::from_le_bytes([block[20], block[21], block[22], block[23]]);
    if size < 32 {
        return None;
    }
    if flags & APE_IS_HEADER != 0 {
        // Header: items and footer follow.
        let len = 32 + size;
        if at + len > b.len() {
            return None;
        }
        Some((at, len))
    } else {
        // Footer: items precede it, and a header precedes those when flagged.
        let header = if flags & APE_HAS_HEADER != 0 { 32 } else { 0 };
        let end = at + 32;
        let start = end.checked_sub(size + header)?;
        Some((start, end - start))
    }
}

pub fn layout(bytes: &[u8]) -> Layout {
    let mut l = Layout::default();
    // Leading tags in any order: ID3v2 tags and an APE header.
    let mut start = 0usize;
    loop {
        if let Some(len) = id3v2_len(&bytes[start..]) {
            if start + len > bytes.len() {
                break;
            }
            l.id3v2.push((start, len));
            start += len;
        } else if let Some((at, len)) = ape_at(bytes, start) {
            l.ape.push((at, len));
            start += len;
        } else {
            break;
        }
    }
    // Trailing tags in any order: an ID3v1 tag and an APE footer.
    let mut end = bytes.len();
    loop {
        if end >= start + 128 && &bytes[end - 128..end - 125] == b"TAG" && l.id3v1.is_none() {
            end -= 128;
            l.id3v1 = Some(end);
        } else if end >= start + 32 {
            match ape_at(bytes, end - 32) {
                Some((at, len)) if at >= start && at + len == end => {
                    l.ape.push((at, len));
                    end = at;
                }
                _ => break,
            }
        } else {
            break;
        }
    }
    l.ape.sort();
    l.frames_start = start.min(end);
    l.frames_end = end;
    // Walk the frames.
    let mut p = l.frames_start;
    let mut first = true;
    while p < end {
        let Some(h) = parse_header(&bytes[p..end]) else {
            break;
        };
        if p + h.len > end {
            break;
        }
        if first {
            first = false;
            l.info = info_frame(bytes, p, &h, end);
        }
        l.frames += 1;
        p += h.len;
    }
    l.complete = p == end && l.frames > 0;
    l
}

fn info_frame(bytes: &[u8], p: usize, h: &FrameHeader, end: usize) -> Option<InfoFrame> {
    let frame = &bytes[p..end.min(p + h.len)];
    let xing_at = 4 + if h.crc { 2 } else { 0 } + h.side_info;
    let kind = match frame.get(xing_at..xing_at + 4) {
        Some(b"Xing") => Some("Xing"),
        Some(b"Info") => Some("Info"),
        _ => None,
    };
    let (kind, tag_at) = match kind {
        Some(k) => (k, xing_at),
        None => {
            if frame.get(36..40) == Some(b"VBRI") {
                ("VBRI", 36)
            } else {
                return None;
            }
        }
    };
    let mut info = InfoFrame {
        offset: p,
        len: h.len,
        kind: kind.to_string(),
        tag_at,
        flags: 0,
        frames: None,
        bytes: None,
        has_toc: false,
        has_quality: false,
        extension_at: None,
        encoder: None,
        delay_padding: None,
        identity: false,
    };
    if kind == "VBRI" {
        return Some(info);
    }
    let be32 = |at: usize| {
        frame
            .get(at..at + 4)
            .map(|f| u32::from_be_bytes([f[0], f[1], f[2], f[3]]))
    };
    info.flags = be32(tag_at + 4).unwrap_or(0);
    let mut at = tag_at + 8;
    if info.flags & 1 != 0 {
        info.frames = be32(at);
        at += 4;
    }
    if info.flags & 2 != 0 {
        info.bytes = be32(at);
        at += 4;
    }
    if info.flags & 4 != 0 {
        info.has_toc = true;
        at += 100;
    }
    if info.flags & 8 != 0 {
        info.has_quality = true;
        at += 4;
    }
    if let Some(ext) = frame.get(at..at + EXT_LEN) {
        info.extension_at = Some(at);
        let printable: Vec<u8> = ext[EXT_ENCODER]
            .iter()
            .take_while(|c| c.is_ascii_graphic() || **c == b' ')
            .copied()
            .collect();
        let name = String::from_utf8_lossy(&printable).trim_end().to_string();
        if !name.is_empty() {
            info.encoder = Some(name);
        }
        // An extension of all zeros is no extension, as a decoder reads it.
        if ext.iter().any(|b| *b != 0) {
            let t = &ext[EXT_DELAY_PADDING];
            info.delay_padding = Some((
                (u16::from(t[0]) << 4) | u16::from(t[1] >> 4),
                (u16::from(t[1] & 0x0F) << 8) | u16::from(t[2]),
            ));
        }
        // Everything in the extension that is not the delay and padding.
        info.identity = ext
            .iter()
            .enumerate()
            .any(|(i, b)| !EXT_DELAY_PADDING.contains(&i) && *b != 0);
    }
    Some(info)
}

/// Read `n` bits at bit offset `at` of `b`, most significant first.
fn bits(b: &[u8], at: usize, n: usize) -> Option<u32> {
    let mut v = 0u32;
    for i in 0..n {
        let bit = at + i;
        let byte = *b.get(bit / 8)?;
        v = (v << 1) | ((byte >> (7 - bit % 8)) & 1) as u32;
    }
    Some(v)
}

/// The number of main-data bits a frame's side information declares: the
/// sum of part2_3_length over its granules and channels. The side
/// information layout is fixed by the standard: main_data_begin, the
/// private bits, scfsi in MPEG-1, then per granule and channel 59 bits in
/// MPEG-1 and 63 in MPEG-2 and 2.5, each starting with the 12-bit
/// part2_3_length.
pub fn main_data_bits(frame: &[u8], h: &FrameHeader) -> Option<u32> {
    let start = 4 + if h.crc { 2 } else { 0 };
    let side = frame.get(start..start + h.side_info)?;
    let (first, granules, per) = match (h.version, h.mono) {
        (Version::Mpeg1, true) => (9 + 5 + 4, 2usize, 59usize),
        (Version::Mpeg1, false) => (9 + 3 + 8, 2, 59),
        (_, true) => (8 + 1, 1, 63),
        (_, false) => (8 + 2, 1, 63),
    };
    let channels = if h.mono { 1 } else { 2 };
    let mut total = 0u32;
    for g in 0..granules {
        for c in 0..channels {
            let at = first + (g * channels + c) * per;
            total += bits(side, at, 12)?;
        }
    }
    Some(total)
}

/// The byte ranges of the file that hold ancillary data: bytes inside the
/// main-data regions of the audio frames that no frame's main data covers.
/// Main data for a frame starts main_data_begin bytes before the frame's
/// own region, in the reservoir the earlier frames left, and runs for the
/// declared bits rounded up to bytes; the standard bounds it to end inside
/// the frame's own region. The information frame's region is left out,
/// since its strip is its own transform. None when a frame's extent cannot
/// be placed, which is the case the scrub refuses.
pub fn ancillary_ranges(bytes: &[u8], l: &Layout) -> Option<Vec<(usize, usize)>> {
    if !l.complete {
        return None;
    }
    // The reservoir stream: every region in order, with its file offset.
    let mut regions: Vec<(usize, usize)> = Vec::new(); // (file offset, len)
    let mut headers = Vec::new();
    let mut p = l.frames_start;
    while p < l.frames_end {
        let h = parse_header(&bytes[p..l.frames_end])?;
        let header_len = 4 + if h.crc { 2 } else { 0 } + h.side_info;
        if h.len < header_len {
            return None;
        }
        regions.push((p + header_len, h.len - header_len));
        headers.push((p, h));
        p += h.len;
    }
    let total: usize = regions.iter().map(|r| r.1).sum();
    let mut covered = vec![false; total];
    let mut pos = 0usize;
    let info_at = l.info.as_ref().map(|i| i.offset);
    for (i, (frame_at, h)) in headers.iter().enumerate() {
        let region_len = regions[i].1;
        if Some(*frame_at) != info_at {
            let frame = &bytes[*frame_at..*frame_at + h.len];
            let mdb = main_data_begin(frame, h)? as usize;
            let nbits = main_data_bits(frame, h)? as usize;
            let nbytes = nbits.div_ceil(8);
            let start = pos.checked_sub(mdb)?;
            let end = start + nbytes;
            if end > pos + region_len {
                return None;
            }
            for c in covered.iter_mut().take(end).skip(start) {
                *c = true;
            }
        }
        pos += region_len;
    }
    // Uncovered bytes, mapped back to file offsets, outside the information
    // frame's region.
    let mut out = Vec::new();
    let mut pos = 0usize;
    for (i, (offset, len)) in regions.iter().enumerate() {
        let is_info = Some(headers[i].0) == info_at;
        if !is_info {
            let mut run_start: Option<usize> = None;
            for k in 0..*len {
                let free = !covered[pos + k];
                match (free, run_start) {
                    (true, None) => run_start = Some(k),
                    (false, Some(s)) => {
                        out.push((offset + s, k - s));
                        run_start = None;
                    }
                    _ => {}
                }
            }
            if let Some(s) = run_start {
                out.push((offset + s, len - s));
            }
        }
        pos += len;
    }
    Some(out)
}

/// Encoder names seen in ancillary bytes.
const ANCILLARY_NAMES: &[&[u8]] = &[b"LAME", b"Lavc", b"Lavf", b"GOGO", b"Xing"];

pub fn scan(bytes: &[u8]) -> (Vec<Detection>, bool) {
    let l = layout(bytes);
    let mut out = Vec::new();
    let mut id3 = Detection::present("id3", "ID3 tag", Honesty::Confirmable);
    for (offset, length) in &l.id3v2 {
        id3.locations.push(Location {
            container: "ID3v2".to_string(),
            offset: *offset,
            length: *length,
            detail: "leading tag".to_string(),
        });
        let body = bytes.get(offset + 10..offset + length).unwrap_or(&[]);
        for frame in ["PRIV", "GEOB", "TSSE", "TENC", "TXXX"] {
            if super::c2pa::contains(body, frame.as_bytes()) {
                id3.evidence.push(format!("{frame} frame"));
            }
        }
    }
    if let Some(at) = l.id3v1 {
        id3.locations.push(Location {
            container: "ID3v1".to_string(),
            offset: at,
            length: 128,
            detail: "trailing tag".to_string(),
        });
    }
    if !id3.locations.is_empty() {
        out.push(id3);
    }
    if !l.ape.is_empty() {
        let mut ape = Detection::present("ape", "APE tag", Honesty::Confirmable);
        for (offset, length) in &l.ape {
            ape.locations.push(Location {
                container: "APE".to_string(),
                offset: *offset,
                length: *length,
                detail: if *offset < l.frames_start {
                    "leading tag".to_string()
                } else {
                    "trailing tag".to_string()
                },
            });
        }
        out.push(ape);
    }
    if let Some(info) = &l.info {
        if info.identity {
            let mut ident = Detection::present(
                "mp3_info",
                "information frame identity",
                Honesty::Confirmable,
            );
            ident.locations.push(Location {
                container: "MPEG frame".to_string(),
                offset: info.offset,
                length: info.len,
                detail: match &info.encoder {
                    Some(e) => format!("{} tag, encoder {e}", info.kind),
                    None => format!("{} tag", info.kind),
                },
            });
            if let Some((d, p)) = info.delay_padding {
                ident.evidence.push(format!("delay {d}, padding {p}"));
            }
            if info.has_toc {
                ident.evidence.push("toc".to_string());
            }
            out.push(ident);
        }
    }
    let complete = l.complete;
    match ancillary_ranges(bytes, &l) {
        Some(ranges) => {
            let nonzero: usize = ranges
                .iter()
                .map(|(at, len)| bytes[*at..*at + *len].iter().filter(|b| **b != 0).count())
                .sum();
            if nonzero > 0 {
                let mut anc = Detection::present(
                    "mp3_ancillary",
                    "MPEG ancillary data",
                    Honesty::Confirmable,
                );
                let total: usize = ranges.iter().map(|r| r.1).sum();
                anc.locations.push(Location {
                    container: "MPEG frame".to_string(),
                    offset: ranges[0].0,
                    length: total,
                    detail: format!("{nonzero} nonzero bytes across {} runs", ranges.len()),
                });
                for name in ANCILLARY_NAMES {
                    let hits = ranges
                        .iter()
                        .filter(|(at, len)| super::c2pa::contains(&bytes[*at..*at + *len], name))
                        .count();
                    if hits > 0 {
                        anc.evidence.push(format!(
                            "{} in {hits} frames",
                            String::from_utf8_lossy(name)
                        ));
                    }
                }
                out.push(anc);
            }
        }
        None if complete => {
            out.push(Detection::with_state(
                "mp3_ancillary",
                "MPEG ancillary data",
                Honesty::Confirmable,
                ScanState::Malformed,
            ));
        }
        None => {}
    }
    if !complete {
        for d in out.iter_mut() {
            if d.class == "mp3_info" {
                d.state = ScanState::Malformed;
            }
        }
    }
    (out, complete)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_layer_three_header_gives_its_length_and_side_info() {
        // MPEG-1 layer III, no CRC, 128 kbps, 44100 Hz, no padding, stereo.
        let h = parse_header(&[0xFF, 0xFB, 0x90, 0x00]).unwrap();
        assert_eq!(h.version, Version::Mpeg1);
        assert!(!h.crc);
        assert_eq!((h.bitrate_kbps, h.sample_rate), (128, 44100));
        assert_eq!(h.len, 417);
        assert_eq!(h.side_info, 32);
        // Padded mono frame.
        let h = parse_header(&[0xFF, 0xFB, 0x92, 0xC0]).unwrap();
        assert_eq!(h.len, 418);
        assert_eq!(h.side_info, 17);
        // MPEG-2, 64 kbps, 22050 Hz, mono: 72000*64/22050 = 208.
        let h = parse_header(&[0xFF, 0xF3, 0x80, 0xC0]).unwrap();
        assert_eq!(h.version, Version::Mpeg2);
        assert_eq!((h.len, h.side_info), (208, 9));
        // Free format, reserved version, and layer II are refused.
        assert!(parse_header(&[0xFF, 0xFB, 0x00, 0x00]).is_none());
        assert!(parse_header(&[0xFF, 0xEB, 0x90, 0x00]).is_none());
        assert!(parse_header(&[0xFF, 0xFD, 0x90, 0x00]).is_none());
    }

    #[test]
    fn main_data_begin_reads_nine_or_eight_bits() {
        let h = parse_header(&[0xFF, 0xFB, 0x90, 0x00]).unwrap();
        let mut frame = vec![0xFF, 0xFB, 0x90, 0x00];
        frame.extend_from_slice(&[0b1010_1010, 0b1000_0000]);
        assert_eq!(main_data_begin(&frame, &h), Some(0b1_0101_0101));
        let h2 = parse_header(&[0xFF, 0xF3, 0x80, 0xC0]).unwrap();
        let mut frame = vec![0xFF, 0xF3, 0x80, 0xC0];
        frame.extend_from_slice(&[0x2B, 0x00]);
        assert_eq!(main_data_begin(&frame, &h2), Some(0x2B));
    }

    #[test]
    fn main_data_bits_sums_part2_3_length_over_granules_and_channels() {
        // MPEG-1 mono: side info 17 bytes; mdb 9 bits, private 5, scfsi 4,
        // then granule 0 part2_3_length at bit 18 and granule 1 at bit 77.
        let h = parse_header(&[0xFF, 0xFB, 0x92, 0xC0]).unwrap();
        let mut side = vec![0u8; 17];
        let set = |side: &mut Vec<u8>, at: usize, v: u32| {
            for i in 0..12 {
                let bit = at + i;
                if (v >> (11 - i)) & 1 == 1 {
                    side[bit / 8] |= 1 << (7 - bit % 8);
                }
            }
        };
        set(&mut side, 18, 1000);
        set(&mut side, 77, 24);
        let mut frame = vec![0xFF, 0xFB, 0x92, 0xC0];
        frame.extend_from_slice(&side);
        assert_eq!(main_data_bits(&frame, &h), Some(1024));
    }

    #[test]
    fn an_ape_footer_locates_its_tag_and_a_bad_walk_is_incomplete() {
        let mut tag = Vec::new();
        let item = b"\x05\x00\x00\x00\x00\x00\x00\x00Tool\x00ABCDE";
        tag.extend_from_slice(item);
        let size = (item.len() + 32) as u32;
        let mut footer = Vec::new();
        footer.extend_from_slice(APE_PREAMBLE);
        footer.extend_from_slice(&2000u32.to_le_bytes());
        footer.extend_from_slice(&size.to_le_bytes());
        footer.extend_from_slice(&1u32.to_le_bytes());
        footer.extend_from_slice(&0u32.to_le_bytes());
        footer.extend_from_slice(&[0u8; 8]);
        tag.extend_from_slice(&footer);
        let mut file = vec![0xFF, 0xFB, 0x90, 0x00];
        file.resize(417, 0);
        let frames_len = file.len();
        file.extend_from_slice(&tag);
        let l = layout(&file);
        assert_eq!(l.ape, vec![(frames_len, tag.len())]);
        assert_eq!((l.frames_start, l.frames_end), (0, frames_len));
        assert!(l.complete);
        // A truncated last frame leaves the walk incomplete.
        let l = layout(&file[..frames_len - 1]);
        assert!(!l.complete);
    }
}
