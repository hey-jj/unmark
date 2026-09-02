//! The pixel degrade transforms PX01 through PX06, applied to a decoded image
//! in the catalog's canonical order: flip (PX06), crop (PX03), resample
//! (PX02), noise (PX05), requantize (PX04), then the container write, which
//! is the re-encode (PX01) when the format is lossy or the plan asks for it.
//!
//! The two-cost split lives here as two stages. `geometry` applies the
//! stages that change which pixels exist and yields the grid-matched
//! reference; `signal` applies the stages that change pixel values on that
//! grid. The signal metric compares the decoded output to the reference.
//!
//! PX01 on a PNG or WebP input is a held seam: the owner's ruling asks for a
//! lossy WebP write and no pure Rust lossy WebP encoder exists. The seam
//! returns `PixelError::Held` with the reason, so a plan that reaches it is
//! reported as held rather than silently written lossless.

use crate::asset::Format;
use crate::codec::{self, CodecError, Image};
use crate::dsp::{self, Rng};
use crate::policy::PolicyPackage;

/// The pinned parameters of a pixel plan, read from the policy package.
#[derive(Clone, Debug, PartialEq)]
pub struct PixelParams {
    /// PX06 operation, `flip-horizontal`, `flip-vertical`, or `rotate-90`.
    pub flip: Option<String>,
    /// PX03 area fraction kept.
    pub crop_area: Option<f64>,
    /// PX02 edge ratio.
    pub resample_ratio: Option<f64>,
    /// PX05 sigma on the 0 to 255 scale.
    pub noise_sigma: Option<f64>,
    /// PX04 bits per channel.
    pub requantize_bits: Option<u32>,
    /// PX01 present in the plan.
    pub reencode: bool,
    pub jpeg_quality: u32,
    pub jpeg_chroma: String,
}

impl PixelParams {
    /// Read the parameters for the PX ids in `ids` from the package. Ids the
    /// package does not know, or with a missing parameter, are an error.
    pub fn from_policy(pkg: &PolicyPackage, ids: &[String]) -> Result<PixelParams, String> {
        let mut p = PixelParams {
            flip: None,
            crop_area: None,
            resample_ratio: None,
            noise_sigma: None,
            requantize_bits: None,
            reencode: false,
            jpeg_quality: 0,
            jpeg_chroma: String::new(),
        };
        // PX01's encoder settings are read even when PX01 is absent, because a
        // lossy container still has to be written with pinned settings.
        let px01 = pkg.transform("PX01").ok_or("policy has no PX01")?;
        p.jpeg_quality = px01.param_i64("jpeg_quality").ok_or("PX01 jpeg_quality")? as u32;
        p.jpeg_chroma = px01
            .param_str("jpeg_chroma")
            .ok_or("PX01 jpeg_chroma")?
            .to_string();
        for id in ids {
            if !id.starts_with("PX") {
                continue;
            }
            let t = pkg
                .transform(id)
                .ok_or_else(|| format!("policy has no {id}"))?;
            match id.as_str() {
                "PX01" => p.reencode = true,
                "PX02" => p.resample_ratio = Some(t.param_f64("ratio").ok_or("PX02 ratio")?),
                "PX03" => p.crop_area = Some(t.param_f64("area").ok_or("PX03 area")?),
                "PX04" => p.requantize_bits = Some(t.param_i64("bits").ok_or("PX04 bits")? as u32),
                "PX05" => p.noise_sigma = Some(t.param_f64("sigma").ok_or("PX05 sigma")?),
                "PX06" => p.flip = Some(t.param_str("op").ok_or("PX06 op")?.to_string()),
                other => return Err(format!("{other} is not a pixel transform")),
            }
        }
        Ok(p)
    }

    /// The geometry cost: the resample ratio and the crop area fraction.
    pub fn geometry_cost(&self) -> (f64, f64) {
        (
            self.resample_ratio.unwrap_or(1.0),
            self.crop_area.unwrap_or(1.0),
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PixelError {
    /// The plan reaches a seam the owner has not ruled on.
    Held(String),
    Codec(CodecError),
    Params(String),
}

impl std::fmt::Display for PixelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PixelError::Held(m) => write!(f, "held: {m}"),
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

/// PX06.
pub fn flip(img: &Image, op: &str) -> Result<Image, PixelError> {
    let c = img.channels;
    let (w, h) = (img.width, img.height);
    let px = |x: usize, y: usize| &img.data[(y * w + x) * c..(y * w + x + 1) * c];
    let mut data = Vec::with_capacity(img.data.len());
    let (nw, nh) = match op {
        "flip-horizontal" => {
            for y in 0..h {
                for x in (0..w).rev() {
                    data.extend_from_slice(px(x, y));
                }
            }
            (w, h)
        }
        "flip-vertical" => {
            for y in (0..h).rev() {
                for x in 0..w {
                    data.extend_from_slice(px(x, y));
                }
            }
            (w, h)
        }
        "rotate-90" => {
            // Clockwise: the new row j reads the old column j bottom to top.
            for j in 0..w {
                for i in 0..h {
                    data.extend_from_slice(px(j, h - 1 - i));
                }
            }
            (h, w)
        }
        other => return Err(PixelError::Params(format!("PX06 op {other}"))),
    };
    Ok(Image {
        width: nw,
        height: nh,
        channels: c,
        data,
    })
}

/// PX03: the centered window keeping `area` of the pixels.
pub fn crop(img: &Image, area: f64) -> Image {
    let edge = area.sqrt();
    let nw = ((img.width as f64 * edge).round() as usize).clamp(1, img.width);
    let nh = ((img.height as f64 * edge).round() as usize).clamp(1, img.height);
    let x0 = (img.width - nw) / 2;
    let y0 = (img.height - nh) / 2;
    let c = img.channels;
    let mut data = Vec::with_capacity(nw * nh * c);
    for y in y0..y0 + nh {
        let row = &img.data[(y * img.width + x0) * c..(y * img.width + x0 + nw) * c];
        data.extend_from_slice(row);
    }
    Image {
        width: nw,
        height: nh,
        channels: c,
        data,
    }
}

/// PX02: both edges by `ratio`, rounded half up, never below one pixel.
pub fn resample(img: &Image, ratio: f64) -> Image {
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

/// PX05: Gaussian noise on the color channels, alpha untouched.
pub fn add_noise(img: &Image, sigma: f64, seed: u64) -> Image {
    let mut rng = Rng::seed(seed);
    let c = img.channels;
    let data = img
        .data
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            if c == 4 && i % 4 == 3 {
                v
            } else {
                (v as f64 + sigma * rng.gaussian())
                    .round()
                    .clamp(0.0, 255.0) as u8
            }
        })
        .collect();
    Image {
        width: img.width,
        height: img.height,
        channels: c,
        data,
    }
}

/// PX04: each color channel to `bits` levels and back to eight, alpha
/// untouched.
pub fn requantize(img: &Image, bits: u32) -> Image {
    let levels = ((1u32 << bits.clamp(1, 8)) - 1) as f64;
    let c = img.channels;
    let data = img
        .data
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            if c == 4 && i % 4 == 3 {
                v
            } else {
                let q = (v as f64 * levels / 255.0).round();
                (q * 255.0 / levels).round().clamp(0.0, 255.0) as u8
            }
        })
        .collect();
    Image {
        width: img.width,
        height: img.height,
        channels: c,
        data,
    }
}

