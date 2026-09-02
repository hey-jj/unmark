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

/// Global structural similarity over two equal-length 8-bit buffers on the same
/// grid. This is the single-window form: means, variances, and covariance over
/// the whole buffer. Returns None when the lengths differ.
pub fn ssim(a: &[u8], b: &[u8]) -> Option<f64> {
    if a.len() != b.len() || a.is_empty() {
        return None;
    }
    let n = a.len() as f64;
    let (mut ma, mut mb) = (0.0f64, 0.0f64);
    for (x, y) in a.iter().zip(b) {
        ma += *x as f64;
        mb += *y as f64;
    }
    ma /= n;
    mb /= n;
    let (mut va, mut vb, mut cov) = (0.0f64, 0.0f64, 0.0f64);
    for (x, y) in a.iter().zip(b) {
        let dx = *x as f64 - ma;
        let dy = *y as f64 - mb;
        va += dx * dx;
        vb += dy * dy;
        cov += dx * dy;
    }
    va /= n;
    vb /= n;
    cov /= n;
    let c1 = (0.01 * 255.0f64).powi(2);
    let c2 = (0.03 * 255.0f64).powi(2);
    let num = (2.0 * ma * mb + c1) * (2.0 * cov + c2);
    let den = (ma * ma + mb * mb + c1) * (va + vb + c2);
    Some(num / den)
}
