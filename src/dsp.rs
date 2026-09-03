//! Scalar signal-processing primitives shared by the degrade transforms and the
//! calibration metrics: a seeded generator, a radix-2 FFT, a Kaiser-windowed
//! sinc resampler, a separable Lanczos3 image resampler, and the two shelving
//! filters behind the EQ tilt. Everything here runs sequentially in f64 on the
//! scalar path, so a run is reproducible across machines with the same crate
//! version.

use std::f64::consts::PI;

// --- libm-free elementary functions ----------------------------------------------
//
// The transforms produce output bytes, so every transcendental they touch is
// computed here in sequential f64 with fixed range reduction and a fixed
// polynomial, never through the platform libm. No fused multiply-add is used:
// the expressions below are plain products and sums, which Rust never
// contracts. Accuracy is a few ulps, and the same bits on every platform.

/// pi/2 split so that `k * PIO2_HI` is exact for |k| below 2^27.
const PIO2_HI: f64 = 1.570_796_310_901_641_8;
const PIO2_MID: f64 = 1.589_325_477_352_819_6e-8;
const PIO2_LO: f64 = 6.368_317_163_510_95e-25;
const TWO_OVER_PI: f64 = std::f64::consts::FRAC_2_PI;

/// Reduce x to (k, r) with x = k * pi/2 + r, |r| at most pi/4.
fn reduce_half_pi(x: f64) -> (i64, f64) {
    let k = (x * TWO_OVER_PI).round();
    let r = ((x - k * PIO2_HI) - k * PIO2_MID) - k * PIO2_LO;
    (k as i64, r)
}

/// sin on |r| at most pi/4 by the Taylor series to r^17, Horner form.
fn sin_poly(r: f64) -> f64 {
    let r2 = r * r;
    let p = 2.811_457_254_345_520_6e-15 + r2 * (-1.561_920_696_858_622_5e-16);
    let p = -7.647_163_731_819_816e-13 + r2 * p;
    let p = 1.605_904_383_682_161_3e-10 + r2 * p;
    let p = -2.505_210_838_544_172e-8 + r2 * p;
    let p = 2.755_731_922_398_589_3e-6 + r2 * p;
    let p = -1.984_126_984_126_984e-4 + r2 * p;
    let p = 8.333_333_333_333_333e-3 + r2 * p;
    let p = -0.166_666_666_666_666_66 + r2 * p;
    r + r * r2 * p
}

/// cos on |r| at most pi/4 by the Taylor series to r^18, Horner form.
fn cos_poly(r: f64) -> f64 {
    let r2 = r * r;
    let p = 4.779_477_332_387_385e-14 + r2 * (-1.561_920_696_858_622_5e-16);
    let p = -1.147_074_559_772_972_5e-11 + r2 * p;
    let p = 2.087_675_698_786_81e-9 + r2 * p;
    let p = -2.755_731_922_398_589e-7 + r2 * p;
    let p = 2.480_158_730_158_73e-5 + r2 * p;
    let p = -1.388_888_888_888_889e-3 + r2 * p;
    let p = 0.041_666_666_666_666_664 + r2 * p;
    let p = -0.5 + r2 * p;
    1.0 + r2 * p
}

/// Sine, libm-free. Arguments must stay below 2^27 times pi/2 in magnitude,
/// which every caller in this crate satisfies by construction.
pub fn sin(x: f64) -> f64 {
    let (k, r) = reduce_half_pi(x);
    match k.rem_euclid(4) {
        0 => sin_poly(r),
        1 => cos_poly(r),
        2 => -sin_poly(r),
        _ => -cos_poly(r),
    }
}

/// Cosine, libm-free, same domain as `sin`.
pub fn cos(x: f64) -> f64 {
    let (k, r) = reduce_half_pi(x);
    match k.rem_euclid(4) {
        0 => cos_poly(r),
        1 => -sin_poly(r),
        2 => -cos_poly(r),
        _ => sin_poly(r),
    }
}

