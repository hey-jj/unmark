//! The dwtDct pixel mark: the keyless watermark the Stable Diffusion
//! pipelines embed, which the CompVis script applies as the UTF-8 text
//! `StableDiffusionV1` and the Diffusers SDXL pipeline as a fixed 48-bit
//! payload. This module is a ground-up detector written from the public
//! algorithm description. It carries no code from the embedding package,
//! which serves only as a subprocess oracle in the efficacy test.
//!
//! The decoder converts BGR to YUV and reads the U channel, crops to
//! multiples of four, takes one level of Haar wavelet and keeps the low-low
//! subband, walks that subband in 4x4 blocks in row-major order, and in each
//! block reads the largest-magnitude value other than the first as the
//! carrier: its magnitude modulo 36 above 18 is a one, at most 18 a zero.
//! Payload bits repeat across blocks cyclically and the repetitions are
//! majority-voted. The reference behavior applies no DCT. The raw low-low
//! block is quantized.
//!
//! The package returns bits and makes no presence decision. Presence here is
//! a declared rule: the Hamming agreement between the voted bits and a known
//! payload at or above `AGREEMENT_THRESHOLD`, whose false-positive fraction
//! on unmarked builder-rendered images is measured and reported beside it.

use crate::codec::Image;

/// The 48-bit Diffusers SDXL payload, highest-order bit first.
pub const SDXL_PAYLOAD: u64 = 0xB3EC907BB19E;
pub const SDXL_BITS: usize = 48;
/// The CompVis payload, 17 bytes of UTF-8, 136 bits.
pub const COMPVIS_TEXT: &str = "StableDiffusionV1";
pub const COMPVIS_BITS: usize = 136;

/// The quantization step and the zero-or-one boundary of the carrier.
pub const SCALE: f64 = 36.0;
pub const HALF: f64 = 18.0;
pub const BLOCK: usize = 4;

/// The declared presence rule: the fraction of payload bits the voted bits
/// must agree with. Measured against builder-rendered unmarked images in
/// `tests/efficacy.rs`, which pins the false-positive fraction the rule
/// produces there.
pub const AGREEMENT_THRESHOLD: f64 = 0.80;

/// The bits of a known payload.
pub fn payload_bits(name: &str) -> Option<Vec<u8>> {
    match name {
        "sdxl-48" => Some(
            (0..SDXL_BITS)
                .map(|i| ((SDXL_PAYLOAD >> (SDXL_BITS - 1 - i)) & 1) as u8)
                .collect(),
        ),
        "compvis-136" => Some(
            COMPVIS_TEXT
                .as_bytes()
                .iter()
                .flat_map(|b| (0..8).map(move |i| (b >> (7 - i)) & 1))
                .collect(),
        ),
        _ => None,
    }
}

pub const PAYLOADS: [&str; 2] = ["sdxl-48", "compvis-136"];

/// The U plane of the image as the package sees it: a BGR to YUV conversion
/// with the 8-bit offset, rounded to the nearest integer and clamped.
pub fn u_plane(img: &Image) -> (Vec<f64>, usize, usize) {
    let c = img.channels;
    let mut u = Vec::with_capacity(img.width * img.height);
    for p in img.data.chunks_exact(c) {
        u.push(u_of(p[0], p[1], p[2]) as f64);
    }
    (u, img.width, img.height)
}

/// The 8-bit U value of one pixel, in the fixed-point arithmetic the
/// reference decoder's colour conversion uses: 14-bit coefficients, Y
/// rounded to an integer first, then U from the blue difference against
/// that rounded Y. A float conversion lands one level off on some pixels,
/// and one level decides the carrier in a nearly flat block, so the integer
/// path is what makes the in-crate read agree with the oracle.
pub fn u_of(r: u8, g: u8, b: u8) -> u8 {
    const SHIFT: i64 = 14;
    const HALF_STEP: i64 = 1 << (SHIFT - 1);
    const R2Y: i64 = 4899;
    const G2Y: i64 = 9617;
    const B2Y: i64 = 1868;
    const B2U: i64 = 8061;
    const DELTA: i64 = 128 << SHIFT;
    let (r, g, b) = (r as i64, g as i64, b as i64);
    let y = (r * R2Y + g * G2Y + b * B2Y + HALF_STEP) >> SHIFT;
    let u = ((b - y) * B2U + DELTA + HALF_STEP) >> SHIFT;
    u.clamp(0, 255) as u8
}

/// One level of the Haar wavelet on a plane cropped to multiples of four,
/// returning the low-low subband and its dimensions.
pub fn haar_low_low(plane: &[f64], width: usize, height: usize) -> (Vec<f64>, usize, usize) {
    let w = width / 4 * 4;
    let h = height / 4 * 4;
    let (lw, lh) = (w / 2, h / 2);
    let mut ll = Vec::with_capacity(lw * lh);
    for y in 0..lh {
        for x in 0..lw {
            let a = plane[(2 * y) * width + 2 * x];
            let b = plane[(2 * y) * width + 2 * x + 1];
            let c = plane[(2 * y + 1) * width + 2 * x];
            let d = plane[(2 * y + 1) * width + 2 * x + 1];
            // The reference decoder's wavelet library filters with two
            // taps of 1/sqrt(2) in double precision, rows first, then
            // columns, each tap product summed later element first. The
            // exact (a+b+c+d)/2 differs from that in the last bit, and a
            // carrier that lands on a multiple of the step reads on the
            // wrong side of the half step, so the arithmetic is mirrored.
            const S: f64 = std::f64::consts::FRAC_1_SQRT_2;
            let left = c * S + a * S;
            let right = d * S + b * S;
            ll.push(right * S + left * S);
        }
    }
    (ll, lw, lh)
}

