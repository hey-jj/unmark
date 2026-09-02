//! The clean verb's exit contract, the guardrails, and the honesty core.

mod common;
use common::*;
use unmark::report::{EXIT_MARK_REMAINS, EXIT_OK, EXIT_RESIDUAL, EXIT_UNSUPPORTED};
use unmark::{clean, inspect, policy, Options, UnmarkError};

fn pkg() -> policy::PolicyPackage {
    policy::load().expect("policy loads")
}

fn owner_opts() -> Options {
    Options {
        i_generated_this: true,
        ..Default::default()
    }
}

#[test]
fn clean_requires_ownership_assertion() {
    let png = build_png(&PngOpts {
        text: true,
        ..Default::default()
    });
    let err = clean(&png, "image-metadata", &Options::default(), &pkg()).unwrap_err();
    assert!(
        matches!(err, UnmarkError::Usage(_)),
        "missing ownership must be a usage error"
    );
}

#[test]
fn clean_gates_on_residual_then_passes_on_acknowledgment() {
    let png = build_png(&PngOpts {
        text: true,
        c2pa: true,
        ..Default::default()
    });
    // Without acknowledgment the run gates at exit 20.
    let out = clean(&png, "image-metadata", &owner_opts(), &pkg()).unwrap();
    assert_eq!(out.report.exit_code, EXIT_RESIDUAL);
    assert!(
        out.output.is_none(),
        "no output while a residual is unacknowledged"
    );
    assert!(out.report.residual_acknowledgment_required);

    // With acknowledgment the run reaches exit 0 and writes output.
    let opts = Options {
        i_generated_this: true,
        acknowledge_residual: true,
        ..Default::default()
    };
    let out = clean(&png, "image-metadata", &opts, &pkg()).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK);
    let bytes = out.output.expect("exit 0 writes output");
    // The confirmable marks are proven gone.
    let det = inspect(&bytes, "image-metadata", &pkg()).unwrap();
    assert!(det
        .scan_states
        .iter()
        .find(|s| s.class == "png_text")
        .map(|s| s.state == unmark::scan::ScanState::ConfirmedAbsent)
        .unwrap_or(false));
}

#[test]
fn report_never_emits_a_clean_verdict() {
    let png = build_png(&PngOpts {
        text: true,
        ..Default::default()
    });
    let opts = Options {
        i_generated_this: true,
        acknowledge_residual: true,
        ..Default::default()
    };
    let out = clean(&png, "image-metadata", &opts, &pkg()).unwrap();
    let json = serde_json::to_string(&out.report).unwrap();
    // The three parts are always present and blind classes are named as not
    // addressed rather than proven absent.
    assert!(json.contains("removed_and_proven"));
    assert!(json.contains("degraded_without_proof"));
    assert!(json.contains("not_addressed"));
    assert!(!out.report.removed_and_proven.is_empty());
    assert!(out
        .report
        .not_addressed
        .iter()
        .any(|s| s.contains("SynthID-Image")));
    // No field claims the asset is clean, unmarked, or human-authored.
    assert!(!json.contains("clean\":true"));
    assert!(!json.contains("\"unmarked\""));
    assert!(!json.to_lowercase().contains("human-authored"));
}

#[test]
fn g2_refuses_a_publisher_manifest_and_the_override_proceeds() {
    let jpg = build_jpeg(&JpegOpts {
        c2pa: true,
        c2pa_payload: Some(b"JP\0\0jumb c2pa store credit: Reuters".to_vec()),
        ..Default::default()
    });
    // The manifest names a third-party publisher, so the strip is refused.
    let out = clean(&jpg, "image-metadata", &owner_opts(), &pkg()).unwrap();
    assert_eq!(out.report.exit_code, EXIT_UNSUPPORTED);
    assert!(out.output.is_none());
    assert!(out
        .report
        .actions
        .iter()
        .any(|a| a.outcome.contains("refused")));

    // The explicit override proceeds past the refusal.
    let opts = Options {
        i_generated_this: true,
        acknowledge_residual: true,
        force_provenance_strip: true,
        ..Default::default()
    };
    let out = clean(&jpg, "image-metadata", &opts, &pkg()).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK);
}

