//! The fidelity-budget machinery. Milestone 1 exercises the byte-identity gate;
//! these tests also cover the metric functions and the two-cost split so the
//! pixel and audio transforms drop in against a proven gate.

use unmark::budget::{self, Budget, Cost};

#[test]
fn byte_identity_gate_passes_on_equal_streams_and_fails_otherwise() {
    assert!(budget::check_byte_identity(b"abc", b"abc").is_ok());
    assert!(budget::check_byte_identity(b"abc", b"abd").is_err());
}

#[test]
fn identical_buffers_score_at_the_ceiling() {
    let a = vec![10u8, 20, 30, 40, 50];
    assert!(budget::psnr(&a, &a).unwrap() >= 100.0);
    assert!((budget::ssim(&a, &a).unwrap() - 1.0).abs() < 1e-9);
}

#[test]
fn a_degraded_buffer_scores_below_the_ceiling() {
    let a = vec![10u8, 20, 30, 40, 50, 60, 70, 80];
    let b = vec![18u8, 12, 44, 30, 66, 48, 90, 70];
    let p = budget::psnr(&a, &b).unwrap();
    assert!(
        p < 60.0,
        "a changed buffer should score a finite PSNR, got {p}"
    );
    let s = budget::ssim(&a, &b).unwrap();
    assert!(s < 1.0);
}

#[test]
fn mismatched_lengths_have_no_defined_metric() {
    assert!(budget::psnr(&[1, 2, 3], &[1, 2]).is_none());
    assert!(budget::ssim(&[1, 2, 3], &[1, 2]).is_none());
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
