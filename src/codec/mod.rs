//! Pixel and sample codecs behind the degrade transforms. Every decoder lands
//! in one of two in-memory forms: an interleaved 8-bit RGB or RGBA image, or
//! per-channel f64 audio normalized to [-1, 1). Every encoder setting is
//! pinned here rather than left at a library default, and
//! `crate::encoder_fingerprint` names each codec crate and version.
//!
//! The metadata tier never touches this module. A container rewrite copies
//! bytes and never decodes, which is what lets it prove byte identity.

#[cfg(feature = "audio")]
pub mod flac;
#[cfg(feature = "image")]
pub mod jpeg;
#[cfg(feature = "audio")]
pub mod mp3;
#[cfg(feature = "image")]
pub mod png;
#[cfg(feature = "audio")]
pub mod wav;
#[cfg(feature = "image")]
pub mod webp;

use crate::asset::Format;

/// Why a decode or encode could not complete.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CodecError {
    /// The bytes are a format or a variant this build does not decode.
    Unsupported(String),
    /// The bytes would not decode.
    Malformed(String),
    /// A pinned setting is one the encoder here cannot honor.
    Setting(String),
}

impl std::fmt::Display for CodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodecError::Unsupported(m) => write!(f, "unsupported: {m}"),
            CodecError::Malformed(m) => write!(f, "malformed: {m}"),
            CodecError::Setting(m) => write!(f, "setting: {m}"),
        }
    }
}

impl std::error::Error for CodecError {}

/// An interleaved 8-bit image, three channels for RGB or four for RGBA.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub channels: usize,
    pub data: Vec<u8>,
}

impl Image {
    pub fn has_alpha(&self) -> bool {
        self.channels == 4
    }

    pub fn long_edge(&self) -> usize {
        self.width.max(self.height)
    }

    /// BT.601 luma per pixel as f64, unrounded.
    pub fn luma(&self) -> Vec<f64> {
        let c = self.channels;
        self.data
            .chunks_exact(c)
            .map(|p| 0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64)
            .collect()
    }

    /// Expand to four channels with opaque alpha, or return a clone.
    pub fn with_alpha(&self) -> Image {
        if self.channels == 4 {
            return self.clone();
        }
        let mut data = Vec::with_capacity(self.width * self.height * 4);
        for p in self.data.as_chunks::<3>().0 {
            data.extend_from_slice(p);
            data.push(255);
        }
        Image {
            width: self.width,
            height: self.height,
            channels: 4,
            data,
        }
    }
}

/// Per-channel f64 audio normalized so the integer full scale is 1.0.
#[derive(Clone, Debug, PartialEq)]
pub struct Audio {
    pub rate: u32,
    /// Bit depth of the container the samples came from or go to. 32 with
    /// `float` set means IEEE float32.
    pub bits: u16,
    /// True when the emitted sample format is float32 rather than integer
    /// PCM. Recorded as emitted, never upconverted.
    pub float: bool,
    pub channels: Vec<Vec<f64>>,
}

impl Audio {
    /// The emitted sample format's name: `int16`, `int24`, or `float32`.
    pub fn sample_format(&self) -> String {
        if self.float {
            "float32".to_string()
        } else {
            format!("int{}", self.bits)
        }
    }

    /// The decoded content as pinned for hashing: PCM interleaved
    /// little-endian in the emitted sample format.
    pub fn interleaved_le_bytes(&self) -> Vec<u8> {
        let n = self.frames();
        let mut out = Vec::with_capacity(n * self.channels.len() * 4);
        if self.float {
            for i in 0..n {
                for c in &self.channels {
                    out.extend_from_slice(&(c[i] as f32).to_le_bytes());
                }
            }
            return out;
        }
        let bytes_per = (self.bits as usize).div_ceil(8);
        for s in self.quantize(self.bits) {
            out.extend_from_slice(&s.to_le_bytes()[..bytes_per]);
        }
        out
    }

    pub fn frames(&self) -> usize {
        self.channels.first().map(|c| c.len()).unwrap_or(0)
    }

    pub fn duration_s(&self) -> f64 {
        self.frames() as f64 / self.rate as f64
    }

    /// Mono downmix by the mean over channels.
    pub fn downmix(&self) -> Vec<f64> {
        let n = self.frames();
        let k = self.channels.len() as f64;
        (0..n)
            .map(|i| self.channels.iter().map(|c| c[i]).sum::<f64>() / k)
            .collect()
    }

    /// Integer samples at `bits`, interleaved, rounded half away from zero
    /// and clamped to the container's range.
    pub fn quantize(&self, bits: u16) -> Vec<i32> {
        let full = (1i64 << (bits - 1)) as f64;
        let lo = -(1i64 << (bits - 1));
        let hi = (1i64 << (bits - 1)) - 1;
        let n = self.frames();
        let mut out = Vec::with_capacity(n * self.channels.len());
        for i in 0..n {
            for c in &self.channels {
                let v = (c[i] * full).round() as i64;
                out.push(v.clamp(lo, hi) as i32);
            }
        }
        out
    }

    /// Build from interleaved integer samples.
    pub fn from_interleaved(samples: &[i32], channels: usize, bits: u16, rate: u32) -> Audio {
        let full = (1i64 << (bits - 1)) as f64;
        let frames = samples.len() / channels;
        let mut chans = vec![Vec::with_capacity(frames); channels];
        for (i, s) in samples.iter().enumerate() {
            chans[i % channels].push(*s as f64 / full);
        }
        Audio {
            rate,
            bits,
            float: false,
            channels: chans,
        }
    }
}

/// Decode an image container into RGB or RGBA.
#[cfg(feature = "image")]
pub fn decode_image(bytes: &[u8], format: Format) -> Result<Image, CodecError> {
    match format {
        Format::Png => png::decode(bytes),
        Format::Jpeg => jpeg::decode(bytes),
        Format::WebP => webp::decode(bytes),
        other => Err(CodecError::Unsupported(format!(
            "{} is not an image container this build decodes",
            other.as_str()
        ))),
    }
}

/// Decode an audio container into normalized samples.
#[cfg(feature = "audio")]
pub fn decode_audio(bytes: &[u8], format: Format) -> Result<Audio, CodecError> {
    match format {
        Format::RiffWav => wav::decode(bytes),
        Format::Flac => flac::decode(bytes),
        Format::Mp3 => mp3::decode(bytes),
        other => Err(CodecError::Unsupported(format!(
            "{} is not an audio container this build decodes",
            other.as_str()
        ))),
    }
}

/// Encode normalized samples into the container at `bits`.
#[cfg(feature = "audio")]
pub fn encode_audio(audio: &Audio, format: Format, bits: u16) -> Result<Vec<u8>, CodecError> {
    match format {
        Format::RiffWav => wav::encode(audio, bits),
        Format::Flac => flac::encode(audio, bits),
        other => Err(CodecError::Unsupported(format!(
            "{} is not an audio container this build encodes",
            other.as_str()
        ))),
    }
}
