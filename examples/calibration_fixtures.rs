//! Generate the committed calibration fixture subset: one small synthetic
//! asset per (format, band) plus a manifest, under `fixtures/calibration/`.
//! Every pattern is drawn here from a seeded generator, so the subset carries
//! no third-party media and regenerates byte for byte.
//!
//!     cargo run --example calibration_fixtures --all-features -- fixtures/calibration
//!
//! A lossy WebP made by an external `cwebp` is welcome in the directory but is
//! not produced here, because no pure Rust lossy WebP encoder exists.

use std::collections::BTreeMap;
use std::path::PathBuf;
use unmark::calibrate::{self, Manifest, ManifestAsset};
use unmark::codec::{self, Audio, Image};
use unmark::dsp::{cos, sin, Rng};

fn image(width: usize, height: usize, channels: usize, seed: u64) -> Image {
    // Texture, gradient, and edges together: a diffusion-style busy field.
    let mut rng = Rng::seed(seed);
    let mut data = Vec::with_capacity(width * height * channels);
    for y in 0..height {
        for x in 0..width {
            let fx = x as f64 / width as f64;
            let fy = y as f64 / height as f64;
            let tex = (sin(x as f64 * 0.37) * cos(y as f64 * 0.29) * 40.0)
                + ((x / 13 + y / 7) % 2) as f64 * 30.0;
            let noise = rng.gaussian() * 4.0;
            let r = 90.0 + 120.0 * fx + tex + noise;
            let g = 60.0 + 150.0 * fy - tex * 0.5 + noise;
            let b = 140.0 - 100.0 * fx * fy + tex * 0.8 - noise;
            data.push(r.round().clamp(0.0, 255.0) as u8);
            data.push(g.round().clamp(0.0, 255.0) as u8);
            data.push(b.round().clamp(0.0, 255.0) as u8);
            if channels == 4 {
                let a = 128.0 + 127.0 * (sin(x as f64 * 0.05) * cos(y as f64 * 0.09));
                data.push(a.round().clamp(0.0, 255.0) as u8);
            }
        }
    }
    Image {
        width,
        height,
        channels,
        data,
    }
}

fn audio(rate: u32, seconds: f64, channels: usize, bits: u16, seed: u64) -> Audio {
    // Harmonic tones with a slow chirp and a noise bed at -40 dB, so the
    // spectrum is neither a single line nor white.
    let mut rng = Rng::seed(seed);
    let n = (rate as f64 * seconds).round() as usize;
    let mut chans = Vec::new();
    for c in 0..channels {
        let base = 110.0 * (c + 2) as f64;
        let mut v = Vec::with_capacity(n);
        for i in 0..n {
            let t = i as f64 / rate as f64;
            let chirp = 1.0 + 0.02 * sin(t * 0.5);
            let mut s = 0.0;
            for h in 1..=6 {
                let f = base * h as f64 * chirp;
                if f < rate as f64 * 0.45 {
                    s += sin(2.0 * std::f64::consts::PI * f * t) / h as f64;
                }
            }
            s *= 0.3;
            s += rng.gaussian() * 0.002;
            v.push(s.clamp(-1.0, 1.0));
        }
        chans.push(v);
    }
    Audio {
        rate,
        bits,
        channels: chans,
    }
}

