//! FLAC through `claxon` for decoding and `flacenc` for encoding. The encoder
//! configuration is pinned: the crate's default subframe and stereo coding,
//! a fixed block size, single-threaded.

use super::{Audio, CodecError};
use std::io::Cursor;

/// The pinned block size, in inter-channel samples.
pub const BLOCK_SIZE: usize = 4096;

pub fn decode(bytes: &[u8]) -> Result<Audio, CodecError> {
    let mut reader = claxon::FlacReader::new(Cursor::new(bytes))
        .map_err(|e| CodecError::Malformed(format!("flac: {e}")))?;
    let info = reader.streaminfo();
    if !matches!(info.bits_per_sample, 8 | 16 | 24) {
        return Err(CodecError::Unsupported(format!(
            "flac: {}-bit samples",
            info.bits_per_sample
        )));
    }
    let channels = info.channels as usize;
    let mut samples = Vec::new();
    for s in reader.samples() {
        samples.push(s.map_err(|e| CodecError::Malformed(format!("flac: {e}")))?);
    }
    if channels == 0 || samples.len() % channels != 0 {
        return Err(CodecError::Malformed(
            "flac: sample count is not a whole number of frames".to_string(),
        ));
    }
    Ok(Audio::from_interleaved(
        &samples,
        channels,
        info.bits_per_sample as u16,
        info.sample_rate,
    ))
}

pub fn encode(audio: &Audio, bits: u16) -> Result<Vec<u8>, CodecError> {
    use flacenc::component::BitRepr;
    use flacenc::error::Verify;
    if !matches!(bits, 16 | 24) || (audio.float && bits == 32) {
        return Err(CodecError::Setting(format!(
            "flac: {bits}-bit output; FLAC carries integer PCM only"
        )));
    }
    let channels = audio.channels.len();
    if channels == 0 || channels > 8 {
        return Err(CodecError::Setting(format!("flac: {channels} channels")));
    }
    let samples = audio.quantize(bits);
    // A final block shorter than sixteen samples is not a valid FLAC block,
    // so the block size steps down until the remainder is zero or at least
    // sixteen.
    let frames = audio.frames();
    let mut block_size = BLOCK_SIZE;
    while block_size > 16 && !frames.is_multiple_of(block_size) && frames % block_size < 16 {
        block_size -= 16;
    }
    let mut config = flacenc::config::Encoder::default();
    config.block_size = block_size;
    config.multithread = false;
    let config = config
        .into_verified()
        .map_err(|e| CodecError::Setting(format!("flac: {e:?}")))?;
    let source = flacenc::source::MemSource::from_samples(
        &samples,
        channels,
        bits as usize,
        audio.rate as usize,
    );
    let stream = flacenc::encode_with_fixed_block_size(&config, source, block_size)
        .map_err(|e| CodecError::Setting(format!("flac: {e:?}")))?;
    let mut sink = flacenc::bitsink::ByteSink::new();
    stream
        .write(&mut sink)
        .map_err(|e| CodecError::Setting(format!("flac: {e:?}")))?;
    Ok(sink.as_slice().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flac_round_trips_losslessly() {
        let full = 32768.0;
        let audio = Audio {
            rate: 8000,
            bits: 16,
            float: false,
            channels: vec![
                (0..9000)
                    .map(|i| ((i as f64 * 0.05).sin() * 12000.0).round() / full)
                    .collect(),
                (0..9000)
                    .map(|i| ((i as f64 * 0.031).cos() * 9000.0).round() / full)
                    .collect(),
            ],
        };
        let bytes = encode(&audio, 16).unwrap();
        assert_eq!(&bytes[..4], b"fLaC");
        let back = decode(&bytes).unwrap();
        assert_eq!(back.rate, 8000);
        assert_eq!(back, audio);
    }
}
