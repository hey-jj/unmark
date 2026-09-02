//! The audio degrade transforms AU01, AU02, AU04, and AU05 on decoded
//! samples, in the catalog's canonical order: resample round trip (AU01),
//! EQ tilt (AU04), time stretch (AU05), then dithered requantization (AU02),
//! which is the last stage because it fixes the output bit depth. AU03 is
//! reserved and not runnable in this version; a plan that names it is
//! refused as held.
//!
//! Every stage runs sequentially in f64 on the scalar path. Only AU02 uses
//! randomness, seeded per asset.

use crate::codec::Audio;
use crate::dsp::{self, Biquad, Rng};
use crate::policy::PolicyPackage;

#[derive(Clone, Debug, PartialEq)]
pub struct SincParams {
    pub half_taps: usize,
    pub beta: f64,
    pub cutoff: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TiltParams {
    pub pivot_hz: f64,
    pub low_gain_db: f64,
    pub high_gain_db: f64,
    pub slope: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RequantParams {
    pub bits: u16,
    pub dither_lsb: f64,
}

/// The pinned parameters of an audio plan, read from the policy package.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioParams {
    /// AU01's resampler, also used by AU05, so it is read whenever the
    /// package carries AU01.
    pub sinc: SincParams,
    pub round_trip: bool,
    pub tilt: Option<TiltParams>,
    pub stretch: Option<f64>,
    pub requantize: Option<RequantParams>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AudioError {
    Held(String),
    Params(String),
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioError::Held(m) => write!(f, "held: {m}"),
            AudioError::Params(m) => write!(f, "params: {m}"),
        }
    }
}

impl AudioParams {
    pub fn from_policy(pkg: &PolicyPackage, ids: &[String]) -> Result<AudioParams, AudioError> {
        let au01 = pkg
            .transform("AU01")
            .ok_or_else(|| AudioError::Params("policy has no AU01".to_string()))?;
        let need = |v: Option<f64>, what: &str| {
            v.ok_or_else(|| AudioError::Params(format!("missing parameter {what}")))
        };
        let mut p = AudioParams {
            sinc: SincParams {
                half_taps: need(au01.param_f64("half_taps"), "AU01 half_taps")? as usize,
                beta: need(au01.param_f64("beta"), "AU01 beta")?,
                cutoff: need(au01.param_f64("cutoff"), "AU01 cutoff")?,
            },
            round_trip: false,
            tilt: None,
            stretch: None,
            requantize: None,
        };
        for id in ids {
            if !id.starts_with("AU") {
                continue;
            }
            let t = pkg
                .transform(id)
                .ok_or_else(|| AudioError::Params(format!("policy has no {id}")))?;
            match id.as_str() {
                "AU01" => p.round_trip = true,
                "AU02" => {
                    p.requantize = Some(RequantParams {
                        bits: need(t.param_f64("bits"), "AU02 bits")? as u16,
                        dither_lsb: need(t.param_f64("dither_lsb"), "AU02 dither_lsb")?,
                    })
                }
                "AU03" => {
                    return Err(AudioError::Held(
                        "AU03 is reserved and not runnable in this version".to_string(),
                    ))
                }
                "AU04" => {
                    p.tilt = Some(TiltParams {
                        pivot_hz: need(t.param_f64("pivot_hz"), "AU04 pivot_hz")?,
                        low_gain_db: need(t.param_f64("low_gain_db"), "AU04 low_gain_db")?,
                        high_gain_db: need(t.param_f64("high_gain_db"), "AU04 high_gain_db")?,
                        slope: need(t.param_f64("slope"), "AU04 slope")?,
                    })
                }
                "AU05" => p.stretch = Some(need(t.param_f64("factor"), "AU05 factor")?),
                other => {
                    return Err(AudioError::Params(format!(
                        "{other} is not an audio transform"
                    )))
                }
            }
        }
        Ok(p)
    }

    /// The output container bit depth for an input at `input_bits`.
    pub fn output_bits(&self, input_bits: u16) -> u16 {
        self.requantize
            .as_ref()
            .map(|r| r.bits)
            .unwrap_or(input_bits)
    }

    /// The geometry cost: the time-stretch factor.
    pub fn time_stretch(&self) -> f64 {
        self.stretch.unwrap_or(1.0)
    }
}

/// AU01's partner rate: 44.1 kHz and 48 kHz swap, anything else goes through
/// 48 kHz.
pub fn partner_rate(rate: u32) -> u32 {
    if rate == 48000 {
        44100
    } else {
        48000
    }
}

fn resample_channel(x: &[f64], p: u64, q: u64, n_out: usize, s: &SincParams) -> Vec<f64> {
    dsp::resample_sinc(x, p, q, n_out, s.half_taps, s.beta, s.cutoff)
}

/// The stretch factor as a rational read to three decimals, so the step is
/// exact and the kernel phases repeat.
pub fn factor_rational(factor: f64) -> (u64, u64) {
    let p = (factor * 1000.0).round().max(1.0) as u64;
    (p, 1000)
}