/// The geometry stages in canonical order. The result is the grid-matched
/// reference for the signal metric.
pub fn geometry(img: &Image, p: &PixelParams) -> Result<Image, PixelError> {
    let mut cur = img.clone();
    if let Some(op) = &p.flip {
        cur = flip(&cur, op)?;
    }
    if let Some(a) = p.crop_area {
        cur = crop(&cur, a);
    }
    if let Some(r) = p.resample_ratio {
        cur = resample(&cur, r);
    }
    Ok(cur)
}

/// The value stages in canonical order, on the reference grid.
pub fn signal(img: &Image, p: &PixelParams, seed: u64) -> Image {
    let mut cur = img.clone();
    if let Some(s) = p.noise_sigma {
        cur = add_noise(&cur, s, seed);
    }
    if let Some(b) = p.requantize_bits {
        cur = requantize(&cur, b);
    }
    cur
}

/// Write the container. A JPEG input is always re-encoded, since it has no
/// lossless write. A PNG or WebP input is written lossless unless the plan
/// carries PX01, which is the held seam.
pub fn encode_output(img: &Image, format: Format, p: &PixelParams) -> Result<Vec<u8>, PixelError> {
    match format {
        Format::Jpeg => Ok(codec::jpeg::encode(img, p.jpeg_quality, &p.jpeg_chroma)?),
        Format::Png if p.reencode => Err(PixelError::Held(
            "PX01 on PNG input: the ruling asks for a lossy WebP write and no pure Rust lossy WebP encoder exists".to_string(),
        )),
        Format::Png => Ok(codec::png::encode(img)?),
        Format::WebP if p.reencode => Err(PixelError::Held(
            "PX01 on WebP input: no pure Rust lossy WebP encoder exists".to_string(),
        )),
        Format::WebP => Ok(codec::webp::encode_lossless(img)?),
        other => Err(PixelError::Codec(CodecError::Unsupported(format!(
            "{} is not a pixel container",
            other.as_str()
        )))),
    }
}

/// A full pixel-plan application: the written bytes, the grid-matched
/// reference, and the decoded output.
#[derive(Clone, Debug)]
pub struct PixelOutcome {
    pub bytes: Vec<u8>,
    pub reference: Image,
    pub output: Image,
}

pub fn apply(
    input: &Image,
    format: Format,
    p: &PixelParams,
    seed: u64,
) -> Result<PixelOutcome, PixelError> {
    let reference = geometry(input, p)?;
    let valued = signal(&reference, p, seed);
    let bytes = encode_output(&valued, format, p)?;
    let output = codec::decode_image(&bytes, format)?;
    Ok(PixelOutcome {
        bytes,
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

    #[test]
    fn a_double_flip_is_identity_and_a_rotate_transposes_size() {
        let a = img(7, 5, 3);
        assert_eq!(
            flip(&flip(&a, "flip-horizontal").unwrap(), "flip-horizontal").unwrap(),
            a
        );
        assert_eq!(
            flip(&flip(&a, "flip-vertical").unwrap(), "flip-vertical").unwrap(),
            a
        );
        let r = flip(&a, "rotate-90").unwrap();
        assert_eq!((r.width, r.height), (5, 7));
    }

    #[test]
    fn crop_keeps_the_area_fraction_centered() {
        let a = img(100, 60, 3);
        let c = crop(&a, 0.9);
        assert_eq!((c.width, c.height), (95, 57));
    }

    #[test]
    fn requantize_to_eight_bits_is_identity_and_noise_leaves_alpha_alone() {
        let a = img(9, 9, 4);
        assert_eq!(requantize(&a, 8), a);
        let n = add_noise(&a, 6.0, 42);
        for (p, q) in a.data.chunks_exact(4).zip(n.data.chunks_exact(4)) {
            assert_eq!(p[3], q[3]);
        }
        assert_ne!(n, a);
        assert_eq!(add_noise(&a, 6.0, 42), n, "same seed, same grain");
    }
}