/// The carrier reading of one block: one when the largest-magnitude value
/// other than the first has magnitude modulo the scale above the half step.
fn block_bit(block: &[f64; 16]) -> u8 {
    let mut pos = 1;
    let mut best = block[1].abs();
    for (i, v) in block.iter().enumerate().skip(2) {
        if v.abs() > best {
            best = v.abs();
            pos = i;
        }
    }
    let val = block[pos].abs();
    if val % SCALE > HALF {
        1
    } else {
        0
    }
}

/// The voted bits for a payload of `n` bits, plus the block count that
/// voted. Fewer blocks than bits means some bits had no vote and read zero.
pub fn decode(img: &Image, n: usize) -> (Vec<u8>, usize) {
    let (u, w, h) = u_plane(img);
    let (ll, lw, lh) = haar_low_low(&u, w, h);
    let mut ones = vec![0usize; n];
    let mut votes = vec![0usize; n];
    let mut num = 0usize;
    for by in 0..lh / BLOCK {
        for bx in 0..lw / BLOCK {
            let mut block = [0.0f64; 16];
            for y in 0..BLOCK {
                for x in 0..BLOCK {
                    block[y * BLOCK + x] = ll[(by * BLOCK + y) * lw + bx * BLOCK + x];
                }
            }
            let bit = block_bit(&block);
            let k = num % n;
            ones[k] += bit as usize;
            votes[k] += 1;
            num += 1;
        }
    }
    let bits = (0..n)
        .map(|k| {
            if votes[k] == 0 {
                0
            } else {
                // The package scales the mean by 255 and compares with 127,
                // which is a strict majority of ones.
                u8::from(ones[k] as f64 * 255.0 / votes[k] as f64 > 127.0)
            }
        })
        .collect();
    (bits, num)
}

/// The presence decision against one known payload.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Decision {
    pub payload: String,
    pub bits: usize,
    /// Blocks that voted. Every payload bit has at least one vote when this
    /// reaches the bit count.
    pub blocks: usize,
    /// The fraction of payload bits the voted bits agree with.
    pub agreement: f64,
    pub threshold: f64,
    pub present: bool,
}

/// Decide presence for every known payload. An image too small to give every
/// payload bit a vote decides absent for that payload.
pub fn detect(img: &Image) -> Vec<Decision> {
    PAYLOADS
        .iter()
        .map(|name| {
            let expected = payload_bits(name).unwrap();
            let (bits, blocks) = decode(img, expected.len());
            let agree = bits.iter().zip(&expected).filter(|(a, b)| a == b).count();
            let agreement = (agree as f64 / expected.len() as f64 * 1000.0).round() / 1000.0;
            Decision {
                payload: name.to_string(),
                bits: expected.len(),
                blocks,
                agreement,
                threshold: AGREEMENT_THRESHOLD,
                present: blocks >= expected.len() && agreement >= AGREEMENT_THRESHOLD,
            }
        })
        .collect()
}

/// Agreement between two bit strings of the same length.
pub fn agreement(a: &[u8], b: &[u8]) -> f64 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    a.iter().zip(b).filter(|(x, y)| x == y).count() as f64 / a.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_payloads_are_the_published_bit_strings() {
        let s = payload_bits("sdxl-48").unwrap();
        let text: String = s.iter().map(|b| if *b == 1 { '1' } else { '0' }).collect();
        assert_eq!(text, "101100111110110010010000011110111011000110011110");
        let c = payload_bits("compvis-136").unwrap();
        assert_eq!(c.len(), 136);
        assert_eq!(&c[..8], &[0, 1, 0, 1, 0, 0, 1, 1], "S is 0x53");
    }

    #[test]
    fn the_carrier_rule_reads_the_quantized_positions() {
        // A block whose carrier sits at the quarter-bin reads zero, at the
        // three-quarter-bin reads one, sign ignored.
        let mut b = [0.0f64; 16];
        b[0] = 500.0;
        b[5] = 36.0 * 3.0 + 9.0;
        assert_eq!(block_bit(&b), 0);
        b[5] = -(36.0 * 3.0 + 27.0);
        assert_eq!(block_bit(&b), 1);
        b[5] = 36.0 * 3.0 + 18.0;
        assert_eq!(block_bit(&b), 0, "at most 18 reads zero");
    }

    #[test]
    fn haar_low_low_halves_each_edge_and_sums_over_two() {
        let plane: Vec<f64> = (0..64).map(|i| i as f64).collect();
        let (ll, w, h) = haar_low_low(&plane, 8, 8);
        assert_eq!((w, h), (4, 4));
        // Mirrors the reference wavelet's tap order, so the sum over two
        // holds to floating precision.
        assert!((ll[0] - (0.0 + 1.0 + 8.0 + 9.0) / 2.0).abs() < 1e-9);
        let (_, w, h) = haar_low_low(&vec![0.0; 10 * 6], 10, 6);
        assert_eq!((w, h), (4, 2), "crops to multiples of four first");
    }

    #[test]
    fn a_tiny_image_decides_absent() {
        let img = Image {
            width: 8,
            height: 8,
            channels: 3,
            data: vec![120; 8 * 8 * 3],
        };
        for d in detect(&img) {
            assert!(!d.present);
            assert!(d.blocks < d.bits);
        }
    }
}