/// AU01: to the partner rate and back, output trimmed or zero-padded to the
/// input length so the reference is the input itself.
pub fn round_trip(audio: &Audio, s: &SincParams) -> Audio {
    let rate = audio.rate;
    let partner = partner_rate(rate);
    let n = audio.frames();
    let n_mid = ((n as u128 * partner as u128) / rate as u128) as usize;
    let channels = audio
        .channels
        .iter()
        .map(|ch| {
            let mid = resample_channel(ch, rate as u64, partner as u64, n_mid, s);
            let mut back = resample_channel(&mid, partner as u64, rate as u64, n, s);
            back.resize(n, 0.0);
            back
        })
        .collect();
    Audio {
        rate,
        bits: audio.bits,
        channels,
    }
}

/// AU04: a low shelf and a high shelf at the pivot with opposite gains.
pub fn tilt(audio: &Audio, t: &TiltParams) -> Audio {
    let rate = audio.rate as f64;
    let low = Biquad::shelf(rate, t.pivot_hz, t.low_gain_db, t.slope, false);
    let high = Biquad::shelf(rate, t.pivot_hz, t.high_gain_db, t.slope, true);
    Audio {
        rate: audio.rate,
        bits: audio.bits,
        channels: audio
            .channels
            .iter()
            .map(|ch| high.apply(&low.apply(ch)))
            .collect(),
    }
}

/// AU05: a speed change by `factor` through the resampler. Output length is
/// the input length over the factor.
pub fn stretch(audio: &Audio, factor: f64, s: &SincParams) -> Audio {
    let n = audio.frames();
    let (p, q) = factor_rational(factor);
    let n_out = ((n as u128 * q as u128) / p as u128).max(1) as usize;
    Audio {
        rate: audio.rate,
        bits: audio.bits,
        channels: audio
            .channels
            .iter()
            .map(|ch| resample_channel(ch, p, q, n_out, s))
            .collect(),
    }
}

/// AU02: triangular dither at `dither_lsb` steps of the target depth, then
/// rounding to that depth. The samples stay normalized; the target depth is
/// what the container write uses.
pub fn dither_requantize(audio: &Audio, r: &RequantParams, seed: u64) -> Audio {
    let mut rng = Rng::seed(seed);
    let full = (1i64 << (r.bits - 1)) as f64;
    let lo = -1.0;
    let hi = (full - 1.0) / full;
    Audio {
        rate: audio.rate,
        bits: r.bits,
        channels: audio
            .channels
            .iter()
            .map(|ch| {
                ch.iter()
                    .map(|&x| {
                        let d = rng.triangular() * r.dither_lsb;
                        ((x * full + d).round() / full).clamp(lo, hi)
                    })
                    .collect()
            })
            .collect(),
    }
}

/// The plan in canonical order.
pub fn apply(audio: &Audio, p: &AudioParams, seed: u64) -> Audio {
    let mut cur = audio.clone();
    if p.round_trip {
        cur = round_trip(&cur, &p.sinc);
    }
    if let Some(t) = &p.tilt {
        cur = tilt(&cur, t);
    }
    if let Some(f) = p.stretch {
        cur = stretch(&cur, f, &p.sinc);
    }
    if let Some(r) = &p.requantize {
        cur = dither_requantize(&cur, r, seed);
    }
    cur
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rate: u32, n: usize, hz: f64) -> Audio {
        Audio {
            rate,
            bits: 16,
            channels: vec![(0..n)
                .map(|i| 0.5 * (2.0 * std::f64::consts::PI * hz * i as f64 / rate as f64).sin())
                .collect()],
        }
    }

    fn sinc() -> SincParams {
        SincParams {
            half_taps: 64,
            beta: 10.0,
            cutoff: 0.97,
        }
    }

    #[test]
    fn the_round_trip_keeps_length_and_a_mid_band_tone() {
        let a = tone(44100, 8000, 1000.0);
        let b = round_trip(&a, &sinc());
        assert_eq!(b.frames(), 8000);
        let err: f64 = a.channels[0][2000..6000]
            .iter()
            .zip(&b.channels[0][2000..6000])
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f64::max);
        assert!(err < 1e-3, "max error {err}");
    }

    #[test]
    fn the_stretch_shortens_by_the_factor() {
        let a = tone(48000, 4800, 440.0);
        let b = stretch(&a, 1.03, &sinc());
        assert_eq!(b.frames(), 4800 * 1000 / 1030);
    }

    #[test]
    fn dither_is_seed_determined_and_within_one_step() {
        let a = tone(8000, 500, 100.0);
        let r = RequantParams {
            bits: 16,
            dither_lsb: 1.0,
        };
        let b = dither_requantize(&a, &r, 9);
        let c = dither_requantize(&a, &r, 9);
        assert_eq!(b, c);
        for (x, y) in a.channels[0].iter().zip(&b.channels[0]) {
            assert!((x - y).abs() <= 1.5 / 32768.0);
        }
    }
}
