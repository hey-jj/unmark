//! Container sniffing and format identity. The sniff reads magic bytes only
//! and never decodes pixel or sample data.

/// A container format unmark can name. The supported-container set below is the
/// enumerated list over which a scan may report `confirmed_absent`. A format
/// outside it can still be sniffed and named, and every mark class in it
/// resolves to `unsupported_format`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Format {
    Jpeg,
    Png,
    WebP,
    /// A RIFF WAV audio container.
    RiffWav,
    /// A RIFF AVI container. Sniffed and named, held out of the supported set.
    RiffAvi,
    /// An ISO base media file (MP4, M4A, MOV).
    Isobmff,
    /// A standalone MP3 or other frame stream that may carry ID3 tags.
    Mp3,
    Flac,
    Ogg,
    Svg,
    Html,
    /// Any other text file, scanned only for invisible Unicode.
    Text,
    /// Bytes unmark cannot identify.
    Unknown,
}

/// The enumerated set of formats over which a scan is exhaustive enough to
/// license the word absent. This is the single source of the honesty property:
/// `confirmed_absent` is reachable only for a format in this constant, and
/// every other format resolves to `unsupported_format`. The three text formats
/// are members because a text file is read exhaustively by code point. Widening
/// coverage means adding to this list and nothing else.
pub const SUPPORTED_CONTAINERS: &[Format] = &[
    Format::Jpeg,
    Format::Png,
    Format::WebP,
    Format::RiffWav,
    Format::Isobmff,
    Format::Mp3,
    Format::Flac,
    Format::Svg,
    Format::Html,
    Format::Text,
];

impl Format {
    pub fn as_str(self) -> &'static str {
        match self {
            Format::Jpeg => "jpeg",
            Format::Png => "png",
            Format::WebP => "webp",
            Format::RiffWav => "riff-wav",
            Format::RiffAvi => "riff-avi",
            Format::Isobmff => "isobmff",
            Format::Mp3 => "mp3",
            Format::Flac => "flac",
            Format::Ogg => "ogg",
            Format::Svg => "svg",
            Format::Html => "html",
            Format::Text => "text",
            Format::Unknown => "unknown",
        }
    }

    /// True when the format is in the enumerated supported-container set.
    pub fn is_supported_container(self) -> bool {
        SUPPORTED_CONTAINERS.contains(&self)
    }

    /// True when the format is a text file read by code point. The text formats
    /// are members of `SUPPORTED_CONTAINERS`. That constant determines which
    /// formats allow an absence report. This predicate only identifies text.
    pub fn is_text(self) -> bool {
        matches!(self, Format::Svg | Format::Html | Format::Text)
    }

    /// The media axis a profile targets.
    pub fn media(self) -> Media {
        match self {
            Format::Jpeg | Format::Png | Format::WebP => Media::Image,
            Format::RiffWav | Format::Mp3 | Format::Flac | Format::Ogg => Media::Audio,
            Format::Isobmff => Media::Audio,
            Format::RiffAvi => Media::Video,
            Format::Svg | Format::Html | Format::Text => Media::Files,
            Format::Unknown => Media::Unknown,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Media {
    Image,
    Audio,
    Video,
    Files,
    Unknown,
}

/// Identify a container from its leading bytes. Handles short input without panicking.
pub fn sniff(bytes: &[u8]) -> Format {
    if bytes.len() >= 8 && bytes[..8] == [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'] {
        return Format::Png;
    }
    if bytes.len() >= 3 && bytes[..3] == [0xFF, 0xD8, 0xFF] {
        return Format::Jpeg;
    }
    if bytes.len() >= 12 && &bytes[..4] == b"RIFF" {
        match &bytes[8..12] {
            b"WEBP" => return Format::WebP,
            b"WAVE" => return Format::RiffWav,
            b"AVI " => return Format::RiffAvi,
            _ => {}
        }
    }
    if bytes.len() >= 4 && &bytes[..4] == b"fLaC" {
        return Format::Flac;
    }
    if bytes.len() >= 4 && &bytes[..4] == b"OggS" {
        return Format::Ogg;
    }
    // ISOBMFF: a leading box whose type is `ftyp`.
    if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
        return Format::Isobmff;
    }
    // A standalone MP3: an ID3v2 header, or an MPEG audio frame sync.
    if bytes.len() >= 3 && &bytes[..3] == b"ID3" {
        return Format::Mp3;
    }
    if bytes.len() >= 2 && bytes[0] == 0xFF && (bytes[1] & 0xE0) == 0xE0 {
        return Format::Mp3;
    }
    sniff_text(bytes)
}

fn sniff_text(bytes: &[u8]) -> Format {
    // Only classify as text when the bytes are valid UTF-8. A binary format
    // unmark does not know stays Unknown and resolves to unsupported_format.
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Format::Unknown;
    };
    let head: String = text.chars().take(512).collect();
    let lower = head.trim_start().to_ascii_lowercase();
    if lower.starts_with("<?xml") && lower.contains("<svg") {
        return Format::Svg;
    }
    if lower.starts_with("<svg") {
        return Format::Svg;
    }
    if lower.starts_with("<!doctype html") || lower.starts_with("<html") {
        return Format::Html;
    }
    Format::Text
}
