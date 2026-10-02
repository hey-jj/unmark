//! MPEG audio layer III decode and encode behind the audio feature. The
//! decoder is nanomp3, a safe port that documents bit-exact agreement with
//! its reference on x86-64 and ARM64; the encoder is rusty_mp3, a pure Rust
//! encoder with no dependencies. Both are MIT or Apache-2.0. The decode
//! yields f32 samples; the encode takes them back at a constant bitrate.

use super::{Audio, CodecError};

/// The decoded audio with the delay and padding of a nameless information
/// frame applied, and the stream's bitrate in kbps: the constant rate of a
/// CBR stream, or the average of a VBR one. The decoder applies the
/// fields of an extension that names its encoder; a minimal frame carries
/// the same fields without the name, so they are applied here, and a
/// decode of a cleaned file comes back at the length its input decoded to.
pub fn decode_with_bitrate(bytes: &[u8]) -> Result<(Audio, u32), CodecError> {
    let (audio, kbps) = decode_raw_with_bitrate(bytes)?;
    let audio = match crate::detect::mp3::layout(bytes).info {
        Some(i) if !i.identity => match i.delay_padding {
            Some((delay, padding)) => apply_delay_padding(&audio, delay, padding),
            None => audio,
        },
        _ => audio,
    };
    Ok((audio, kbps))
}