#[test]
fn g2_does_not_fire_on_a_generative_claim_generator() {
    // A user's own generated output names the software, not a publisher. The
    // strip proceeds.
    let jpg = build_jpeg(&JpegOpts {
        c2pa: true,
        c2pa_payload: Some(b"JP\0\0jumb c2pa store claim_generator: ComfyUI".to_vec()),
        ..Default::default()
    });
    let opts = Options {
        i_generated_this: true,
        acknowledge_residual: true,
        ..Default::default()
    };
    let out = clean(&jpg, "image-metadata", &opts, &pkg()).unwrap();
    assert_eq!(
        out.report.exit_code, EXIT_OK,
        "a generative claim generator must not refuse"
    );
}

#[test]
fn g3_camera_origin_warns() {
    let jpg = build_jpeg(&JpegOpts {
        exif: true,
        exif_payload: Some(camera_tiff()),
        ..Default::default()
    });
    let r = inspect(&jpg, "image-metadata", &pkg()).unwrap();
    assert!(
        r.camera_origin_warning.is_some(),
        "a camera EXIF should raise the camera-origin warning"
    );
}

#[test]
fn inspect_residual_keys_on_asset_intrinsic_evidence() {
    // A PNG carrying a prompt chunk is asset-intrinsic evidence, so the blind
    // classes report at the residual tier on inspect.
    let generative = build_png(&PngOpts {
        text: true,
        ..Default::default()
    });
    let r = inspect(&generative, "image-metadata", &pkg()).unwrap();
    let synthid = r
        .findings
        .iter()
        .find(|f| f.class == "synthid_image")
        .unwrap();
    assert_eq!(synthid.tier, unmark::report::Tier::Residual);

    // A plain image with no such evidence reports the same class at note tier,
    // so inspect does not cry wolf on arbitrary files.
    let plain = build_png(&PngOpts::default());
    let r = inspect(&plain, "image-metadata", &pkg()).unwrap();
    let synthid = r
        .findings
        .iter()
        .find(|f| f.class == "synthid_image")
        .unwrap();
    assert_eq!(synthid.tier, unmark::report::Tier::Note);
}

#[test]
fn unknown_container_is_unsupported() {
    // Invalid UTF-8 bytes (0xC0 is never a valid lead) that match no container
    // sniff to Unknown.
    let blob = vec![0xC0u8; 40];
    let err = clean(&blob, "image-metadata", &owner_opts(), &pkg()).unwrap_err();
    assert!(matches!(err, UnmarkError::Unsupported(_)));
}

#[test]
fn non_v1_profile_is_rejected() {
    let png = build_png(&PngOpts::default());
    let err = inspect(&png, "image-safe", &pkg()).unwrap_err();
    assert!(
        matches!(err, UnmarkError::Usage(_)),
        "image-safe is milestone 2"
    );
}

#[test]
fn verify_confirms_a_cleaned_output() {
    let png = build_png(&PngOpts {
        text: true,
        c2pa: true,
        ..Default::default()
    });
    let opts = Options {
        i_generated_this: true,
        acknowledge_residual: true,
        ..Default::default()
    };
    let out = clean(&png, "image-metadata", &opts, &pkg()).unwrap();
    let bytes = out.output.unwrap();
    let report_json = serde_json::to_string(&out.report).unwrap();
    assert_eq!(
        unmark::verify(&bytes, &report_json, &pkg()),
        unmark::VerifyOutcome::Verified
    );
    // Verifying the original, uncleaned bytes fails.
    assert!(matches!(
        unmark::verify(&png, &report_json, &pkg()),
        unmark::VerifyOutcome::Mismatch(_)
    ));
    let _ = EXIT_MARK_REMAINS;
}
