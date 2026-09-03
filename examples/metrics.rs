//! Metrics over a directory: the fidelity study kept from the calibration
//! work. For every image under a directory it reports the dwtDct decisions
//! and, at each resize ratio of a sweep, whether the mark still decodes and
//! the PSNR and SSIM of the round trip (resize down, resize back to the input
//! grid). This is how the pinned resize ratio was measured and presented.
//!
//!     cargo run --release --example metrics -- DIR [--ratios 0.95,0.9,...] [--jpeg]
//!
//! Output is one JSON object per file on stdout. Nothing is written to disk.

use std::path::PathBuf;
use unmark::budget;
use unmark::codec::{self, Image};
use unmark::mark::dwtdct;
use unmark::transform::pixel;

fn main() {
    let mut args = std::env::args().skip(1);
    let mut dir: Option<PathBuf> = None;
    let mut ratios: Vec<f64> = vec![0.95, 0.9, 0.85, 0.8, 0.75, 0.7, 0.65, 0.6, 0.55, 0.5];
    let mut jpeg = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--ratios" => {
                ratios = args
                    .next()
                    .unwrap_or_default()
                    .split(',')
                    .filter_map(|r| r.trim().parse().ok())
                    .collect();
            }
            "--jpeg" => jpeg = true,
            other => dir = Some(PathBuf::from(other)),
        }
    }
    let Some(dir) = dir else {
        eprintln!("usage: metrics DIR [--ratios r1,r2,...] [--jpeg]");
        std::process::exit(2);
    };
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("directory")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file())
        .collect();
    files.sort();
    for path in files {
        let bytes = std::fs::read(&path).unwrap();
        let format = unmark::asset::sniff(&bytes);
        let Ok(img) = codec::decode_image(&bytes, format) else {
            continue;
        };
        let before = dwtdct::detect(&img);
        let mut sweep = Vec::new();
        for r in &ratios {
            let small = pixel::resize(&img, *r);
            // The written form the run would produce: JPEG at the pin when
            // asked, otherwise the lossless pixels.
            let written: Image = if jpeg {
                let bytes = codec::jpeg::encode(&small, 92, "4:4:4").unwrap();
                codec::jpeg::decode(&bytes).unwrap()
            } else {
                small.clone()
            };
            let after = dwtdct::detect(&written);
            let back = Image {
                width: img.width,
                height: img.height,
                channels: img.channels,
                data: unmark::dsp::resample_lanczos3(
                    &written.data,
                    written.width,
                    written.height,
                    written.channels,
                    img.width,
                    img.height,
                ),
            };
            let psnr = budget::psnr(&img.data, &back.data);
            let ssim = budget::ssim(&img.data, &back.data, img.width, img.height, img.channels);
            sweep.push(serde_json::json!({
                "ratio": r,
                "width": written.width,
                "height": written.height,
                "present": after.iter().any(|d| d.present),
                "agreement": after.iter().map(|d| serde_json::json!({"payload": d.payload, "agreement": d.agreement})).collect::<Vec<_>>(),
                "round_trip_psnr_db": psnr.map(|v| (v * 100.0).round() / 100.0),
                "round_trip_ssim": ssim.map(|v| (v * 10000.0).round() / 10000.0),
            }));
        }
        let out = serde_json::json!({
            "file": path.file_name().map(|n| n.to_string_lossy().to_string()),
            "width": img.width,
            "height": img.height,
            "before": before,
            "sweep": sweep,
        });
        println!("{out}");
    }
}
