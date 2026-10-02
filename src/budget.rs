//! Fidelity budgets. A metadata plan is bounded by byte-identity of the pixel
//! or sample stream. A pixel or audio plan is bounded by two costs: a
//! signal-fidelity cost measured on the output grid, and a geometry cost
//! bounded by parameter floors. Both pass or the plan refuses.
//!
//! Milestone 1 exercises the byte-identity gate only, because every transform
//! shipped is a metadata strip. The metric machinery and the two-cost split are
//! built now so the pixel and audio transforms drop in without reworking the
//! gate. The metric ceilings are PROVISIONAL and carry a TODO in the policy
//! package: a corpus calibration pass sets the final numbers before release.

/// A profile's fidelity budget, read from the policy package.
#[derive(Clone, Debug)]
pub enum Budget {
    /// Byte-identity of the pixel or sample stream. Any deviation is a bug.
    Exact,
    Image {
        psnr_floor_db: f64,
        ssim_floor: f64,
        resample_ratio_min: f64,
        crop_area_min: f64,
    },
    Audio {
        lsd_ceiling_db: f64,
        resample_ratio_min: f64,
        time_stretch_min: f64,
        time_stretch_max: f64,
    },
}

/// The measured cost of a plan, split into a signal cost and a geometry cost.
#[derive(Clone, Copy, Debug, Default)]
pub struct Cost {
    /// Signal fidelity on the output grid. For an image plan this is PSNR and
    /// SSIM, for audio the log-spectral distance. None for a metadata plan.
    pub psnr_db: Option<f64>,
    pub ssim: Option<f64>,
    pub lsd_db: Option<f64>,
    /// Geometry. A resample ratio of the output over the input support, a crop
    /// area fraction kept, a time-stretch factor. 1.0 means untouched.
    pub resample_ratio: f64,
    pub crop_area: f64,
    pub time_stretch: f64,
}

impl Cost {
    pub fn none() -> Cost {
        Cost {
            psnr_db: None,
            ssim: None,
            lsd_db: None,
            resample_ratio: 1.0,
            crop_area: 1.0,
            time_stretch: 1.0,
        }
    }
}

/// Why a plan exceeded its budget.
#[derive(Clone, Debug, PartialEq)]
pub struct BudgetExceeded {
    pub reason: String,
}

/// Check a metric-bounded plan's cost against its budget. Both the signal cost
/// and the geometry cost must pass. The byte-identity gate is checked
/// separately with `check_byte_identity`, because it compares streams rather
/// than reading a metric.
pub fn check(budget: &Budget, cost: &Cost) -> Result<(), BudgetExceeded> {
    match budget {
        Budget::Exact => Ok(()),
        Budget::Image {
            psnr_floor_db,
            ssim_floor,
            resample_ratio_min,
            crop_area_min,
        } => {
            if let Some(p) = cost.psnr_db {
                if p < *psnr_floor_db {
                    return Err(exceeded(format!(
                        "PSNR {p:.2} dB is below the {psnr_floor_db:.2} dB floor"
                    )));
                }
            }
            if let Some(s) = cost.ssim {
                if s < *ssim_floor {
                    return Err(exceeded(format!(
                        "SSIM {s:.4} is below the {ssim_floor:.4} floor"
                    )));
                }
            }
            if cost.resample_ratio < *resample_ratio_min {
                return Err(exceeded(format!(
                    "resample ratio {:.3} is below the {:.3} floor",
                    cost.resample_ratio, resample_ratio_min
                )));
            }
            if cost.crop_area < *crop_area_min {
                return Err(exceeded(format!(
                    "crop keeps {:.3} of the area, below the {:.3} floor",
                    cost.crop_area, crop_area_min
                )));
            }
            Ok(())
        }
        Budget::Audio {
            lsd_ceiling_db,
            resample_ratio_min,
            time_stretch_min,
            time_stretch_max,
        } => {
            if let Some(l) = cost.lsd_db {
                if l > *lsd_ceiling_db {
                    return Err(exceeded(format!(
                        "log-spectral distance {l:.3} dB exceeds the {lsd_ceiling_db:.3} dB ceiling"
                    )));
                }
            }
            if cost.resample_ratio < *resample_ratio_min {
                return Err(exceeded(format!(
                    "resample ratio {:.3} is below the {:.3} floor",
                    cost.resample_ratio, resample_ratio_min
                )));
            }
            if cost.time_stretch < *time_stretch_min || cost.time_stretch > *time_stretch_max {
                return Err(exceeded(format!(
                    "time-stretch {:.3} is outside the {:.3} to {:.3} range",
                    cost.time_stretch, time_stretch_min, time_stretch_max
                )));
            }
            Ok(())
        }
    }
}

