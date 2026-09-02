//! JPEG. Decoding goes through `jpeg-decoder` with its platform-independent
//! path, so the same bytes decode to the same pixels on every machine.
//! Encoding is a ground-up baseline sequential encoder: JFIF header, the
//! standard Annex K quantization and Huffman tables, quality scaling by the
//! usual 5000/q and 200-2q rule, a separable f64 DCT, 4:4:4 chroma. Nothing
//! in it is adaptive, so the output is a pure function of the pixels and the
//! quality.

use super::{CodecError, Image};

pub fn decode(bytes: &[u8]) -> Result<Image, CodecError> {
    let mut d = jpeg_decoder::Decoder::new(bytes);
    let pixels = d
        .decode()
        .map_err(|e| CodecError::Malformed(format!("jpeg: {e}")))?;
    let info = d
        .info()
        .ok_or_else(|| CodecError::Malformed("jpeg: no frame info".to_string()))?;
    let width = info.width as usize;
    let height = info.height as usize;
    let data = match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => pixels,
        jpeg_decoder::PixelFormat::L8 => pixels.iter().flat_map(|&g| [g, g, g]).collect(),
        other => {
            return Err(CodecError::Unsupported(format!(
                "jpeg: {other:?} pixel format"
            )))
        }
    };
    if data.len() != width * height * 3 {
        return Err(CodecError::Malformed(
            "jpeg: buffer size mismatch".to_string(),
        ));
    }
    Ok(Image {
        width,
        height,
        channels: 3,
        data,
    })
}

// --- encoder ------------------------------------------------------------------

const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

const LUMA_QUANT: [u16; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56,
    14, 17, 22, 29, 51, 87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113,
    92, 49, 64, 78, 87, 103, 121, 120, 101, 72, 92, 95, 98, 112, 100, 103, 99,
];

const CHROMA_QUANT: [u16; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26, 56, 99, 99, 99, 99, 99,
    47, 66, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
];

const DC_LUMA_BITS: [u8; 16] = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];
const DC_LUMA_VALS: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
const DC_CHROMA_BITS: [u8; 16] = [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0];
const DC_CHROMA_VALS: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
const AC_LUMA_BITS: [u8; 16] = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7d];
const AC_LUMA_VALS: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07,
    0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08, 0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0,
    0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28,
    0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
    0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
    0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7,
    0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5,
    0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2,
    0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];
const AC_CHROMA_BITS: [u8; 16] = [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77];
const AC_CHROMA_VALS: [u8; 162] = [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71,
    0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33, 0x52, 0xf0,
    0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34, 0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26,
    0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
    0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
    0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
    0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5,
    0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3,
    0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda,
    0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];

/// A canonical Huffman table: (code, length) indexed by symbol.
struct Huff {
    codes: [(u16, u8); 256],
}

impl Huff {
    fn build(bits: &[u8; 16], vals: &[u8]) -> Huff {
        let mut codes = [(0u16, 0u8); 256];
        let mut code: u16 = 0;
        let mut k = 0;
        for (len_minus_one, &count) in bits.iter().enumerate() {
            for _ in 0..count {
                codes[vals[k] as usize] = (code, len_minus_one as u8 + 1);
                code += 1;
                k += 1;
            }
            code <<= 1;
        }
        Huff { codes }
    }
}

struct BitWriter {
    out: Vec<u8>,
    acc: u32,
    n: u32,
}

impl BitWriter {
    fn put(&mut self, code: u32, len: u32) {
        self.acc = (self.acc << len) | (code & ((1 << len) - 1));
        self.n += len;
        while self.n >= 8 {
            let b = ((self.acc >> (self.n - 8)) & 0xFF) as u8;
            self.out.push(b);
            if b == 0xFF {
                self.out.push(0);
            }
            self.n -= 8;
        }
    }

    fn flush(&mut self) {
        if self.n > 0 {
            let pad = 8 - self.n;
            self.put((1 << pad) - 1, pad);
        }
    }
}

fn scaled_table(base: &[u16; 64], quality: u32) -> [u16; 64] {
    let q = quality.clamp(1, 100);
    let scale = if q < 50 { 5000 / q } else { 200 - 2 * q };
    let mut out = [0u16; 64];
    for i in 0..64 {
        let v = (base[i] as u32 * scale + 50) / 100;
        out[i] = v.clamp(1, 255) as u16;
    }
    out
}

fn category(v: i32) -> (u32, u32) {
    // (bit length, value bits) for a DC difference or an AC coefficient.
    let a = v.unsigned_abs();
    let len = 32 - a.leading_zeros();
    let bits = if v < 0 { (v - 1) as u32 } else { v as u32 };
    (len, bits & ((1u32 << len) - 1))
}

