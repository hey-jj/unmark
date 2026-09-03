//! WebP through `image-webp`. Decoding covers lossy VP8 and lossless VP8L
//! stills, with or without alpha. Encoding is lossless VP8L only, because no
//! pure Rust lossy WebP encoder exists; a plan that needs a lossy WebP write
//! reports itself as held rather than silently writing lossless.

use super::{CodecError, Image};
use std::io::Cursor;

pub fn decode(bytes: &[u8]) -> Result<Image, CodecError> {
    let mut d = image_webp::WebPDecoder::new(Cursor::new(bytes))
        .map_err(|e| CodecError::Malformed(format!("webp: {e}")))?;
    if d.is_animated() {
        return Err(CodecError::Unsupported(
            "webp: animated files are not calibrated".to_string(),
        ));
    }
    let (w, h) = d.dimensions();
    let channels = if d.has_alpha() { 4 } else { 3 };
    let size = d
        .output_buffer_size()
        .ok_or_else(|| CodecError::Malformed("webp: output size overflow".to_string()))?;
    let mut buf = vec![0u8; size];
    d.read_image(&mut buf)
        .map_err(|e| CodecError::Malformed(format!("webp: {e}")))?;
    let (width, height) = (w as usize, h as usize);
    if buf.len() != width * height * channels {
        return Err(CodecError::Malformed(
            "webp: buffer size mismatch".to_string(),
        ));
    }
    Ok(Image {
        width,
        height,
        channels,
        data: buf,
    })
}

/// Whether the first frame is lossy VP8.
pub fn is_lossy(bytes: &[u8]) -> Result<bool, CodecError> {
    let mut d = image_webp::WebPDecoder::new(Cursor::new(bytes))
        .map_err(|e| CodecError::Malformed(format!("webp: {e}")))?;
    Ok(d.is_lossy())
}

pub fn encode_lossless(img: &Image) -> Result<Vec<u8>, CodecError> {
    encode_lossless_with(img, None, None, None)
}

/// Lossless VP8L with the metadata chunks the encoder carries: EXIF, XMP,
/// and an ICC profile, each as the chunk payload.
pub fn encode_lossless_with(
    img: &Image,
    exif: Option<Vec<u8>>,
    xmp: Option<Vec<u8>>,
    icc: Option<Vec<u8>>,
) -> Result<Vec<u8>, CodecError> {
    let color = match img.channels {
        3 => image_webp::ColorType::Rgb8,
        4 => image_webp::ColorType::Rgba8,
        n => return Err(CodecError::Setting(format!("webp: {n} channels"))),
    };
    let mut out = Vec::new();
    let mut enc = image_webp::WebPEncoder::new(&mut out);
    let mut params = image_webp::EncoderParams::default();
    params.use_predictor_transform = true;
    enc.set_params(params);
    if let Some(e) = exif {
        enc.set_exif_metadata(e);
    }
    if let Some(x) = xmp {
        enc.set_xmp_metadata(x);
    }
    if let Some(i) = icc {
        enc.set_icc_profile(i);
    }
    enc.encode(&img.data, img.width as u32, img.height as u32, color)
        .map_err(|e| CodecError::Setting(format!("webp: {e}")))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lossless_webp_round_trips() {
        for channels in [3usize, 4] {
            let img = Image {
                width: 9,
                height: 4,
                channels,
                data: (0..9 * 4 * channels)
                    .map(|i| (i * 53 % 256) as u8)
                    .collect(),
            };
            let bytes = encode_lossless(&img).unwrap();
            assert!(!is_lossy(&bytes).unwrap());
            assert_eq!(decode(&bytes).unwrap(), img);
        }
    }
}
