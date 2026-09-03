//! RIFF WAV, read and written here without a dependency. Integer PCM at 8,
//! 16, 24, and 32 bits and IEEE float32 (format tag 3) are decoded, including
//! the WAVE_FORMAT_EXTENSIBLE wrapper around either. Encoding writes the
//! canonical 44-byte header at 16 or 24 bits, or float32 when the samples
//! came in as float32 and no requantization ran.

use super::{Audio, CodecError};

fn u16le(b: &[u8]) -> u16 {
    u16::from_le_bytes([b[0], b[1]])
}

fn u32le(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

pub fn decode(bytes: &[u8]) -> Result<Audio, CodecError> {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(CodecError::Malformed("wav: not a RIFF WAVE".to_string()));
    }
    let mut pos = 12;
    let mut fmt: Option<(u16, u16, u32, u16)> = None;
    let mut data: Option<&[u8]> = None;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32le(&bytes[pos + 4..pos + 8]) as usize;
        let start = pos + 8;
        let end = start.saturating_add(size).min(bytes.len());
        let body = &bytes[start..end];
        if id == b"fmt " {
            if body.len() < 16 {
                return Err(CodecError::Malformed("wav: short fmt chunk".to_string()));
            }
            let mut tag = u16le(&body[0..2]);
            let channels = u16le(&body[2..4]);
            let rate = u32le(&body[4..8]);
            let bits = u16le(&body[14..16]);
            if tag == 0xFFFE {
                if body.len() < 26 {
                    return Err(CodecError::Malformed(
                        "wav: short extensible fmt chunk".to_string(),
                    ));
                }
                tag = u16le(&body[24..26]);
            }
            fmt = Some((tag, channels, rate, bits));
        } else if id == b"data" {
            data = Some(body);
        }
        pos = start + size + (size & 1);
    }
    let (tag, channels, rate, bits) =
        fmt.ok_or_else(|| CodecError::Malformed("wav: no fmt chunk".to_string()))?;
    let data = data.ok_or_else(|| CodecError::Malformed("wav: no data chunk".to_string()))?;
    if channels == 0 || rate == 0 {
        return Err(CodecError::Malformed(
            "wav: zero channels or rate".to_string(),
        ));
    }
    if tag == 3 {
        if bits != 32 {
            return Err(CodecError::Unsupported(format!("wav: {bits}-bit float")));
        }
        let frame_bytes = 4 * channels as usize;
        let frames = data.len() / frame_bytes;
        let mut chans = vec![Vec::with_capacity(frames); channels as usize];
        for f in 0..frames {
            for (c, chan) in chans.iter_mut().enumerate() {
                let at = f * frame_bytes + c * 4;
                let v = f32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]);
                chan.push(v as f64);
            }
        }
        return Ok(Audio {
            rate,
            bits: 32,
            float: true,
            channels: chans,
        });
    }
    if tag != 1 {
        return Err(CodecError::Unsupported(format!(
            "wav: format tag {tag} is not integer PCM or float32"
        )));
    }
    if !matches!(bits, 8 | 16 | 24 | 32) {
        return Err(CodecError::Unsupported(format!("wav: {bits}-bit PCM")));
    }
    let bytes_per = bits as usize / 8;
    let frame_bytes = bytes_per * channels as usize;
    let frames = data.len() / frame_bytes;
    let mut samples = Vec::with_capacity(frames * channels as usize);
    for f in 0..frames {
        for c in 0..channels as usize {
            let at = f * frame_bytes + c * bytes_per;
            let s = &data[at..at + bytes_per];
            let v: i32 = match bits {
                8 => s[0] as i32 - 128,
                16 => i16::from_le_bytes([s[0], s[1]]) as i32,
                24 => (i32::from_le_bytes([0, s[0], s[1], s[2]])) >> 8,
                _ => i32::from_le_bytes([s[0], s[1], s[2], s[3]]),
            };
            samples.push(v);
        }
    }
    Ok(Audio::from_interleaved(
        &samples,
        channels as usize,
        bits,
        rate,
    ))
}

pub fn encode(audio: &Audio, bits: u16) -> Result<Vec<u8>, CodecError> {
    let float = bits == 32 && audio.float;
    if !(matches!(bits, 16 | 24) || float) {
        return Err(CodecError::Setting(format!("wav: {bits}-bit output")));
    }
    let channels = audio.channels.len() as u16;
    if channels == 0 {
        return Err(CodecError::Setting("wav: no channels".to_string()));
    }
    let bytes_per = bits as usize / 8;
    let data_len = audio.frames() * channels as usize * bytes_per;
    let mut out = Vec::with_capacity(44 + data_len + 1);
    out.extend_from_slice(b"RIFF");
    let riff_len = 36 + data_len + (data_len & 1);
    out.extend_from_slice(&(riff_len as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&(if float { 3u16 } else { 1u16 }).to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&audio.rate.to_le_bytes());
    let block_align = channels as u32 * bytes_per as u32;
    out.extend_from_slice(&(audio.rate * block_align).to_le_bytes());
    out.extend_from_slice(&(block_align as u16).to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data_len as u32).to_le_bytes());
    if float {
        let n = audio.frames();
        for i in 0..n {
            for c in &audio.channels {
                out.extend_from_slice(&(c[i] as f32).to_le_bytes());
            }
        }
    } else {
        for s in audio.quantize(bits) {
            let b = s.to_le_bytes();
            match bits {
                16 => out.extend_from_slice(&b[..2]),
                _ => out.extend_from_slice(&b[..3]),
            }
        }
    }
    if data_len & 1 == 1 {
        out.push(0);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float32_wav_round_trips_as_emitted() {
        let audio = Audio {
            rate: 44100,
            bits: 32,
            float: true,
            channels: vec![
                (0..300).map(|i| (i as f64 * 0.01).sin() * 0.7).collect(),
                (0..300).map(|i| (i as f64 * 0.02).cos() * 0.3).collect(),
            ],
        };
        let bytes = encode(&audio, 32).unwrap();
        assert_eq!(
            u16::from_le_bytes([bytes[20], bytes[21]]),
            3,
            "format tag 3"
        );
        let back = decode(&bytes).unwrap();
        assert!(back.float && back.bits == 32);
        assert_eq!(back.sample_format(), "float32");
        for (a, b) in audio.channels.iter().zip(&back.channels) {
            for (x, y) in a.iter().zip(b) {
                assert_eq!(*x as f32, *y as f32);
            }
        }
        assert_eq!(back.interleaved_le_bytes(), bytes[44..].to_vec());
    }

    #[test]
    fn wav_round_trips_at_16_and_24_bits() {
        for bits in [16u16, 24] {
            let full = (1i64 << (bits - 1)) as f64;
            let audio = Audio {
                rate: 8000,
                bits,
                float: false,
                channels: vec![
                    (0..50)
                        .map(|i| (i as f64 * 1000.0 - 25000.0) / full)
                        .collect(),
                    (0..50).map(|i| (i as f64 * -500.0 + 3.0) / full).collect(),
                ],
            };
            let bytes = encode(&audio, bits).unwrap();
            let back = decode(&bytes).unwrap();
            assert_eq!(back.rate, 8000);
            assert_eq!(back.bits, bits);
            assert_eq!(back.channels.len(), 2);
            for (a, b) in audio.channels.iter().zip(&back.channels) {
                for (x, y) in a.iter().zip(b) {
                    assert!((x - y).abs() < 1.0 / full, "{x} vs {y}");
                }
            }
        }
    }
}
