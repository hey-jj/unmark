//! The fidelity-budget machinery. Milestone 1 exercises the byte-identity gate.
//! These tests also cover the metric functions and the two-cost split so the
//! pixel and audio transforms can use a tested gate.

use unmark::budget::{self, Budget, Cost};

#[test]
fn byte_identity_gate_passes_on_equal_streams_and_fails_otherwise() {
    assert!(budget::check_byte_identity(b"abc", b"abc").is_ok());
    assert!(budget::check_byte_identity(b"abc", b"abd").is_err());
}

fn image(w: usize, h: usize, seed: usize) -> Vec<u8> {
    (0..w * h * 3)
        .map(|i| ((i * 31 + seed * 7) % 256) as u8)
        .collect()
}

#[test]
fn identical_buffers_score_at_the_ceiling() {
    let a = image(16, 16, 0);
    assert!(budget::psnr(&a, &a).unwrap() >= 100.0);
    assert!((budget::ssim(&a, &a, 16, 16, 3).unwrap() - 1.0).abs() < 1e-9);
}

#[test]
fn a_degraded_buffer_scores_below_the_ceiling() {
    let a = image(16, 16, 0);
    let b = image(16, 16, 5);
    let p = budget::psnr(&a, &b).unwrap();
    assert!(
        p < 60.0,
        "a changed buffer should score a finite PSNR, got {p}"
    );
    let s = budget::ssim(&a, &b, 16, 16, 3).unwrap();
    assert!(s < 1.0);
}

#[test]
fn mismatched_lengths_and_sub_window_images_have_no_defined_metric() {
    assert!(budget::psnr(&[1, 2, 3], &[1, 2]).is_none());
    let a = image(16, 16, 0);
    let b = image(16, 15, 0);
    assert!(budget::ssim(&a, &b, 16, 16, 3).is_none());
    let small = image(7, 7, 0);
    assert!(budget::ssim(&small, &small, 7, 7, 3).is_none());
}

#[test]
fn ssim_is_windowed_rather_than_global() {
    // Two flat images of different brightness: a global window scores the
    // mean shift only, while the windowed mean scores it in every window.
    // A localized corruption in one window must move the score by that
    // window's share and no more.
    let a = image(32, 32, 0);
    let mut b = a.clone();
    for y in 0..8 {
        for x in 0..8 {
            for c in 0..3 {
                b[(y * 32 + x) * 3 + c] = 255 - b[(y * 32 + x) * 3 + c];
            }
        }
    }
    let s = budget::ssim(&a, &b, 32, 32, 3).unwrap();
    // Sixteen windows, one inverted: that window scores near -1 and the
    // mean sits near 14/16, where a global window would barely move.
    assert!(s > 0.80 && s < 0.95, "windowed SSIM {s}");
}

#[test]
fn log_spectral_distance_is_zero_for_identity_and_refuses_short_clips() {
    let p = budget::LsdParams::PINNED;
    let x: Vec<f64> = (0..8192).map(|i| (0.02 * i as f64).sin() * 0.5).collect();
    assert!(budget::lsd(&x, &x, &p).unwrap().abs() < 1e-12);
    assert!(
        budget::averaged_spectrum_distance(&x, &x, &p)
            .unwrap()
            .abs()
            < 1e-12
    );
    let short: Vec<f64> = x[..4000].to_vec();
    assert!(budget::lsd(&short, &short, &p).is_none());
    // A gain change of 6 dB on a tone shows as roughly 6 dB on the tonal
    // bins and nothing on floored bins, so the distance is finite and positive.
    let y: Vec<f64> = x.iter().map(|v| v * 0.5).collect();
    let d = budget::lsd(&x, &y, &p).unwrap();
    assert!(d > 0.0 && d < 7.0, "lsd {d}");
}

#[test]
fn the_two_cost_split_bounds_signal_and_geometry_separately() {
    let img = Budget::Image {
        psnr_floor_db: 38.0,
        ssim_floor: 0.98,
        resample_ratio_min: 0.75,
        crop_area_min: 1.0,
    };
    // Signal below the floor fails on the signal cost.
    let mut cost = Cost::none();
    cost.psnr_db = Some(30.0);
    cost.ssim = Some(0.99);
    assert!(budget::check(&img, &cost).is_err());

    // Signal fine but geometry below the ratio floor fails on the geometry cost.
    let mut cost = Cost::none();
    cost.psnr_db = Some(41.0);
    cost.ssim = Some(0.99);
    cost.resample_ratio = 0.5;
    assert!(budget::check(&img, &cost).is_err());

    // Both within budget passes.
    let mut cost = Cost::none();
    cost.psnr_db = Some(41.0);
    cost.ssim = Some(0.99);
    cost.resample_ratio = 0.8;
    assert!(budget::check(&img, &cost).is_ok());
}

#[test]
fn audio_budget_is_a_ceiling() {
    let audio = Budget::Audio {
        lsd_ceiling_db: 1.0,
        resample_ratio_min: 0.9,
        time_stretch_min: 1.0,
        time_stretch_max: 1.0,
    };
    let mut cost = Cost::none();
    cost.lsd_db = Some(2.0);
    assert!(
        budget::check(&audio, &cost).is_err(),
        "LSD over the ceiling must fail"
    );
    let mut cost = Cost::none();
    cost.lsd_db = Some(0.5);
    assert!(budget::check(&audio, &cost).is_ok());
}