/// Natural logarithm for x above zero, libm-free: x = m * 2^e with m in
/// [sqrt(1/2), sqrt(2)), then ln m = 2 atanh((m - 1) / (m + 1)) by its
/// series to the 23rd power.
pub fn ln(x: f64) -> f64 {
    debug_assert!(x > 0.0 && x.is_finite());
    let bits = x.to_bits();
    let mut e = ((bits >> 52) & 0x7ff) as i64 - 1023;
    let mut m = f64::from_bits((bits & 0x000f_ffff_ffff_ffff) | (1023u64 << 52));
    if m > std::f64::consts::SQRT_2 {
        m *= 0.5;
        e += 1;
    }
    let s = (m - 1.0) / (m + 1.0);
    let s2 = s * s;
    let mut term = s;
    let mut sum = 0.0;
    let mut n = 1.0;
    // Fixed 12 terms: s, s^3/3, ... s^23/23.
    for _ in 0..12 {
        sum += term / n;
        term *= s2;
        n += 2.0;
    }
    e as f64 * std::f64::consts::LN_2 + 2.0 * sum
}

/// Exponential, libm-free: x = k ln 2 + r, e^r by the Taylor series to
/// r^13, scaled by 2^k through the exponent field.
pub fn exp(x: f64) -> f64 {
    debug_assert!(x.abs() < 700.0);
    // ln 2 split so that k * LN2_HI is exact for |k| below 2^27.
    const LN2_HI: f64 = 0.6931471675634384;
    const LN2_LO: f64 = 1.2996506893889889e-08;
    let k = (x / std::f64::consts::LN_2).round();
    let r = (x - k * LN2_HI) - k * LN2_LO;
    let mut term = 1.0;
    let mut sum = 1.0;
    for n in 1..=13 {
        term = term * r / n as f64;
        sum += term;
    }
    let scale = f64::from_bits(((k as i64 + 1023) as u64) << 52);
    sum * scale
}

/// 10^x, libm-free.
pub fn pow10(x: f64) -> f64 {
    exp(x * std::f64::consts::LN_10)
}

// --- seeded randomness -------------------------------------------------------

/// xoshiro256** seeded through splitmix64. The generator is pinned by name
/// because the noise it produces is part of a transform's output.
#[derive(Clone, Debug)]
pub struct Rng {
    s: [u64; 4],
}

impl Rng {
    pub fn seed(seed: u64) -> Rng {
        let mut x = seed;
        let mut next = || {
            x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = x;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        };
        Rng {
            s: [next(), next(), next(), next()],
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    /// Uniform in [0, 1) with 53 bits of resolution.
    pub fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Standard normal by Box-Muller, cosine half only, so one draw consumes
    /// two uniforms and the sequence is fixed.
    pub fn gaussian(&mut self) -> f64 {
        let u1 = 1.0 - self.uniform();
        let u2 = self.uniform();
        (-2.0 * ln(u1)).sqrt() * cos(2.0 * PI * u2)
    }

    /// Triangular in (-1, 1): the sum of two uniforms minus one.
    pub fn triangular(&mut self) -> f64 {
        self.uniform() + self.uniform() - 1.0
    }
}

/// The per-asset seed: the policy base seed XOR the first eight bytes of the
/// input's sha256, big-endian. A fixed seed alone would stamp one noise
/// pattern on every asset, itself a correlational mark.
pub fn asset_seed(base_seed: u64, input_sha256: &[u8; 32]) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&input_sha256[..8]);
    base_seed ^ u64::from_be_bytes(b)
}

// --- FFT ---------------------------------------------------------------------

/// In-place iterative radix-2 complex FFT. `re` and `im` must have the same
/// power-of-two length. Unscaled forward transform.
pub fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    assert_eq!(n, im.len());
    assert!(n.is_power_of_two(), "fft length must be a power of two");
    // Bit reversal.
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * PI / len as f64;
        let (wr, wi) = (cos(ang), sin(ang));
        let half = len / 2;
        let mut start = 0;
        while start < n {
            let (mut cr, mut ci) = (1.0f64, 0.0f64);
            for k in 0..half {
                let a = start + k;
                let b = a + half;
                let tr = re[b] * cr - im[b] * ci;
                let ti = re[b] * ci + im[b] * cr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                let ncr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = ncr;
            }
            start += len;
        }
        len <<= 1;
    }
}