fn main() {
    let out: PathBuf = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("fixtures/calibration"));
    std::fs::create_dir_all(&out).expect("create the fixture directory");
    let mut assets: Vec<ManifestAsset> = Vec::new();
    let mut add = |name: &str,
                   bytes: &[u8],
                   format: &str,
                   w: Option<usize>,
                   h: Option<usize>,
                   dur: Option<f64>,
                   rate: Option<u32>,
                   ch: Option<u32>,
                   class: &str| {
        std::fs::write(out.join(name), bytes).expect("write fixture");
        assets.push(ManifestAsset {
            path: name.to_string(),
            sha256: calibrate::sha256_hex(bytes),
            container: format.to_string(),
            width: w.map(|v| v as u32),
            height: h.map(|v| v as u32),
            duration_s: dur,
            rate,
            channels: ch,
            generator: "unmark calibration_fixtures (synthetic)".to_string(),
            content_class: class.to_string(),
            post_processed: false,
            ladder_step: String::new(),
            control: false,
            fixture: true,
            round: "fixture".to_string(),
            tags: vec!["synthetic".to_string()],
            doc_id: name
                .trim_end_matches(|c| c != '.')
                .trim_end_matches('.')
                .to_string(),
            derived: "none".to_string(),
            derived_from: String::new(),
            documented_marks: Vec::new(),
            near_duplicate_of: None,
            // The generator wrote these at their native rate and container.
            native_rate: Some(true),
            native_origin: Some(true),
            source_quality: None,
            variance_ok: None,
            band: None,
            fields: BTreeMap::new(),
        });
    };

    // Images: small at or under 640, medium to 1280, large above. Edges not
    // divisible by 16 exercise MCU padding.
    let png_s = image(320, 200, 3, 1);
    add(
        "png-small.png",
        &codec::png::encode(&png_s).unwrap(),
        "png",
        Some(320),
        Some(200),
        None,
        None,
        None,
        "dense texture",
    );
    let png_m = image(900, 40, 4, 2);
    add(
        "png-medium-alpha.png",
        &codec::png::encode(&png_m).unwrap(),
        "png",
        Some(900),
        Some(40),
        None,
        None,
        None,
        "dense texture",
    );
    let png_l = image(1400, 32, 3, 3);
    add(
        "png-large.png",
        &codec::png::encode(&png_l).unwrap(),
        "png",
        Some(1400),
        Some(32),
        None,
        None,
        None,
        "dense texture",
    );

    let jpg_s = image(320, 208, 3, 4);
    add(
        "jpeg-small.jpg",
        &codec::jpeg::encode(&jpg_s, 95, "4:4:4").unwrap(),
        "jpeg",
        Some(320),
        Some(208),
        None,
        None,
        None,
        "dense texture",
    );
    let jpg_m = image(904, 40, 3, 5);
    add(
        "jpeg-medium.jpg",
        &codec::jpeg::encode(&jpg_m, 95, "4:4:4").unwrap(),
        "jpeg",
        Some(904),
        Some(40),
        None,
        None,
        None,
        "dense texture",
    );
    let jpg_l = image(1408, 32, 3, 6);
    add(
        "jpeg-large.jpg",
        &codec::jpeg::encode(&jpg_l, 95, "4:4:4").unwrap(),
        "jpeg",
        Some(1408),
        Some(32),
        None,
        None,
        None,
        "dense texture",
    );

    let webp_s = image(320, 200, 4, 7);
    add(
        "webp-small-alpha.webp",
        &codec::webp::encode_lossless(&webp_s).unwrap(),
        "webp",
        Some(320),
        Some(200),
        None,
        None,
        None,
        "dense texture",
    );
    let webp_m = image(900, 40, 3, 8);
    add(
        "webp-medium.webp",
        &codec::webp::encode_lossless(&webp_m).unwrap(),
        "webp",
        Some(900),
        Some(40),
        None,
        None,
        None,
        "dense texture",
    );
    let webp_l = image(1300, 32, 3, 9);
    add(
        "webp-large.webp",
        &codec::webp::encode_lossless(&webp_l).unwrap(),
        "webp",
        Some(1300),
        Some(32),
        None,
        None,
        None,
        "dense texture",
    );

    // Audio: short at or under 10 s, medium to 60 s, long above. Lower rates
    // on the longer clips keep the subset small.
    let wav_s = audio(44100, 1.0, 2, 16, 11);
    add(
        "wav-short-stereo.wav",
        &codec::wav::encode(&wav_s, 16).unwrap(),
        "wav",
        None,
        None,
        Some(1.0),
        Some(44100),
        Some(2),
        "tonal synthetic",
    );
    let wav_m = audio(8000, 12.0, 1, 24, 12);
    add(
        "wav-medium-24bit.wav",
        &codec::wav::encode(&wav_m, 24).unwrap(),
        "wav",
        None,
        None,
        Some(12.0),
        Some(8000),
        Some(1),
        "tonal synthetic",
    );
    let wav_l = audio(4000, 61.0, 1, 16, 13);
    add(
        "wav-long.wav",
        &codec::wav::encode(&wav_l, 16).unwrap(),
        "wav",
        None,
        None,
        Some(61.0),
        Some(4000),
        Some(1),
        "tonal synthetic",
    );

    let flac_s = audio(48000, 1.0, 2, 16, 14);
    add(
        "flac-short-stereo.flac",
        &codec::flac::encode(&flac_s, 16).unwrap(),
        "flac",
        None,
        None,
        Some(1.0),
        Some(48000),
        Some(2),
        "tonal synthetic",
    );
    let flac_m = audio(8000, 11.0, 1, 24, 15);
    add(
        "flac-medium-24bit.flac",
        &codec::flac::encode(&flac_m, 24).unwrap(),
        "flac",
        None,
        None,
        Some(11.0),
        Some(8000),
        Some(1),
        "tonal synthetic",
    );
    let flac_l = audio(8000, 62.0, 1, 16, 16);
    add(
        "flac-long.flac",
        &codec::flac::encode(&flac_l, 16).unwrap(),
        "flac",
        None,
        None,
        Some(62.0),
        Some(8000),
        Some(1),
        "tonal synthetic",
    );

    let manifest = Manifest {
        schema_version: calibrate::MANIFEST_SCHEMA_VERSION.to_string(),
        source_inventory: None,
        prepare: None,
        assets,
    };
    let mut json = serde_json::to_string_pretty(&manifest).unwrap();
    json.push('\n');
    std::fs::write(out.join("manifest.json"), json).expect("write manifest");
    eprintln!("wrote {}", out.display());
}
