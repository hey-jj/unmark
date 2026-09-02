//! PNG through the `png` crate. Decoding expands palette, low-depth gray, and
//! tRNS to 8-bit RGB or RGBA and strips 16-bit samples to 8. Encoding is
//! pinned to the crate's `High` compression with the adaptive filter.

use super::{CodecError, Image};
use std::io::Cursor;

pub fn decode(bytes: &[u8]) -> Result<Image, CodecError> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|e| CodecError::Malformed(format!("png: {e}")))?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| CodecError::Malformed("png: output size overflow".to_string()))?;
    let mut buf = vec![0u8; size];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| CodecError::Malformed(format!("png: {e}")))?;
    buf.truncate(info.buffer_size());
    let (color, depth) = reader.output_color_type();
    if depth != png::BitDepth::Eight {
        return Err(CodecError::Unsupported(format!(
            "png: {depth:?} output depth after expansion"
        )));
    }
    let width = info.width as usize;
    let height = info.height as usize;
    let (channels, data) = match color {
        png::ColorType::Rgb => (3, buf),
        png::ColorType::Rgba => (4, buf),
        png::ColorType::Grayscale => (3, buf.iter().flat_map(|&g| [g, g, g]).collect()),
        png::ColorType::GrayscaleAlpha => (
            4,
            buf.chunks_exact(2)
                .flat_map(|p| [p[0], p[0], p[0], p[1]])
                .collect(),
        ),
        png::ColorType::Indexed => {
            return Err(CodecError::Malformed(
                "png: palette survived expansion".to_string(),
            ))
        }
    };
    if data.len() != width * height * channels {
        return Err(CodecError::Malformed(
            "png: buffer size mismatch".to_string(),
        ));
    }
    Ok(Image {
        width,
        height,
        channels,
        data,
    })
}

pub fn encode(img: &Image) -> Result<Vec<u8>, CodecError> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, img.width as u32, img.height as u32);
        enc.set_color(match img.channels {
            3 => png::ColorType::Rgb,
            4 => png::ColorType::Rgba,
            n => return Err(CodecError::Setting(format!("png: {n} channels"))),
        });
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::High);
        enc.set_filter(png::Filter::Adaptive);
        let mut w = enc
            .write_header()
            .map_err(|e| CodecError::Setting(format!("png: {e}")))?;
        w.write_image_data(&img.data)
            .map_err(|e| CodecError::Setting(format!("png: {e}")))?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_round_trips_rgb_and_rgba() {
        for channels in [3usize, 4] {
            let img = Image {
                width: 5,
                height: 3,
                channels,
                data: (0..5 * 3 * channels)
                    .map(|i| (i * 37 % 256) as u8)
                    .collect(),
            };
            let bytes = encode(&img).unwrap();
            assert_eq!(decode(&bytes).unwrap(), img);
        }
    }
}
