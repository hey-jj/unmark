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
    /// PX03 border crop: pixels per edge and the cap as a fraction of the
    /// edge, when the plan carries it.
    pub crop: Option<(usize, f64)>,
    /// PX06 rotation in degrees, when the plan carries it.
    pub rotate_degrees: Option<f64>,
    /// PX07 Gaussian blur sigma in pixels, when the plan carries it.
    pub blur_sigma: Option<f64>,
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
        let crop = if ids.iter().any(|i| i == "PX03") {
            let t = pkg.transform("PX03").ok_or("policy has no PX03")?;
            Some((
                t.param_i64("pixels").ok_or("PX03 pixels")? as usize,
                t.param_f64("cap").ok_or("PX03 cap")?,
            ))
        } else {
            None
        };
        let rotate_degrees = if ids.iter().any(|i| i == "PX06") {
            let t = pkg.transform("PX06").ok_or("policy has no PX06")?;
            Some(t.param_f64("degrees").ok_or("PX06 degrees")?)
        } else {
            None
        };
        let blur_sigma = if ids.iter().any(|i| i == "PX07") {
            let t = pkg.transform("PX07").ok_or("policy has no PX07")?;
            Some(t.param_f64("sigma").ok_or("PX07 sigma")?)
        } else {
            None
        };
        Ok(PixelParams {
            crop,
            rotate_degrees,
            blur_sigma,
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

/// PX03: cut `pixels` from every edge, capped at `cap` of that edge, so a
/// small image keeps most of itself. Never below one pixel on an edge.
pub fn crop_border(img: &Image, pixels: usize, cap: f64) -> Image {
    let cut_w = pixels.min((img.width as f64 * cap).floor() as usize);
    let cut_h = pixels.min((img.height as f64 * cap).floor() as usize);
    let nw = img.width.saturating_sub(2 * cut_w).max(1);
    let nh = img.height.saturating_sub(2 * cut_h).max(1);
    let c = img.channels;
    let mut data = Vec::with_capacity(nw * nh * c);
    for y in 0..nh {
        let row = (y + cut_h) * img.width;
        let start = (row + cut_w) * c;
        data.extend_from_slice(&img.data[start..start + nw * c]);
    }
    Image {
        width: nw,
        height: nh,
        channels: c,
        data,
    }
}

fn sample_bilinear(img: &Image, x: f64, y: f64, ch: usize) -> f64 {
    let xf = x.clamp(0.0, (img.width - 1) as f64);
    let yf = y.clamp(0.0, (img.height - 1) as f64);
    let x0 = xf.floor() as usize;
    let y0 = yf.floor() as usize;
    let x1 = (x0 + 1).min(img.width - 1);
    let y1 = (y0 + 1).min(img.height - 1);
    let fx = xf - x0 as f64;
    let fy = yf - y0 as f64;
    let c = img.channels;
    let at = |xx: usize, yy: usize| img.data[(yy * img.width + xx) * c + ch] as f64;
    let top = at(x0, y0) * (1.0 - fx) + at(x1, y0) * fx;
    let bottom = at(x0, y1) * (1.0 - fx) + at(x1, y1) * fx;
    top * (1.0 - fy) + bottom * fy
}

/// PX06: rotate about the centre by `degrees` on the same canvas, bilinear
/// sampling, the edge clamped where the source falls outside the frame.
pub fn rotate(img: &Image, degrees: f64) -> Image {
    let theta = degrees * std::f64::consts::PI / 180.0;
    let (s, co) = (dsp::sin(theta), dsp::cos(theta));
    let cx = (img.width as f64 - 1.0) / 2.0;
    let cy = (img.height as f64 - 1.0) / 2.0;
    let c = img.channels;
    let mut data = Vec::with_capacity(img.data.len());
    for y in 0..img.height {
        for x in 0..img.width {
            let dx = x as f64 - cx;
            let dy = y as f64 - cy;
            // The inverse rotation finds the source of this output pixel.
            let sx = co * dx + s * dy + cx;
            let sy = -s * dx + co * dy + cy;
            for ch in 0..c {
                let v = sample_bilinear(img, sx, sy, ch);
                data.push(v.round().clamp(0.0, 255.0) as u8);
            }
        }
    }
    Image {
        width: img.width,
        height: img.height,
        channels: c,
        data,
    }
}

/// PX07: a separable Gaussian blur of standard deviation `sigma` pixels,
/// kernel radius three sigma, edges clamped.
pub fn blur(img: &Image, sigma: f64) -> Image {
    if sigma <= 0.0 {
        return img.clone();
    }
    let radius = (3.0 * sigma).ceil() as usize;
    let kernel: Vec<f64> = (0..=2 * radius)
        .map(|i| {
            let d = i as f64 - radius as f64;
            dsp::exp(-(d * d) / (2.0 * sigma * sigma))
        })
        .collect();
    let sum: f64 = kernel.iter().sum();
    let kernel: Vec<f64> = kernel.iter().map(|k| k / sum).collect();
    let (w, h, c) = (img.width, img.height, img.channels);
    let src: Vec<f64> = img.data.iter().map(|&v| v as f64).collect();
    let mut horizontal = vec![0.0f64; src.len()];
    for y in 0..h {
        for x in 0..w {
            for ch in 0..c {
                let mut acc = 0.0;
                for (i, k) in kernel.iter().enumerate() {
                    let sx = (x as i64 + i as i64 - radius as i64).clamp(0, w as i64 - 1) as usize;
                    acc += k * src[(y * w + sx) * c + ch];
                }
                horizontal[(y * w + x) * c + ch] = acc;
            }
        }
    }
    let mut data = Vec::with_capacity(src.len());
    for y in 0..h {
        for x in 0..w {
            for ch in 0..c {
                let mut acc = 0.0;
                for (i, k) in kernel.iter().enumerate() {
                    let sy = (y as i64 + i as i64 - radius as i64).clamp(0, h as i64 - 1) as usize;
                    acc += k * horizontal[(sy * w + x) * c + ch];
                }
                data.push(acc.round().clamp(0.0, 255.0) as u8);
            }
        }
    }
    Image {
        width: w,
        height: h,
        channels: c,
        data,
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

/// The geometric and filtering steps of the run in their order: crop,
/// rotate, blur, resize. The result is the grid-matched reference the
/// encode is measured against.
pub fn degrade(input: &Image, p: &PixelParams) -> Image {
    let mut img = input.clone();
    if let Some((pixels, cap)) = p.crop {
        img = crop_border(&img, pixels, cap);
    }
    if let Some(d) = p.rotate_degrees {
        img = rotate(&img, d);
    }
    if let Some(s) = p.blur_sigma {
        img = blur(&img, s);
    }
    if let Some(r) = p.resize_ratio {
        img = resize(&img, r);
    }
    img
}

pub fn apply(input: &Image, format: Format, p: &PixelParams) -> Result<PixelOutcome, PixelError> {
    let reference = degrade(input, p);
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
            crop: None,
            rotate_degrees: None,
            blur_sigma: None,
            resize_ratio: ratio,
            jpeg_quality: 92,
            jpeg_chroma: "4:4:4".to_string(),
        }
    }

    #[test]
    fn crop_rotate_and_blur_keep_the_channel_count_and_bounds() {
        let a = img(100, 60, 3);
        let c = crop_border(&a, 32, 0.1);
        assert_eq!(
            (c.width, c.height),
            (80, 48),
            "capped at a tenth of each edge"
        );
        let c = crop_border(&a, 4, 0.5);
        assert_eq!((c.width, c.height), (92, 52));
        let r = rotate(&a, 75.0);
        assert_eq!((r.width, r.height, r.channels), (100, 60, 3));
        let r0 = rotate(&a, 0.0);
        assert_eq!(r0.data, a.data, "a zero rotation is the identity");
        let b = blur(&a, 4.0);
        assert_eq!(b.data.len(), a.data.len());
        let flat = Image {
            width: 20,
            height: 20,
            channels: 3,
            data: vec![77; 20 * 20 * 3],
        };
        assert_eq!(
            blur(&flat, 2.0).data,
            flat.data,
            "a flat image blurs to itself"
        );
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
