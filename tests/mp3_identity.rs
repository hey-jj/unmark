//! The cross-platform identity gate for the MP3 path: the decoded samples,
//! the highpassed samples, and the re-encoded bytes of every fixture must
//! match the digests pinned in fixtures/mp3/identity.json. CI runs this on
//! an x86_64 Linux leg and an aarch64 macOS leg, so a stage that differs
//! between them fails here on one of them.
#![cfg(feature = "audio")]

use sha2::{Digest, Sha256};
use std::path::PathBuf;
use unmark::codec::mp3;
use unmark::transform::audio::{self, AudioParams};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
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

#[test]
fn every_mp3_stage_matches_its_pinned_digest() {
    let dir: PathBuf = [env!("CARGO_MANIFEST_DIR"), "fixtures", "mp3"]
        .iter()
        .collect();
    let pins: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("identity.json")).unwrap()).unwrap();
    let pins = pins.as_object().unwrap();
    assert!(!pins.is_empty());
    let mut problems = Vec::new();
    for (name, pin) in pins {
        let bytes = std::fs::read(dir.join(name)).unwrap();
        let (audio, kbps) = mp3::decode_with_bitrate(&bytes).unwrap();
        let decoded = digest_samples(&audio.channels);
        let processed = audio::apply(
            &audio,
            &AudioParams {
                highpass_hz: Some(1500.0),
            },
        );
        let highpassed = digest_samples(&processed.channels);
        let enc = mp3::encode(&processed, kbps).unwrap();
        let used = enc.bitrate_kbps;
        let mut h = Sha256::new();
        h.update(&enc.bytes);
        let encoded = hex(&h.finalize());
        for (stage, got) in [
            ("decoded", decoded),
            ("highpassed", highpassed),
            ("encoded", encoded),
        ] {
            let want = pin[stage].as_str().unwrap();
            if got != want {
                problems.push(format!(
                    "{name}: the {stage} stage digest differs from the pin"
                ));
            }
        }
        if pin["bitrate_kbps"].as_u64() != Some(used as u64) {
            problems.push(format!("{name}: bitrate {used} differs from the pin"));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
