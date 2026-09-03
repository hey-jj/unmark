//! The audio path of the default run: AU06, a second-order highpass at the
//! pinned cutoff, the one audio transform with a cited effect (AudioSeal
//! detection accuracy 0.61 under a 1500 Hz highpass). The samples are written
//! back as emitted, integer or float32, never upconverted.

use crate::codec::Audio;
use crate::dsp::Biquad;
use crate::policy::PolicyPackage;

#[derive(Clone, Debug, PartialEq)]
pub struct AudioParams {
    pub highpass_hz: Option<f64>,
}

impl AudioParams {
    pub fn from_policy(pkg: &PolicyPackage, ids: &[String]) -> Result<AudioParams, String> {
        let highpass_hz = if ids.iter().any(|i| i == "AU06") {
            let t = pkg.transform("AU06").ok_or("policy has no AU06")?;
            Some(t.param_f64("cutoff_hz").ok_or("AU06 cutoff_hz")?)
        } else {
            None
        };
        Ok(AudioParams { highpass_hz })
    }
}

/// AU06: a second-order highpass, Butterworth Q, applied per channel from
/// rest on the scalar path.
pub fn highpass(audio: &Audio, cutoff_hz: f64) -> Audio {
    let f = Biquad::highpass(
        audio.rate as f64,
        cutoff_hz,
        std::f64::consts::FRAC_1_SQRT_2,
    );
    Audio {
        rate: audio.rate,
        bits: audio.bits,
        float: audio.float,
        channels: audio.channels.iter().map(|ch| f.apply(ch)).collect(),
    }
}

pub fn apply(audio: &Audio, p: &AudioParams) -> Audio {
    match p.highpass_hz {
        Some(hz) => highpass(audio, hz),
        None => audio.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rate: u32, n: usize, hz: f64) -> Vec<f64> {
        (0..n)
            .map(|i| crate::dsp::sin(2.0 * std::f64::consts::PI * hz * i as f64 / rate as f64))
            .collect()
    }

    #[test]
    fn the_highpass_passes_a_high_tone_and_cuts_a_low_one() {
        let rate = 48000;
        let a = Audio {
            rate,
            bits: 16,
            float: false,
            channels: vec![tone(rate, 48000, 100.0), tone(rate, 48000, 8000.0)],
        };
        let b = highpass(&a, 1500.0);
        let rms = |v: &[f64]| (v[24000..].iter().map(|x| x * x).sum::<f64>() / 24000.0).sqrt();
        assert!(
            rms(&b.channels[0]) < 0.02,
            "100 Hz tone cut: {}",
            rms(&b.channels[0])
        );
        assert!(
            rms(&b.channels[1]) > 0.65,
            "8 kHz tone kept: {}",
            rms(&b.channels[1])
        );
    }
}