/// The periodic Hann window of length n: 0.5 - 0.5 cos(2 pi k / n).
pub fn hann_periodic(n: usize) -> Vec<f64> {
    (0..n)
        .map(|k| 0.5 - 0.5 * cos(2.0 * PI * k as f64 / n as f64))
        .collect()
}

/// Power spectrum of one frame, `frame.len()` a power of two, scaled so that
/// power is |X_k / N|^2, bins 0 through N/2 inclusive.
pub fn power_spectrum(frame: &[f64], window: &[f64]) -> Vec<f64> {
    let n = frame.len();
    let mut re: Vec<f64> = frame.iter().zip(window).map(|(x, w)| x * w).collect();
    let mut im = vec![0.0; n];
    fft(&mut re, &mut im);
    let scale = 1.0 / (n as f64 * n as f64);
    (0..=n / 2)
        .map(|k| (re[k] * re[k] + im[k] * im[k]) * scale)
        .collect()
}

// --- windowed-sinc resampling --------------------------------------------------

fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let half = x / 2.0;
    let mut k = 1.0;
    loop {
        term *= (half / k) * (half / k);
        sum += term;
        if term < sum * 1e-16 {
            break;
        }
        k += 1.0;
    }
    sum
}

fn sinc(x: f64) -> f64 {
    if x == 0.0 {
        1.0
    } else {
        let px = PI * x;
        sin(px) / px
    }
}

