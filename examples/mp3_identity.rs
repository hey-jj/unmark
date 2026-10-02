//! The MP3 identity pins: for every fixture under a directory, the sha256 of
//! the decoded samples, of the highpassed samples, and of the re-encoded
//! bytes, as one JSON object. CI runs the pinned comparison on an x86_64
//! Linux leg and an aarch64 macOS leg, so a stage whose digest differs
//! between them is not byte-identical across platforms.
//!
//!     cargo run --release --features audio --example mp3_identity -- fixtures/mp3

use sha2::{Digest, Sha256};
use std::path::PathBuf;
use unmark::codec::mp3;
use unmark::transform::audio::{self, AudioParams};

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(64);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn digest_samples(channels: &[Vec<f64>]) -> String {
    let mut h = Sha256::new();
    for ch in channels {
        for v in ch {
            h.update((*v as f32).to_le_bytes());
        }
    }
    hex(&h.finalize())
}

pub fn stages(bytes: &[u8]) -> Option<(String, String, String, u32)> {
    let (audio, kbps) = mp3::decode_with_bitrate(bytes).ok()?;
    let decoded = digest_samples(&audio.channels);
    let processed = audio::apply(
        &audio,
        &AudioParams {
            highpass_hz: Some(1500.0),
        },
    );
    let highpassed = digest_samples(&processed.channels);
    let enc = mp3::encode(&processed, kbps).ok()?;
    let mut h = Sha256::new();
    h.update(&enc.bytes);
    Some((decoded, highpassed, hex(&h.finalize()), enc.bitrate_kbps))
}

fn main() {
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            eprintln!("usage: mp3_identity DIR");
            std::process::exit(2);
        });
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("directory")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "mp3"))
        .collect();
    files.sort();
    let mut rows = Vec::new();
    for path in files {
        let bytes = std::fs::read(&path).unwrap();
        let Some((decoded, highpassed, encoded, kbps)) = stages(&bytes) else {
            continue;
        };
        rows.push(format!(
            "  \"{}\": {{\"decoded\": \"{decoded}\", \"highpassed\": \"{highpassed}\", \"encoded\": \"{encoded}\", \"bitrate_kbps\": {kbps}}}",
            path.file_name().unwrap().to_string_lossy()
        ));
    }
    println!("{{\n{}\n}}", rows.join(",\n"));
}