/// The metadata-tier gate: the pixel or sample stream must be byte-identical.
/// An empty stream on either side is no proof at all. A walker that found no
/// payload chunk yields an empty stream, and two empty streams compare equal,
/// which is how a rewrite that lost the payload could otherwise pass.
pub fn check_byte_identity(before: &[u8], after: &[u8]) -> Result<(), BudgetExceeded> {
    if before.is_empty() || after.is_empty() {
        return Err(exceeded(
            "the encoded signal stream is empty on at least one side, so identity cannot be proven"
                .to_string(),
        ));
    }
    if before == after {
        Ok(())
    } else {
        Err(exceeded(format!(
            "the encoded signal stream changed under a metadata profile: {} input bytes, {} output bytes",
            before.len(),
            after.len()
        )))
    }
}

fn exceeded(reason: String) -> BudgetExceeded {
    BudgetExceeded { reason }
}

// --- pure metric functions, exercised by milestone 2 -----------------------

/// Peak signal-to-noise ratio in dB between two equal-length 8-bit sample
/// buffers on the same grid. Returns None when the lengths differ. Infinite
/// PSNR (identical buffers) reports as a large finite ceiling so the gate reads
/// it as passing.
pub fn psnr(a: &[u8], b: &[u8]) -> Option<f64> {
    if a.len() != b.len() || a.is_empty() {
        return None;
    }
    let mut sq = 0.0f64;
    for (x, y) in a.iter().zip(b) {
        let d = *x as f64 - *y as f64;
        sq += d * d;
    }
    let mse = sq / a.len() as f64;
    if mse == 0.0 {
        return Some(120.0);
    }
    Some(10.0 * (255.0f64 * 255.0 / mse).log10())
}

/// Structural similarity as the mean over non-overlapping 8x8 windows of
/// BT.601 luma, computed from interleaved 8-bit RGB or RGBA buffers on the
/// same grid. Partial windows at the right and bottom edges are dropped, so
/// an image narrower or shorter than the window is unscorable and returns
/// None, as does a size or channel mismatch. Population variance and the
/// standard constants with K1 0.01 and K2 0.03 on the 255 range.
pub fn ssim(a: &[u8], b: &[u8], width: usize, height: usize, channels: usize) -> Option<f64> {
    if a.len() != b.len() || a.len() != width * height * channels || !(3..=4).contains(&channels) {
        return None;
    }
    let window = SSIM_WINDOW;
    if width < window || height < window {
        return None;
    }
    let luma = |buf: &[u8], x: usize, y: usize| -> f64 {
        let p = &buf[(y * width + x) * channels..];
        0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64
    };
    let c1 = (0.01 * 255.0f64).powi(2);
    let c2 = (0.03 * 255.0f64).powi(2);
    let n = (window * window) as f64;
    let mut total = 0.0;
    let mut count = 0usize;
    for wy in 0..height / window {
        for wx in 0..width / window {
            let (mut ma, mut mb) = (0.0f64, 0.0f64);
            for y in 0..window {
                for x in 0..window {
                    ma += luma(a, wx * window + x, wy * window + y);
                    mb += luma(b, wx * window + x, wy * window + y);
                }
            }
            ma /= n;
            mb /= n;
            let (mut va, mut vb, mut cov) = (0.0f64, 0.0f64, 0.0f64);
            for y in 0..window {
                for x in 0..window {
                    let dx = luma(a, wx * window + x, wy * window + y) - ma;
                    let dy = luma(b, wx * window + x, wy * window + y) - mb;
                    va += dx * dx;
                    vb += dy * dy;
                    cov += dx * dy;
                }
            }
            va /= n;
            vb /= n;
            cov /= n;
            let num = (2.0 * ma * mb + c1) * (2.0 * cov + c2);
            let den = (ma * ma + mb * mb + c1) * (va + vb + c2);
            total += num / den;
            count += 1;
        }
    }
    Some(total / count as f64)
}

/// The SSIM window edge, pinned in the policy package as `ssim_window`.
pub const SSIM_WINDOW: usize = 8;

/// The pinned log-spectral-distance parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LsdParams {
    pub frame: usize,
    pub hop: usize,
    pub power_floor: f64,
    pub min_frames: usize,
}

impl LsdParams {
    pub const PINNED: LsdParams = LsdParams {
        frame: 2048,
        hop: 1024,
        power_floor: 1e-10,
        min_frames: 4,
    };
}