/// The decoder's output as it comes, delay and padding included unless the
/// stream's own extension names an encoder.
pub fn decode_raw_with_bitrate(bytes: &[u8]) -> Result<(Audio, u32), CodecError> {
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

/// The encoder's delay in samples, measured: an impulse encoded through
/// rusty_mp3 and decoded through nanomp3 lands 1057 samples late, and
/// nanomp3 adds the 529-sample decoder delay to the value a LAME-style
/// extension carries, so the extension value is 528. The same at every
/// bitrate pin and sample rate; a test re-measures it.
pub const ENCODER_DELAY: u16 = 528;

/// The decoder's own delay nanomp3 adds to the extension's value.
pub const DECODER_DELAY: usize = 529;

/// Samples per frame per channel for the stream version.
pub fn samples_per_frame(version: crate::detect::mp3::Version) -> usize {
    match version {
        crate::detect::mp3::Version::Mpeg1 => 1152,
        _ => 576,
    }
}

/// What the encode wrote and the fields its information frame carries.
#[derive(Clone, Debug)]
pub struct Encoded {
    pub bytes: Vec<u8>,
    pub bitrate_kbps: u32,
    /// Audio frames after the information frame.
    pub frames: u32,
    pub delay: u16,
    pub padding: u16,
}

/// Encode at a constant bitrate, snapped to the layer III table for the
/// stream's MPEG version. The input is padded with silence so the encoder
/// flushes every sample through its delay, and the encoder's own
/// information frame is rewritten as the minimal one: frame count, byte
/// count, and the measured delay and padding, zero elsewhere.
pub fn encode(audio: &Audio, bitrate_kbps: u32) -> Result<Encoded, CodecError> {
    let n = audio.channels.len();
    if n == 0 || n > 2 {
        return Err(CodecError::Setting(format!("mp3: {n} channels")));
    }
    let frames_in = audio.frames();
    // Two frames of silence cover the delay at any input length.
    let tail = 2 * 1152;
    let mut interleaved = Vec::with_capacity((frames_in + tail) * n);
    for i in 0..frames_in {
        for ch in &audio.channels {
            interleaved.push(ch[i].clamp(-1.0, 1.0) as f32);
        }
    }
    interleaved.resize((frames_in + tail) * n, 0.0);
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
    // The encoder's information frame comes first; it is replaced by the
    // minimal one carrying the fields a decoder needs for gapless output.
    let l = crate::detect::mp3::layout(&out);
    let Some(info) = l.info.as_ref().filter(|i| i.offset == l.frames_start) else {
        return Err(CodecError::Setting(
            "mp3: the encoder wrote no information frame".to_string(),
        ));
    };
    let frames = l.frames.saturating_sub(1) as u32;
    let h = crate::detect::mp3::parse_header(&out[info.offset..]).ok_or_else(|| {
        CodecError::Setting("mp3: the information frame header did not parse".to_string())
    })?;
    let spf = samples_per_frame(h.version);
    let total = frames as usize * spf;
    let padding = total
        .checked_sub(ENCODER_DELAY as usize + frames_in)
        .ok_or_else(|| {
            CodecError::Setting("mp3: the encode is shorter than its input".to_string())
        })?;
    let padding = u16::try_from(padding)
        .map_err(|_| CodecError::Setting("mp3: the padding exceeds the field".to_string()))?;
    let mut header_bytes = [0u8; 4];
    header_bytes.copy_from_slice(&out[info.offset..info.offset + 4]);
    let minimal = crate::container::mp3::build_info_frame(
        &header_bytes,
        h.crc,
        h.side_info,
        info.len,
        (frames, (l.frames_end - l.frames_start) as u32),
        (ENCODER_DELAY, padding),
    );
    out[info.offset..info.offset + info.len].copy_from_slice(&minimal);
    Ok(Encoded {
        bytes: out,
        bitrate_kbps: used,
        frames,
        delay: ENCODER_DELAY,
        padding,
    })
}

/// Trim a decoded stream by the delay and padding an information frame
/// carries, the way a gapless decoder does: the delay plus the decoder's
/// own delay off the head, the padding less the decoder's delay off the
/// tail.
pub fn apply_delay_padding(audio: &Audio, delay: u16, padding: u16) -> Audio {
    let head = delay as usize + DECODER_DELAY;
    let tail = (padding as usize).saturating_sub(DECODER_DELAY);
    let channels = audio
        .channels
        .iter()
        .map(|ch| {
            let end = ch.len().saturating_sub(tail);
            ch[head.min(end)..end].to_vec()
        })
        .collect();
    Audio {
        rate: audio.rate,
        bits: audio.bits,
        float: audio.float,
        channels,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn impulse(rate: u32, len: usize) -> Audio {
        let mut samples = vec![0.0f64; len];
        samples[2000] = 0.9;
        Audio {
            rate,
            bits: 16,
            float: false,
            channels: vec![samples],
        }
    }

    /// Re-measure the encoder delay at every bitrate pin and three sample
    /// rates: the impulse lands ENCODER_DELAY plus the decoder delay late,
    /// and the padding field closes the stream to the input length.
    #[test]
    fn the_measured_delay_holds_at_every_pin_and_the_padding_closes_the_length() {
        let cases: Vec<(u32, u32, usize)> = [32u32, 48, 64, 96, 128, 160, 192, 256, 320]
            .iter()
            .flat_map(|&k| [(44100u32, k, 10000usize), (44100, k, 44100)])
            .chain([(22050, 64, 20000), (48000, 128, 20001), (32000, 96, 7)])
            .collect();
        for (rate, kbps, len) in cases {
            let audio = impulse(rate, len.max(2001));
            let enc = encode(&audio, kbps).unwrap();
            let (back, _) = decode_raw_with_bitrate(&enc.bytes).unwrap();
            let out = &back.channels[0];
            let (pos, _) = out
                .iter()
                .enumerate()
                .fold((0usize, 0.0f64), |best, (i, v)| {
                    if v.abs() > best.1 {
                        (i, v.abs())
                    } else {
                        best
                    }
                });
            assert_eq!(
                pos - 2000,
                ENCODER_DELAY as usize + DECODER_DELAY,
                "rate {rate} kbps {kbps} len {len}"
            );
            let trimmed = apply_delay_padding(&back, enc.delay, enc.padding);
            assert_eq!(
                trimmed.channels[0].len(),
                audio.frames(),
                "rate {rate} kbps {kbps} len {len}"
            );
            let (applied, _) = decode_with_bitrate(&enc.bytes).unwrap();
            assert_eq!(applied.channels[0].len(), audio.frames());
            assert_eq!(applied.channels[0], trimmed.channels[0]);
            // The frame carries what was written.
            let info = crate::detect::mp3::layout(&enc.bytes).info.unwrap();
            assert_eq!(info.delay_padding, Some((enc.delay, enc.padding)));
            assert_eq!(info.frames, Some(enc.frames));
            assert!(!info.identity);
        }
    }

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
        let enc = encode(&audio, 128).unwrap();
        assert_eq!(enc.bitrate_kbps, 128);
        let (back, kbps) = decode_with_bitrate(&enc.bytes).unwrap();
        assert_eq!(kbps, 128);
        assert_eq!(back.rate, rate);
        assert_eq!(back.channels.len(), 1);
        assert_eq!(back.frames(), audio.frames());
    }
}
