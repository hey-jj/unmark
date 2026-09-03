//! The pixel path of the default run: decode, resize at the pinned ratio
//! (the removal path for the dwtDct mark), and write the same container. A
//! JPEG input has no lossless write, so it comes back as a baseline JPEG at
//! the pinned quality from the crate's own encoder; a PNG or WebP input is
//! written losslessly in its own container. The container choice sits behind
//! an `Emitter`, one seam.
//!
//! The grid-matched reference for the sanity floor is the input resampled by
//! the same Lanczos3 path, so the floor measures the encode alone.

use crate::asset::Format;
use crate::codec::{self, CodecError, Image};
use crate::dsp;
use crate::policy::PolicyPackage;

/// The pinned parameters of the pixel run, read from the policy package.
#[derive(Clone, Debug, PartialEq)]
pub struct PixelParams {
    /// PX02 edge ratio, when the plan carries it.
    pub resize_ratio: Option<f64>,
    pub jpeg_quality: u32,
    pub jpeg_chroma: String,
}

impl PixelParams {
    pub fn from_policy(pkg: &PolicyPackage, ids: &[String]) -> Result<PixelParams, String> {
        let px01 = pkg.transform("PX01").ok_or("policy has no PX01")?;
        let jpeg_quality = px01.param_i64("jpeg_quality").ok_or("PX01 jpeg_quality")? as u32;
        let jpeg_chroma = px01
            .param_str("jpeg_chroma")
            .ok_or("PX01 jpeg_chroma")?
            .to_string();
        let resize_ratio = if ids.iter().any(|i| i == "PX02") {
            let t = pkg.transform("PX02").ok_or("policy has no PX02")?;
            Some(t.param_f64("ratio").ok_or("PX02 ratio")?)
        } else {
            None
        };
        Ok(PixelParams {
            resize_ratio,
            jpeg_quality,
            jpeg_chroma,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PixelError {
    Codec(CodecError),
    Params(String),
}

impl std::fmt::Display for PixelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PixelError::Codec(e) => write!(f, "codec: {e}"),
            PixelError::Params(m) => write!(f, "params: {m}"),
        }
    }
}

impl From<CodecError> for PixelError {
    fn from(e: CodecError) -> Self {
        PixelError::Codec(e)
    }
}

/// PX02: both edges by `ratio`, rounded half up, never below one pixel.
pub fn resize(img: &Image, ratio: f64) -> Image {
    let nw = ((img.width as f64 * ratio + 0.5).floor() as usize).max(1);
    let nh = ((img.height as f64 * ratio + 0.5).floor() as usize).max(1);
    let data = dsp::resample_lanczos3(&img.data, img.width, img.height, img.channels, nw, nh);
    Image {
        width: nw,
        height: nh,
        channels: img.channels,
        data,
    }
}

/// What an emitter wrote.
#[derive(Clone, Debug)]
pub struct Emitted {
    pub bytes: Vec<u8>,
    pub format: Format,
    /// True when a lossy encode happened.
    pub reencoded: bool,
    /// The pin the encode ran at, for the report.
    pub pin: Option<String>,
}

/// The container choice is one seam. A future lossy encoder slots in here
/// without touching the plan or the metrics.
pub trait Emitter {
    fn emit(&self, img: &Image, input: Format, p: &PixelParams) -> Result<Emitted, PixelError>;
}

/// The 0.2.0 rule: a JPEG input comes back as a baseline JPEG at the pinned
/// quality; a PNG or WebP input is written losslessly in its own container,
/// alpha kept.
pub struct DefaultEmitter;

impl Emitter for DefaultEmitter {
    fn emit(&self, img: &Image, input: Format, p: &PixelParams) -> Result<Emitted, PixelError> {
        match input {
            Format::Jpeg => Ok(Emitted {
                bytes: codec::jpeg::encode(img, p.jpeg_quality, &p.jpeg_chroma)?,
                format: Format::Jpeg,
                reencoded: true,
                pin: Some(format!(
                    "quality {}, {} chroma, jpeg-encoder in-crate {}",
                    p.jpeg_quality,
                    p.jpeg_chroma,
                    codec::jpeg::ENCODER_VERSION
                )),
            }),
            Format::Png => Ok(Emitted {
                bytes: codec::png::encode(img)?,
                format: Format::Png,
                reencoded: false,
                pin: None,
            }),
            Format::WebP => Ok(Emitted {
                bytes: codec::webp::encode_lossless(img)?,
                format: Format::WebP,
                reencoded: false,
                pin: None,
            }),
            other => Err(PixelError::Codec(CodecError::Unsupported(format!(
                "{} is not a pixel container",
                other.as_str()
            )))),
        }
    }
}

pub fn encode_output(img: &Image, format: Format, p: &PixelParams) -> Result<Emitted, PixelError> {
    DefaultEmitter.emit(img, format, p)
}

/// A full pixel run: the written output, the grid-matched reference, and the
/// decoded output.
#[derive(Clone, Debug)]
pub struct PixelOutcome {
    pub emitted: Emitted,
    pub reference: Image,
    pub output: Image,
}

pub fn apply(input: &Image, format: Format, p: &PixelParams) -> Result<PixelOutcome, PixelError> {
    let reference = match p.resize_ratio {
        Some(r) => resize(input, r),
        None => input.clone(),
    };
    let emitted = encode_output(&reference, format, p)?;
    let output = codec::decode_image(&emitted.bytes, emitted.format)?;
    Ok(PixelOutcome {
        emitted,
        reference,
        output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(w: usize, h: usize, c: usize) -> Image {
        Image {
            width: w,
            height: h,
            channels: c,
            data: (0..w * h * c).map(|i| (i * 31 % 256) as u8).collect(),
        }
    }

    fn params(ratio: Option<f64>) -> PixelParams {
        PixelParams {
            resize_ratio: ratio,
            jpeg_quality: 92,
            jpeg_chroma: "4:4:4".to_string(),
        }
    }

    #[test]
    fn resize_rounds_half_up_and_keeps_channels() {
        let a = img(101, 51, 4);
        let r = resize(&a, 0.5);
        assert_eq!((r.width, r.height, r.channels), (51, 26, 4));
    }

    #[test]
    fn the_emitter_keeps_the_container_and_alpha() {
        let alpha = img(16, 16, 4);
        let e = encode_output(&alpha, Format::Png, &params(None)).unwrap();
        assert_eq!(e.format, Format::Png);
        assert!(!e.reencoded);
        assert_eq!(codec::png::decode(&e.bytes).unwrap().channels, 4);
        let e = encode_output(&alpha, Format::WebP, &params(None)).unwrap();
        assert_eq!(e.format, Format::WebP);
        let e = encode_output(&img(16, 16, 3), Format::Jpeg, &params(None)).unwrap();
        assert_eq!(e.format, Format::Jpeg);
        assert!(e.reencoded && e.pin.as_deref().unwrap().contains("quality 92"));
    }
}