/// Per-frame log power spectra, dB, of a mono signal: periodic Hann frames
/// of `frame` samples at `hop`, power |X_k / N|^2 floored at `power_floor`.
/// Fewer than `min_frames` frames is unscorable and returns None.
pub fn log_spectra(x: &[f64], p: &LsdParams) -> Option<Vec<Vec<f64>>> {
    if x.len() < p.frame {
        return None;
    }
    let frames = (x.len() - p.frame) / p.hop + 1;
    if frames < p.min_frames {
        return None;
    }
    let window = crate::dsp::hann_periodic(p.frame);
    let mut out = Vec::with_capacity(frames);
    for f in 0..frames {
        let start = f * p.hop;
        let power = crate::dsp::power_spectrum(&x[start..start + p.frame], &window);
        out.push(
            power
                .iter()
                .map(|&v| 10.0 * v.max(p.power_floor).log10())
                .collect(),
        );
    }
    Some(out)
}

/// Frame-wise log-spectral distance in dB between two mono signals on the
/// same grid: RMS across bins per frame, mean across frames, over the frames
/// both signals have. None when either is unscorable.
pub fn lsd(reference: &[f64], output: &[f64], p: &LsdParams) -> Option<f64> {
    let a = log_spectra(reference, p)?;
    let b = log_spectra(output, p)?;
    let frames = a.len().min(b.len());
    if frames < p.min_frames {
        return None;
    }
    let mut total = 0.0;
    for f in 0..frames {
        let bins = a[f].len();
        let sq: f64 = a[f].iter().zip(&b[f]).map(|(x, y)| (x - y) * (x - y)).sum();
        total += (sq / bins as f64).sqrt();
    }
    Some(total / frames as f64)
}

/// The AU05 metric: the RMS distance in dB between the time-averaged log
/// power spectra of two mono signals that share no grid. Power averages in
/// the linear domain across frames, then the floor and the log apply.
pub fn averaged_spectrum_distance(reference: &[f64], output: &[f64], p: &LsdParams) -> Option<f64> {
    let avg = |x: &[f64]| -> Option<Vec<f64>> {
        if x.len() < p.frame {
            return None;
        }
        let frames = (x.len() - p.frame) / p.hop + 1;
        if frames < p.min_frames {
            return None;
        }
        let window = crate::dsp::hann_periodic(p.frame);
        let mut acc = vec![0.0f64; p.frame / 2 + 1];
        for f in 0..frames {
            let start = f * p.hop;
            let power = crate::dsp::power_spectrum(&x[start..start + p.frame], &window);
            for (a, v) in acc.iter_mut().zip(power) {
                *a += v;
            }
        }
        Some(
            acc.iter()
                .map(|v| 10.0 * (v / frames as f64).max(p.power_floor).log10())
                .collect(),
        )
    };
    let a = avg(reference)?;
    let b = avg(output)?;
    let sq: f64 = a.iter().zip(&b).map(|(x, y)| (x - y) * (x - y)).sum();
    Some((sq / a.len() as f64).sqrt())
}

/// The sample lag at which `output` best matches `reference`, searched over
/// `0..=max_lag` by normalised cross-correlation on the first `window`
/// samples. An MP3 encoder and decoder together add a fixed delay at the
/// head of the decoded stream (1105 samples for the layer III pair this
/// crate carries), and a spectral distance over misaligned frames reads
/// broadband content as if it were two different signals. The lag is an
/// integer argmax, so it is the same on every platform.
pub fn align_lag(reference: &[f64], output: &[f64], max_lag: usize, window: usize) -> usize {
    let n = window.min(reference.len());
    if n == 0 {
        return 0;
    }
    let ref_norm: f64 = reference[..n].iter().map(|v| v * v).sum::<f64>().sqrt();
    let mut best = (0usize, f64::NEG_INFINITY);
    for lag in 0..=max_lag {
        if lag + n > output.len() {
            break;
        }
        let out = &output[lag..lag + n];
        let dot: f64 = reference[..n].iter().zip(out).map(|(a, b)| a * b).sum();
        let out_norm: f64 = out.iter().map(|v| v * v).sum::<f64>().sqrt();
        let score = if ref_norm > 0.0 && out_norm > 0.0 {
            dot / (ref_norm * out_norm)
        } else {
            0.0
        };
        if score > best.1 {
            best = (lag, score);
        }
    }
    best.0
}

#[cfg(test)]
mod align_tests {
    use super::*;

    #[test]
    fn the_lag_search_finds_a_known_shift() {
        let reference: Vec<f64> = (0..20000)
            .map(|i| crate::dsp::sin(i as f64 * 0.37) + crate::dsp::sin(i as f64 * 0.011))
            .collect();
        let mut output = vec![0.0; 1105];
        output.extend_from_slice(&reference);
        output.extend_from_slice(&[0.0; 500]);
        assert_eq!(align_lag(&reference, &output, 2304, 16384), 1105);
        assert_eq!(align_lag(&reference, &reference, 2304, 16384), 0);
        assert_eq!(align_lag(&[], &output, 10, 16), 0);
    }
}