fn dct_block(block: &[f64; 64], cos: &[[f64; 8]; 8]) -> [f64; 64] {
    let mut tmp = [0.0f64; 64];
    // Rows.
    for y in 0..8 {
        for u in 0..8 {
            let mut s = 0.0;
            for x in 0..8 {
                s += block[y * 8 + x] * cos[x][u];
            }
            tmp[y * 8 + u] = s;
        }
    }
    // Columns.
    let mut out = [0.0f64; 64];
    for u in 0..8 {
        for v in 0..8 {
            let mut s = 0.0;
            for y in 0..8 {
                s += tmp[y * 8 + u] * cos[y][v];
            }
            let cu = if u == 0 {
                std::f64::consts::FRAC_1_SQRT_2
            } else {
                1.0
            };
            let cv = if v == 0 {
                std::f64::consts::FRAC_1_SQRT_2
            } else {
                1.0
            };
            out[v * 8 + u] = 0.25 * cu * cv * s;
        }
    }
    out
}

fn segment(out: &mut Vec<u8>, marker: u8, payload: &[u8]) {
    out.push(0xFF);
    out.push(marker);
    let len = (payload.len() + 2) as u16;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(payload);
}

/// Encode RGB (alpha, if present, is dropped) as a baseline JPEG at
/// `quality` with the named chroma layout. Only "4:4:4" is honored.
pub fn encode(img: &Image, quality: u32, chroma: &str) -> Result<Vec<u8>, CodecError> {
    if chroma != "4:4:4" {
        return Err(CodecError::Setting(format!(
            "jpeg: chroma {chroma} is not encoded here, only 4:4:4"
        )));
    }
    if img.width == 0 || img.height == 0 || img.width > 65535 || img.height > 65535 {
        return Err(CodecError::Setting(
            "jpeg: dimensions out of range".to_string(),
        ));
    }
    let lq = scaled_table(&LUMA_QUANT, quality);
    let cq = scaled_table(&CHROMA_QUANT, quality);
    let dc_l = Huff::build(&DC_LUMA_BITS, &DC_LUMA_VALS);
    let ac_l = Huff::build(&AC_LUMA_BITS, &AC_LUMA_VALS);
    let dc_c = Huff::build(&DC_CHROMA_BITS, &DC_CHROMA_VALS);
    let ac_c = Huff::build(&AC_CHROMA_BITS, &AC_CHROMA_VALS);

    let mut out = vec![0xFF, 0xD8];
    // JFIF APP0.
    let app0 = [b'J', b'F', b'I', b'F', 0, 1, 1, 0, 0, 1, 0, 1, 0, 0];
    segment(&mut out, 0xE0, &app0);
    // DQT, both tables in zigzag order.
    let mut dqt = Vec::with_capacity(130);
    for (id, table) in [(0u8, &lq), (1u8, &cq)] {
        dqt.push(id);
        for &z in &ZIGZAG {
            dqt.push(table[z] as u8);
        }
    }
    segment(&mut out, 0xDB, &dqt);
    // SOF0: 8-bit, three components, all 1x1 sampling.
    let mut sof = vec![8];
    sof.extend_from_slice(&(img.height as u16).to_be_bytes());
    sof.extend_from_slice(&(img.width as u16).to_be_bytes());
    sof.push(3);
    sof.extend_from_slice(&[1, 0x11, 0, 2, 0x11, 1, 3, 0x11, 1]);
    segment(&mut out, 0xC0, &sof);
    // DHT, four tables.
    let mut dht = Vec::new();
    for (class_id, bits, vals) in [
        (0x00u8, &DC_LUMA_BITS, &DC_LUMA_VALS[..]),
        (0x10, &AC_LUMA_BITS, &AC_LUMA_VALS[..]),
        (0x01, &DC_CHROMA_BITS, &DC_CHROMA_VALS[..]),
        (0x11, &AC_CHROMA_BITS, &AC_CHROMA_VALS[..]),
    ] {
        dht.push(class_id);
        dht.extend_from_slice(bits);
        dht.extend_from_slice(vals);
    }
    segment(&mut out, 0xC4, &dht);
    // SOS.
    segment(&mut out, 0xDA, &[3, 1, 0x00, 2, 0x11, 3, 0x11, 0, 63, 0]);

    // Entropy-coded data.
    let mut cos = [[0.0f64; 8]; 8];
    for (x, row) in cos.iter_mut().enumerate() {
        for (u, c) in row.iter_mut().enumerate() {
            *c = (((2 * x + 1) as f64) * u as f64 * std::f64::consts::PI / 16.0).cos();
        }
    }
    let mut bw = BitWriter {
        out: Vec::new(),
        acc: 0,
        n: 0,
    };
    let mut prev_dc = [0i32; 3];
    let c = img.channels;
    let mut blocks = [[0.0f64; 64]; 3];
    for by in (0..img.height).step_by(8) {
        for bx in (0..img.width).step_by(8) {
            for y in 0..8 {
                let sy = (by + y).min(img.height - 1);
                for x in 0..8 {
                    let sx = (bx + x).min(img.width - 1);
                    let p = &img.data[(sy * img.width + sx) * c..];
                    let (r, g, b) = (p[0] as f64, p[1] as f64, p[2] as f64);
                    let yy = 0.299 * r + 0.587 * g + 0.114 * b;
                    let cb = -0.168736 * r - 0.331264 * g + 0.5 * b + 128.0;
                    let cr = 0.5 * r - 0.418688 * g - 0.081312 * b + 128.0;
                    blocks[0][y * 8 + x] = yy - 128.0;
                    blocks[1][y * 8 + x] = cb - 128.0;
                    blocks[2][y * 8 + x] = cr - 128.0;
                }
            }
            for comp in 0..3 {
                let (qt, dc_h, ac_h) = if comp == 0 {
                    (&lq, &dc_l, &ac_l)
                } else {
                    (&cq, &dc_c, &ac_c)
                };
                let coef = dct_block(&blocks[comp], &cos);
                let mut q = [0i32; 64];
                for i in 0..64 {
                    q[i] = (coef[ZIGZAG[i]] / qt[ZIGZAG[i]] as f64).round() as i32;
                }
                // DC.
                let diff = q[0] - prev_dc[comp];
                prev_dc[comp] = q[0];
                let (len, bits) = category(diff);
                let (code, clen) = dc_h.codes[len as usize];
                bw.put(code as u32, clen as u32);
                if len > 0 {
                    bw.put(bits, len);
                }
                // AC.
                let mut run = 0u32;
                for &v in &q[1..] {
                    if v == 0 {
                        run += 1;
                        continue;
                    }
                    while run > 15 {
                        let (code, clen) = ac_h.codes[0xF0];
                        bw.put(code as u32, clen as u32);
                        run -= 16;
                    }
                    let (len, bits) = category(v);
                    let sym = (run << 4) | len;
                    let (code, clen) = ac_h.codes[sym as usize];
                    bw.put(code as u32, clen as u32);
                    bw.put(bits, len);
                    run = 0;
                }
                if run > 0 {
                    let (code, clen) = ac_h.codes[0x00];
                    bw.put(code as u32, clen as u32);
                }
            }
        }
    }
    bw.flush();
    out.extend_from_slice(&bw.out);
    out.extend_from_slice(&[0xFF, 0xD9]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_huffman_tables_are_complete() {
        assert_eq!(AC_LUMA_BITS.iter().map(|&b| b as usize).sum::<usize>(), 162);
        assert_eq!(
            AC_CHROMA_BITS.iter().map(|&b| b as usize).sum::<usize>(),
            162
        );
        assert_eq!(DC_LUMA_BITS.iter().map(|&b| b as usize).sum::<usize>(), 12);
        assert_eq!(
            DC_CHROMA_BITS.iter().map(|&b| b as usize).sum::<usize>(),
            12
        );
        // Kraft: a prefix code of these lengths fits.
        for bits in [&AC_LUMA_BITS, &AC_CHROMA_BITS] {
            let k: f64 = bits
                .iter()
                .enumerate()
                .map(|(i, &n)| n as f64 / (1u64 << (i + 1)) as f64)
                .sum();
            assert!(k <= 1.0);
        }
    }

    #[test]
    fn a_gradient_survives_an_encode_decode_at_high_quality() {
        let (w, h) = (37usize, 21usize);
        let mut data = Vec::new();
        for y in 0..h {
            for x in 0..w {
                data.push((x * 6) as u8);
                data.push((y * 11) as u8);
                data.push(((x + y) * 3) as u8);
            }
        }
        let img = Image {
            width: w,
            height: h,
            channels: 3,
            data,
        };
        let bytes = encode(&img, 95, "4:4:4").unwrap();
        let back = decode(&bytes).unwrap();
        assert_eq!((back.width, back.height, back.channels), (w, h, 3));
        let mse: f64 = img
            .data
            .iter()
            .zip(&back.data)
            .map(|(a, b)| (*a as f64 - *b as f64).powi(2))
            .sum::<f64>()
            / img.data.len() as f64;
        assert!(mse < 12.0, "mse {mse}");
    }
}
