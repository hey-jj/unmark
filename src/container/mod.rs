//! Byte-level container rewriters. A rewriter drops a metadata structure and
//! copies every other byte, so the encoded pixel or sample stream comes out
//! identical. This path never decodes and never shares code with a transform
//! that re-encodes, which is what lets the metadata tier prove its claims.

pub mod flac;
pub mod id3;
pub mod jpeg;
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
    /// MC06: drop ancillary chunks not on the preservation keep list.
    pub unlisted: bool,
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
        Format::Mp3 => Ok(id3::rewrite(bytes, spec)),
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
        Format::Mp3 => id3::signal_stream(bytes),
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
