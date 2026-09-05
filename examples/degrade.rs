//! The held-transform measurement: for every image under a directory,
//! apply the border crop, the rotation, and the blur each alone at the
//! strengths given, report the in-crate dwtDct agreement before and after
//! and the PSNR and SSIM of the result against the input (a geometric
//! result is resampled back to the input grid first), and write each result
//! as a PNG under OUT so the oracle can decode it.
//!
//!     cargo run --release --example degrade -- DIR OUT [--rotate 75] [--blur 4] [--crop 32,0.1]

use std::path::PathBuf;
use unmark::budget;
use unmark::codec::{self, Image};
use unmark::dsp;
use unmark::mark::dwtdct;
use unmark::transform::pixel;

fn fidelity(input: &Image, out: &Image) -> (Option<f64>, Option<f64>) {
    let back = if out.width != input.width || out.height != input.height {
        Image {
            width: input.width,
            height: input.height,
            channels: out.channels,
            data: dsp::resample_lanczos3(
                &out.data,
                out.width,
                out.height,
                out.channels,
                input.width,
                input.height,
            ),
        }
    } else {
        out.clone()
    };
    (
        budget::psnr(&input.data, &back.data),
        budget::ssim(
            &input.data,
            &back.data,
            input.width,
            input.height,
            input.channels,
        ),
    )
}

fn agreements(img: &Image) -> Vec<String> {
    dwtdct::detect(img)
        .iter()
        .map(|d| format!("\"{}\":{:.3}", d.payload, d.agreement))
        .collect()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut positional: Vec<String> = Vec::new();
    let (mut rot, mut blur_sigma, mut crop) = (75.0f64, 4.0f64, (32usize, 0.1f64));
    while let Some(a) = args.next() {
        match a.as_str() {
            "--rotate" => rot = args.next().unwrap_or_default().parse().unwrap_or(rot),
            "--blur" => {
                blur_sigma = args
                    .next()
                    .unwrap_or_default()
                    .parse()
                    .unwrap_or(blur_sigma)
            }
            "--crop" => {
                let v = args.next().unwrap_or_default();
                let mut it = v.split(',');
                let px = it.next().and_then(|s| s.parse().ok()).unwrap_or(crop.0);
                let cap = it.next().and_then(|s| s.parse().ok()).unwrap_or(crop.1);
                crop = (px, cap);
            }
            other => positional.push(other.to_string()),
        }
    }
    if positional.len() != 2 {
        eprintln!("usage: degrade DIR OUT [--rotate DEG] [--blur SIGMA] [--crop PX,CAP]");
        std::process::exit(2);
    }
    let dir = PathBuf::from(&positional[0]);
    let out_dir = PathBuf::from(&positional[1]);
    std::fs::create_dir_all(&out_dir).unwrap();
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
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let before = agreements(&img);
        let mut cells = Vec::new();
        for (name, out) in [
            ("crop", pixel::crop_border(&img, crop.0, crop.1)),
            ("rotate", pixel::rotate(&img, rot)),
            ("blur", pixel::blur(&img, blur_sigma)),
        ] {
            let after = agreements(&out);
            let (psnr, ssim) = fidelity(&img, &out);
            let png = codec::png::encode(&out).unwrap();
            std::fs::write(out_dir.join(format!("{stem}-{name}.png")), png).unwrap();
            cells.push(format!(
                "\"{name}\":{{\"after\":{{{}}},\"psnr_db\":{},\"ssim\":{},\"width\":{},\"height\":{}}}",
                after.join(","),
                psnr.map(|v| format!("{v:.2}")).unwrap_or_else(|| "null".into()),
                ssim.map(|v| format!("{v:.4}")).unwrap_or_else(|| "null".into()),
                out.width,
                out.height
            ));
        }
        println!(
            "{{\"file\":\"{}\",\"before\":{{{}}},{}}}",
            path.file_name().unwrap().to_string_lossy(),
            before.join(","),
            cells.join(",")
        );
    }
}