/// Kaiser-windowed sinc resampling over a rational step `p / q`: output
/// sample i sits at input position `i * p / q`. `half_taps` is the kernel
/// half-width in samples at unit ratio; when the step exceeds one the kernel
/// widens by the step so the transition band keeps its shape. `cutoff` scales
/// the lowpass below the narrower Nyquist. Samples outside the input read as
/// zero. The kernel is evaluated once per phase (i * p mod q), so the result
/// is a pure function of the arguments whatever the input length.
pub fn resample_sinc(
    x: &[f64],
    p: u64,
    q: u64,
    n_out: usize,
    half_taps: usize,
    beta: f64,
    cutoff: f64,
) -> Vec<f64> {
    assert!(p > 0 && q > 0, "resample step must be positive");
    let g = gcd(p, q);
    let (p, q) = (p / g, q / g);
    let step = p as f64 / q as f64;
    let fc = cutoff * if step > 1.0 { 1.0 / step } else { 1.0 };
    let half_width = (half_taps as f64 / fc).ceil();
    let i0_beta = bessel_i0(beta);
    let kernel_for = |frac: f64| -> Vec<(i64, f64)> {
        let j0 = (frac - half_width).ceil() as i64;
        let j1 = (frac + half_width).floor() as i64;
        let mut taps = Vec::with_capacity((j1 - j0 + 1) as usize);
        for j in j0..=j1 {
            let u = j as f64 - frac;
            let a = u / half_width;
            let w = if a.abs() >= 1.0 {
                0.0
            } else {
                bessel_i0(beta * (1.0 - a * a).sqrt()) / i0_beta
            };
            taps.push((j, fc * sinc(fc * u) * w));
        }
        taps
    };
    // Precompute every phase when the count is modest; otherwise compute
    // per sample by the same formula.
    let phases: Option<Vec<Vec<(i64, f64)>>> = if q <= 16384 {
        Some((0..q).map(|r| kernel_for(r as f64 / q as f64)).collect())
    } else {
        None
    };
    let n_in = x.len() as i64;
    let mut out = Vec::with_capacity(n_out);
    for i in 0..n_out as u64 {
        let num = i * p;
        let base = (num / q) as i64;
        let r = num % q;
        let computed;
        let taps: &[(i64, f64)] = match &phases {
            Some(ph) => &ph[r as usize],
            None => {
                computed = kernel_for(r as f64 / q as f64);
                &computed
            }
        };
        let mut acc = 0.0;
        for (j, h) in taps {
            let k = base + j;
            if k >= 0 && k < n_in {
                acc += x[k as usize] * h;
            }
        }
        out.push(acc);
    }
    out
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

// --- Lanczos3 image resampling --------------------------------------------------

fn lanczos3(x: f64) -> f64 {
    if x.abs() >= 3.0 {
        0.0
    } else {
        sinc(x) * sinc(x / 3.0)
    }
}

/// Weights for one output index along an axis of `n_in` samples mapped to
/// `n_out`. The kernel widens by the inverse ratio when shrinking. Indices are
/// clamped to the edge, so the border replicates.
fn lanczos_weights(n_in: usize, n_out: usize, i: usize) -> Vec<(usize, f64)> {
    let ratio = n_out as f64 / n_in as f64;
    let scale = if ratio < 1.0 { 1.0 / ratio } else { 1.0 };
    let center = (i as f64 + 0.5) / ratio - 0.5;
    let support = 3.0 * scale;
    let k0 = (center - support).floor() as i64 + 1;
    let k1 = (center + support).floor() as i64;
    let mut w = Vec::new();
    let mut sum = 0.0;
    let mut k = k0;
    while k <= k1 {
        let weight = lanczos3((k as f64 - center) / scale);
        if weight != 0.0 {
            let idx = k.clamp(0, n_in as i64 - 1) as usize;
            w.push((idx, weight));
            sum += weight;
        }
        k += 1;
    }
    if sum != 0.0 {
        for (_, weight) in &mut w {
            *weight /= sum;
        }
    }
    w
}

/// Separable Lanczos3 resample of an interleaved 8-bit image, horizontal pass
/// then vertical, f64 accumulation, rounding and clamping once at the end.
pub fn resample_lanczos3(
    data: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    new_width: usize,
    new_height: usize,
) -> Vec<u8> {
    // Horizontal pass into f64.
    let mut tmp = vec![0.0f64; new_width * height * channels];
    let hw: Vec<Vec<(usize, f64)>> = (0..new_width)
        .map(|i| lanczos_weights(width, new_width, i))
        .collect();
    for y in 0..height {
        let row = &data[y * width * channels..(y + 1) * width * channels];
        for (i, weights) in hw.iter().enumerate() {
            for c in 0..channels {
                let mut acc = 0.0;
                for (k, w) in weights {
                    acc += row[k * channels + c] as f64 * w;
                }
                tmp[(y * new_width + i) * channels + c] = acc;
            }
        }
    }
    // Vertical pass.
    let mut out = vec![0u8; new_width * new_height * channels];
    let vw: Vec<Vec<(usize, f64)>> = (0..new_height)
        .map(|j| lanczos_weights(height, new_height, j))
        .collect();
    for (j, weights) in vw.iter().enumerate() {
        for i in 0..new_width {
            for c in 0..channels {
                let mut acc = 0.0;
                for (k, w) in weights {
                    acc += tmp[(k * new_width + i) * channels + c] * w;
                }
                out[(j * new_width + i) * channels + c] = acc.round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

// --- shelving filters ----------------------------------------------------------

/// A biquad in direct form I, coefficients normalized so a0 is one.
#[derive(Clone, Debug)]
pub struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

impl Biquad {
    /// A shelf from the Audio EQ Cookbook forms. `high` selects the high
    /// shelf, otherwise the low shelf. `slope` is the shelf slope parameter S.
    pub fn shelf(rate: f64, freq_hz: f64, gain_db: f64, slope: f64, high: bool) -> Biquad {
        let a = pow10(gain_db / 40.0);
        let w0 = 2.0 * PI * freq_hz / rate;
        let (sw, cw) = (sin(w0), cos(w0));
        let alpha = sw / 2.0 * ((a + 1.0 / a) * (1.0 / slope - 1.0) + 2.0).sqrt();
        let sq = 2.0 * a.sqrt() * alpha;
        let (b0, b1, b2, a0, a1, a2) = if high {
            (
                a * ((a + 1.0) + (a - 1.0) * cw + sq),
                -2.0 * a * ((a - 1.0) + (a + 1.0) * cw),
                a * ((a + 1.0) + (a - 1.0) * cw - sq),
                (a + 1.0) - (a - 1.0) * cw + sq,
                2.0 * ((a - 1.0) - (a + 1.0) * cw),
                (a + 1.0) - (a - 1.0) * cw - sq,
            )
        } else {
            (
                a * ((a + 1.0) - (a - 1.0) * cw + sq),
                2.0 * a * ((a - 1.0) - (a + 1.0) * cw),
                a * ((a + 1.0) - (a - 1.0) * cw - sq),
                (a + 1.0) + (a - 1.0) * cw + sq,
                -2.0 * ((a - 1.0) + (a + 1.0) * cw),
                (a + 1.0) + (a - 1.0) * cw - sq,
            )
        };
        Biquad {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    /// Filter a channel from rest, sequentially.
    pub fn apply(&self, x: &[f64]) -> Vec<f64> {
        let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);
        let mut out = Vec::with_capacity(x.len());
        for &x0 in x {
            let y0 = self.b0 * x0 + self.b1 * x1 + self.b2 * x2 - self.a1 * y1 - self.a2 * y2;
            x2 = x1;
            x1 = x0;
            y2 = y1;
            y1 = y0;
            out.push(y0);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_elementary_functions_track_the_platform_within_a_few_ulps() {
        let ulps = |a: f64, b: f64| ((a - b).abs() / b.abs().max(1e-300)) / f64::EPSILON;
        for i in -20000..20000 {
            let x = i as f64 * 0.0137;
            assert!((sin(x) - x.sin()).abs() < 4.0 * f64::EPSILON, "sin {x}");
            assert!((cos(x) - x.cos()).abs() < 4.0 * f64::EPSILON, "cos {x}");
        }
        for i in 1..20000 {
            let x = i as f64 * 0.37;
            assert!(ulps(ln(x), x.ln()) < 4.0, "ln {x}");
            let y = i as f64 * 0.005 - 40.0;
            assert!(ulps(exp(y), y.exp()) < 8.0, "exp {y}");
        }
        assert!(ulps(pow10(1.5 / 40.0), 10f64.powf(1.5 / 40.0)) < 8.0);
        assert_eq!(sin(0.0), 0.0);
        assert_eq!(cos(0.0), 1.0);
    }

    #[test]
    fn the_generator_is_seed_determined() {
        let mut a = Rng::seed(7);
        let mut b = Rng::seed(7);
        for _ in 0..10 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        let mut c = Rng::seed(8);
        assert_ne!(a.next_u64(), c.next_u64());
    }

    #[test]
    fn the_fft_recovers_a_single_tone() {
        let n = 64;
        let mut re: Vec<f64> = (0..n)
            .map(|k| (2.0 * PI * 4.0 * k as f64 / n as f64).cos())
            .collect();
        let mut im = vec![0.0; n];
        fft(&mut re, &mut im);
        let mag: Vec<f64> = re
            .iter()
            .zip(&im)
            .map(|(r, i)| (r * r + i * i).sqrt())
            .collect();
        assert!((mag[4] - 32.0).abs() < 1e-9);
        assert!(mag[5].abs() < 1e-9);
    }

    #[test]
    fn unit_step_resampling_reproduces_the_input_away_from_the_edges() {
        let x: Vec<f64> = (0..400).map(|k| (0.05 * k as f64).sin()).collect();
        let y = resample_sinc(&x, 1, 1, 400, 64, 10.0, 0.97);
        for k in 100..300 {
            assert!(
                (x[k] - y[k]).abs() < 1e-3,
                "sample {k}: {} vs {}",
                x[k],
                y[k]
            );
        }
    }

    #[test]
    fn lanczos_identity_size_is_identity() {
        let data: Vec<u8> = (0..64u32).map(|v| (v * 4) as u8).collect();
        let out = resample_lanczos3(&data, 8, 8, 1, 8, 8);
        assert_eq!(out, data);
    }

    #[test]
    fn a_zero_gain_shelf_is_transparent() {
        let f = Biquad::shelf(48000.0, 1000.0, 0.0, 1.0, true);
        let x: Vec<f64> = (0..100).map(|k| (0.3 * k as f64).sin()).collect();
        let y = f.apply(&x);
        for (a, b) in x.iter().zip(&y) {
            assert!((a - b).abs() < 1e-9);
        }
    }
}
