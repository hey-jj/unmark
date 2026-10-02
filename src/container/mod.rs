//! Byte-level container rewriters. A rewriter drops a metadata structure and
//! copies every other byte, so the encoded pixel or sample stream comes out
//! identical. This path never decodes and never shares code with a transform
//! that re-encodes, which is what lets the metadata tier prove its claims.

pub mod flac;
pub mod id3;
pub mod jpeg;
pub mod mp3;
pub mod mp4;
pub mod png;
pub mod riff;

use crate::asset::Format;

/// Which metadata structures a rewrite drops. Each flag maps to one confirmable
/// mark class. The rewriter reads only the flags relevant to its format.
#[derive(Clone, Copy, Debug, Default)]
pub struct DropSpec {
    pub c2pa: bool,
    pub xmp: bool,
    pub exif: bool,
    pub png_text: bool,
    pub iptc: bool,
    pub id3: bool,
    pub ilst: bool,
    pub riff_ancillary: bool,
    /// MC06: drop ancillary chunks not on the keep list.
    pub unlisted: bool,
    /// MC10: drop the FLAC Vorbis comment block whole.
    pub vorbis: bool,
    /// MC11: drop JPEG COM and application segments decoding does not need.
    pub jpeg_segments: bool,
    /// MC12: drop WAV production metadata chunks (bext, iXML, aXML, _PMX,
    /// cue, smpl, inst).
    pub riff_production: bool,
    /// MC13: drop APE tags from an MPEG audio file.
    pub ape: bool,
    /// MC14: rewrite the Xing, Info, or VBRI information frame in place as
    /// its minimal form, the encoder identity zeroed.
    pub info_identity: bool,
    /// MC15: zero the ancillary bytes of MPEG audio frames.
    pub ancillary: bool,
}

/// The reason a rewrite could not be completed for a container in this build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RewriteError {
    /// The container is one this build does not rewrite.
    Unsupported(String),
    /// The structure would not parse safely.
    Malformed(String),
    /// A targeted class cannot be stripped from this container in this build.
    /// Carries the mark class that was declined so the caller attributes the
    /// decline to the right transform rather than to a hardcoded one.
    Declined { class: String, reason: String },
}

impl std::fmt::Display for RewriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RewriteError::Unsupported(m) => write!(f, "unsupported container: {m}"),
            RewriteError::Malformed(m) => write!(f, "malformed container: {m}"),
            RewriteError::Declined { class, reason } => {
                write!(f, "declined stripping {class}: {reason}")
            }
        }
    }
}

/// Rewrite the container, dropping the structures named in `spec`. Returns the
/// new bytes. The pixel or sample stream is preserved by construction.
pub fn rewrite(bytes: &[u8], format: Format, spec: &DropSpec) -> Result<Vec<u8>, RewriteError> {
    match format {
        Format::Png => png::rewrite(bytes, spec),
        Format::Jpeg => Ok(jpeg::rewrite(bytes, spec)),
        Format::WebP | Format::RiffWav | Format::RiffAvi => riff::rewrite(bytes, spec),
        Format::Isobmff => mp4::rewrite(bytes, spec),
        Format::Mp3 => mp3::rewrite(bytes, spec),
        Format::Flac => flac::rewrite(bytes, spec),
        _ if format.is_text() => Ok(bytes.to_vec()),
        other => Err(RewriteError::Unsupported(other.as_str().to_string())),
    }
}

/// The stable signal the metadata tier promises to leave untouched, compared
/// between input and output. For a binary container it is the encoded pixel or
/// sample stream. For a text file it is the visible content: the text with every
/// invisible code point and every generator-header line removed, so a strip that
/// deletes only those spans leaves the signal identical, while any change to
/// visible content fails the gate.
pub fn signal_stream(bytes: &[u8], format: Format) -> Vec<u8> {
    match format {
        Format::Png => png::signal_stream(bytes),
        Format::Jpeg => jpeg::signal_stream(bytes),
        Format::WebP | Format::RiffWav | Format::RiffAvi => riff::signal_stream(bytes),
        Format::Isobmff => mp4::signal_stream(bytes),
        Format::Mp3 => mp3::signal_stream(bytes),
        Format::Flac => flac::signal_stream(bytes),
        _ if format.is_text() => text_signal(bytes),
        _ => bytes.to_vec(),
    }
}

/// The visible skeleton of a text file: every invisible code point and every
/// generator-header line removed. Removing the same classes from both input and
/// output normalizes away exactly the spans a text strip is allowed to touch, so
/// the comparison catches only changes to visible content.
fn text_signal(bytes: &[u8]) -> Vec<u8> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return bytes.to_vec();
    };
    // Drop every invisible span (hygiene, wrapper, and malformed alike).
    let spans = crate::detect::invisibles::spans(text);
    let mut visible = String::with_capacity(text.len());
    let mut last = 0;
    for s in spans {
        if s.byte_start >= last {
            visible.push_str(&text[last..s.byte_start]);
            last = s.byte_end;
        }
    }
    visible.push_str(&text[last..]);
    // Drop generator-header lines, the other class a text strip may remove.
    crate::transform::strip_generator_headers(&visible).into_bytes()
}

/// CRC-32/ISO-HDLC, the PNG chunk CRC.
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn png_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 12);
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_input = kind.to_vec();
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
    out
}

