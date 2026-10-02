//! MPEG audio layer III decode and encode behind the audio feature. The
//! decoder is nanomp3, a safe port that documents bit-exact agreement with
//! its reference on x86-64 and ARM64; the encoder is rusty_mp3, a pure Rust
//! encoder with no dependencies. Both are MIT or Apache-2.0. The decode
//! yields f32 samples; the encode takes them back at a constant bitrate.

use super::{Audio, CodecError};

/// The decoded audio and the stream's bitrate in kbps: the constant rate
/// of a CBR stream, or the average of a VBR one.
pub fn decode_with_bitrate(bytes: &[u8]) -> Result<(Audio, u32), CodecError> {
    let d = nanomp3::decode_all_with::<f32>(bytes, nanomp3::Options::default());
    let Some(ch) = d.channels else {
        return Err(CodecError::Malformed(
            "mp3: no audio frames decoded".to_string(),
        ));
    };
    if d.layer != 3 {
        return Err(CodecError::Unsupported(format!(
            "mp3: layer {} frames",
            d.layer
        )));
    }
    if d.format_changed {
        return Err(CodecError::Malformed(
            "mp3: the stream changed format mid-way".to_string(),
        ));
    }
    let n = ch as usize;
    let mut channels: Vec<Vec<f64>> = (0..n)
        .map(|_| Vec::with_capacity(d.samples.len() / n))
        .collect();
    for frame in d.samples.chunks_exact(n) {
        for (c, s) in frame.iter().enumerate() {
            channels[c].push(*s as f64);
        }
    }
    Ok((
        Audio {
            rate: d.sample_rate,
            bits: 16,
            float: false,
            channels,
        },
        d.avg_bitrate_kbps,
    ))
}

pub fn decode(bytes: &[u8]) -> Result<Audio, CodecError> {
    decode_with_bitrate(bytes).map(|(a, _)| a)
}

/// Encode at a constant bitrate, snapped to the layer III table for the
/// stream's MPEG version. Returns the bytes and the bitrate used.
pub fn encode(audio: &Audio, bitrate_kbps: u32) -> Result<(Vec<u8>, u32), CodecError> {
    let n = audio.channels.len();
    if n == 0 || n > 2 {
        return Err(CodecError::Setting(format!("mp3: {n} channels")));
    }
    let frames = audio.frames();
    let mut interleaved = Vec::with_capacity(frames * n);
    for i in 0..frames {
        for ch in &audio.channels {
            interleaved.push(ch[i].clamp(-1.0, 1.0) as f32);
        }
    }
    let header = rusty_mp3::encoder_header(audio.rate, n as u16, bitrate_kbps)
        .map_err(|e| CodecError::Setting(format!("mp3: {e}")))?;
    let used = rusty_mp3::snap_bitrate(header.version, bitrate_kbps);
    let mut enc = rusty_mp3::Mp3Encoder::new(rusty_mp3::Mp3EncoderConfig {
        bitrate_kbps: used,
        vbr_quality: None,
    });
    enc.push_pcm_f32(&interleaved, n as u16, audio.rate)
        .map_err(|e| CodecError::Setting(format!("mp3: {e}")))?;
    enc.finish();
    let mut out = Vec::new();
    while let Ok(packet) = enc.next_packet() {
        out.extend_from_slice(&packet);
    }
    if out.is_empty() {
        return Err(CodecError::Setting(
            "mp3: the encoder wrote nothing".to_string(),
        ));
    }
    Ok((out, used))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tone_round_trips_through_encode_and_decode() {
        let rate = 44100u32;
        let samples: Vec<f64> = (0..rate as usize)
            .map(|i| {
                0.5 * crate::dsp::sin(2.0 * std::f64::consts::PI * 440.0 * i as f64 / rate as f64)
            })
            .collect();
        let audio = Audio {
            rate,
            bits: 16,
            float: false,
            channels: vec![samples],
        };
        let (bytes, used) = encode(&audio, 128).unwrap();
        assert_eq!(used, 128);
        let (back, kbps) = decode_with_bitrate(&bytes).unwrap();
        assert_eq!(kbps, 128);
        assert_eq!(back.rate, rate);
        assert_eq!(back.channels.len(), 1);
        assert!(
            back.frames() >= audio.frames(),
            "{} < {}",
            back.frames(),
            audio.frames()
        );
    }
}
