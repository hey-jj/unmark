//! A lightweight TIFF/EXIF reader. It extracts the fields the report and the
//! camera-origin heuristic need and does not decode image data.

/// What an EXIF payload told us. The presence flags feed guardrail G3.
#[derive(Clone, Debug, Default)]
pub struct ExifFacts {
    pub make: Option<String>,
    pub model: Option<String>,
    pub software: Option<String>,
    pub has_exposure: bool,
    pub has_aperture: bool,
    pub has_iso: bool,
    pub has_lens: bool,
}

impl ExifFacts {
    /// The shutter, aperture, and ISO triple that marks a capture.
    pub fn has_exposure_triple(&self) -> bool {
        self.has_exposure && self.has_aperture && self.has_iso
    }
}

const TAG_MAKE: u16 = 0x010F;
const TAG_MODEL: u16 = 0x0110;
const TAG_SOFTWARE: u16 = 0x0131;
const TAG_EXIF_IFD: u16 = 0x8769;
const TAG_EXPOSURE_TIME: u16 = 0x829A;
const TAG_FNUMBER: u16 = 0x829D;
const TAG_ISO: u16 = 0x8827;
const TAG_FOCAL_LENGTH: u16 = 0x920A;
const TAG_LENS_MODEL: u16 = 0xA434;

/// Parse a TIFF block that begins at the byte order marker (`II` or `MM`).
/// Returns None when the header does not parse. Never panics.
pub fn parse_tiff(b: &[u8]) -> Option<ExifFacts> {
    let le = match b.get(0..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let magic = read_u16(b, 2, le)?;
    if magic != 42 {
        return None;
    }
    let ifd0 = read_u32(b, 4, le)? as usize;
    let mut facts = ExifFacts::default();
    let mut exif_ifd_off: Option<usize> = None;
    read_ifd(b, ifd0, le, &mut facts, &mut exif_ifd_off, false);
    if let Some(off) = exif_ifd_off {
        read_ifd(b, off, le, &mut facts, &mut None, true);
    }
    Some(facts)
}

fn read_ifd(
    b: &[u8],
    off: usize,
    le: bool,
    facts: &mut ExifFacts,
    exif_ifd_off: &mut Option<usize>,
    in_exif: bool,
) {
    let Some(count) = read_u16(b, off, le) else {
        return;
    };
    let mut p = off + 2;
    for _ in 0..count {
        let Some(tag) = read_u16(b, p, le) else {
            return;
        };
        let typ = read_u16(b, p + 2, le).unwrap_or(0);
        let num = read_u32(b, p + 4, le).unwrap_or(0);
        let value_off = p + 8;
        match tag {
            TAG_MAKE if !in_exif => facts.make = read_ascii(b, value_off, typ, num, le),
            TAG_MODEL if !in_exif => facts.model = read_ascii(b, value_off, typ, num, le),
            TAG_SOFTWARE if !in_exif => facts.software = read_ascii(b, value_off, typ, num, le),
            TAG_EXIF_IFD if !in_exif => {
                *exif_ifd_off = read_u32(b, value_off, le).map(|v| v as usize)
            }
            TAG_EXPOSURE_TIME => facts.has_exposure = true,
            TAG_FNUMBER => facts.has_aperture = true,
            TAG_ISO => facts.has_iso = true,
            TAG_FOCAL_LENGTH | TAG_LENS_MODEL => facts.has_lens = true,
            _ => {}
        }
        p += 12;
    }
}

/// Read an ASCII field. When it fits in the four inline bytes it is inline,
/// otherwise the four bytes are an offset into the TIFF block.
fn read_ascii(b: &[u8], value_off: usize, typ: u16, num: u32, le: bool) -> Option<String> {
    if typ != 2 || num == 0 {
        return None;
    }
    let len = num as usize;
    let bytes = if len <= 4 {
        b.get(value_off..value_off + len)?
    } else {
        let off = read_u32(b, value_off, le)? as usize;
        b.get(off..off + len)?
    };
    let s: String = bytes
        .iter()
        .take_while(|c| **c != 0)
        .filter(|c| c.is_ascii_graphic() || **c == b' ')
        .map(|c| *c as char)
        .collect();
    let s = s.trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn read_u16(b: &[u8], at: usize, le: bool) -> Option<u16> {
    let s = b.get(at..at + 2)?;
    Some(if le {
        u16::from_le_bytes([s[0], s[1]])
    } else {
        u16::from_be_bytes([s[0], s[1]])
    })
}

fn read_u32(b: &[u8], at: usize, le: bool) -> Option<u32> {
    let s = b.get(at..at + 4)?;
    Some(if le {
        u32::from_le_bytes([s[0], s[1], s[2], s[3]])
    } else {
        u32::from_be_bytes([s[0], s[1], s[2], s[3]])
    })
}