/// Carry the ancillary metadata that survived the metadata strips in
/// `source` into `fresh`, a container the pixel path re-encoded from the
/// same image. A `--keep` on a metadata class means the class is present in
/// `source`, so copying every ancillary structure across preserves exactly
/// the kept items. PNG copies every chunk that is not critical to the pixel
/// stream after the fresh IHDR; JPEG copies every APP1 through APP15 and
/// COM segment after the fresh APP0. WebP is carried by the encoder itself.
pub fn carry_ancillary(source: &[u8], fresh: &[u8], format: Format) -> Vec<u8> {
    match format {
        Format::Png => {
            let (src, _) = crate::detect::png_text::chunks(source);
            let carried: Vec<Vec<u8>> = src
                .iter()
                .filter(|c| !matches!(&c.kind, b"IHDR" | b"IDAT" | b"IEND" | b"PLTE" | b"tRNS"))
                .map(|c| png_chunk(&c.kind, c.data))
                .collect();
            if carried.is_empty() {
                return fresh.to_vec();
            }
            let (dst, _) = crate::detect::png_text::chunks(fresh);
            let Some(ihdr) = dst.iter().find(|c| &c.kind == b"IHDR") else {
                return fresh.to_vec();
            };
            let cut = ihdr.start + ihdr.total;
            let mut out = fresh[..cut].to_vec();
            for c in carried {
                out.extend_from_slice(&c);
            }
            out.extend_from_slice(&fresh[cut..]);
            out
        }
        Format::Jpeg => {
            let (src, _) = crate::detect::jpeg::segments(source);
            let carried: Vec<&[u8]> = src
                .iter()
                .filter(|s| (0xE1..=0xEF).contains(&s.marker) || s.marker == 0xFE)
                .filter_map(|s| source.get(s.start..s.start + s.total))
                .collect();
            if carried.is_empty() {
                return fresh.to_vec();
            }
            let (dst, _) = crate::detect::jpeg::segments(fresh);
            // After the fresh APP0 when there is one, else right after SOI.
            let cut = dst
                .iter()
                .find(|s| s.marker == 0xE0)
                .map(|s| s.start + s.total)
                .unwrap_or(2);
            let mut out = fresh[..cut].to_vec();
            for seg in carried {
                out.extend_from_slice(seg);
            }
            out.extend_from_slice(&fresh[cut..]);
            out
        }
        Format::Flac => {
            use crate::detect::flac::{blocks, STREAMINFO, VORBIS_COMMENT};
            let (src, _, ok) = blocks(source);
            if !ok {
                return fresh.to_vec();
            }
            // Padding and the seek table describe the old frames; every
            // other block is metadata the strips left in place.
            let carried: Vec<&[u8]> = src
                .iter()
                .filter(|b| !matches!(b.kind, STREAMINFO | 1 | 3))
                .filter_map(|b| source.get(b.start..b.start + b.total))
                .collect();
            if carried.is_empty() {
                return fresh.to_vec();
            }
            let src_has_vorbis = src.iter().any(|b| b.kind == VORBIS_COMMENT);
            let (dst, audio_start, ok) = blocks(fresh);
            if !ok || dst.is_empty() {
                return fresh.to_vec();
            }
            let mut out_blocks: Vec<Vec<u8>> = dst
                .iter()
                .filter(|b| !(b.kind == VORBIS_COMMENT && src_has_vorbis))
                .filter_map(|b| fresh.get(b.start..b.start + b.total).map(|s| s.to_vec()))
                .collect();
            out_blocks.extend(carried.iter().map(|c| c.to_vec()));
            let last = out_blocks.len() - 1;
            let mut out = b"fLaC".to_vec();
            for (i, b) in out_blocks.iter_mut().enumerate() {
                if i == last {
                    b[0] |= 0x80;
                } else {
                    b[0] &= 0x7F;
                }
                out.extend_from_slice(b);
            }
            out.extend_from_slice(&fresh[audio_start..]);
            out
        }
        Format::Mp3 => {
            // The tags the strips left stand at either end of the frames;
            // the fresh frames go between them.
            let l = crate::detect::mp3::layout(source);
            if !l.complete {
                return fresh.to_vec();
            }
            let mut out = Vec::with_capacity(source.len() + fresh.len());
            out.extend_from_slice(&source[..l.frames_start]);
            out.extend_from_slice(fresh);
            out.extend_from_slice(&source[l.frames_end..]);
            out
        }
        Format::RiffWav => {
            let (src, _) = crate::detect::riff::chunks(source);
            let carried: Vec<&[u8]> = src
                .iter()
                .filter(|c| !matches!(&c.id, b"fmt " | b"data" | b"fact"))
                .filter_map(|c| source.get(c.start..c.start + c.total))
                .collect();
            if carried.is_empty() || fresh.len() < 12 {
                return fresh.to_vec();
            }
            let mut out = fresh.to_vec();
            for c in carried {
                out.extend_from_slice(c);
            }
            let riff_len = (out.len() - 8) as u32;
            out[4..8].copy_from_slice(&riff_len.to_le_bytes());
            out
        }
        _ => fresh.to_vec(),
    }
}

/// The WebP metadata the lossless encoder can carry, plus whether a chunk
/// it cannot carry (a C2PA chunk) is present.
#[derive(Debug, Default, Clone)]
pub struct WebpMetadata {
    pub exif: Option<Vec<u8>>,
    pub xmp: Option<Vec<u8>>,
    pub icc: Option<Vec<u8>>,
    pub uncarried_c2pa: bool,
}

impl WebpMetadata {
    pub fn any_carried(&self) -> bool {
        self.exif.is_some() || self.xmp.is_some() || self.icc.is_some()
    }
}

pub fn webp_metadata(source: &[u8]) -> WebpMetadata {
    let (chunks, _) = crate::detect::riff::chunks(source);
    let mut m = WebpMetadata::default();
    for c in chunks {
        match &c.id {
            b"EXIF" => m.exif = Some(c.data.to_vec()),
            b"XMP " => m.xmp = Some(c.data.to_vec()),
            b"ICCP" => m.icc = Some(c.data.to_vec()),
            b"C2PA" => m.uncarried_c2pa = true,
            _ => {}
        }
    }
    m
}
