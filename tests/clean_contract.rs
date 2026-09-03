//! The clean contract under the strip-everything posture: one default run
//! per container, every opt-out a flag that turns a strip off, the certified
//! capture rule, the uncertain-EXIF hint, and a report that never emits a
//! clean verdict and never says "weakened".

mod common;
use common::*;
use unmark::capture::CaptureStatus;
use unmark::report::{EXIT_OK, EXIT_USAGE};
use unmark::scan::ScanState;
use unmark::{clean, inspect, plan, policy, Options, UnmarkError, VerifyOutcome};

fn pkg() -> policy::PolicyPackage {
    policy::load().expect("policy loads")
}

fn keep(items: &[&str]) -> Options {
    Options {
        keep: items.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    }
}

#[test]
fn the_default_run_strips_every_confirmable_mark_and_proves_it_gone() {
    let png = build_png(&PngOpts {
        text: true,
        exif: true,
        xmp: true,
        c2pa: true,
        ..Default::default()
    });
    let out = clean(&png, &Options::default(), &pkg()).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
    let bytes = out.output.expect("the run writes");
    let after = unmark::detect::inspect(&bytes);
    for class in ["png_text", "exif", "xmp", "c2pa"] {
        assert_eq!(
            after.get(class).map(|d| d.state),
            Some(ScanState::ConfirmedAbsent),
            "{class} remains"
        );
    }
    for label in [
        "PNG text chunk",
        "EXIF metadata",
        "XMP packet",
        "C2PA manifest",
    ] {
        assert!(
            out.report
                .stripped_and_proven_gone
                .iter()
                .any(|l| l == label),
            "{label} not listed as stripped: {:?}",
            out.report.stripped_and_proven_gone
        );
    }
    assert!(out.report.kept.is_empty());
    assert_eq!(out.report.capture.status, "none");
    // The one-pixel placeholder is too small for the resize to matter, but
    // the output stays a PNG.
    assert_eq!(out.report.output_format, "png");
}

#[test]
fn keep_turns_a_strip_off_and_is_reported_as_kept_by_flag() {
    let png = build_png(&PngOpts {
        text: true,
        exif: true,
        ..Default::default()
    });
    let out = clean(&png, &keep(&["exif"]), &pkg()).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK);
    let after = unmark::detect::inspect(&out.output.unwrap());
    assert_eq!(
        after.get("exif").map(|d| d.state),
        Some(ScanState::ConfirmedPresent),
        "--keep exif preserves EXIF"
    );
    assert_eq!(
        after.get("png_text").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
    let kept = out
        .report
        .kept
        .iter()
        .find(|k| k.item.starts_with("MC03"))
        .expect("MC03 reported kept");
    assert!(kept.reason.contains("kept by flag --keep exif"));
    let action = out
        .report
        .actions
        .iter()
        .find(|a| a.transform == "MC03")
        .unwrap();
    assert!(action.outcome.contains("kept by flag"));
    // A keep by transform id and by transform name work the same way.
    for k in ["MC03", "strip-exif"] {
        let out = clean(&png, &keep(&[k]), &pkg()).unwrap();
        assert!(out.report.kept.iter().any(|k| k.item.starts_with("MC03")));
    }
}

#[test]
fn an_unknown_keep_is_a_usage_error() {
    let png = build_png(&PngOpts::default());
    let err = clean(&png, &keep(&["nothing-like-this"]), &pkg()).unwrap_err();
    assert!(matches!(err, UnmarkError::Usage(_)));
    let err = inspect(&png, &keep(&["nothing-like-this"]), &pkg()).unwrap_err();
    assert!(matches!(err, UnmarkError::Usage(_)));
}

#[test]
fn certified_capture_is_byte_identical_by_default_and_strips_under_the_flag() {
    let png = build_png(&PngOpts {
        text: true,
        c2pa: true,
        c2pa_payload: Some(capture_store(Some("Leica Camera AG"))),
        ..Default::default()
    });
    let p = pkg();
    let reading = unmark::capture::read_capture(
        &png,
        &unmark::detect::inspect(&png),
        unmark::asset::Format::Png,
    );
    assert_eq!(reading.status, CaptureStatus::Certified);
    let out = clean(&png, &Options::default(), &p).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK);
    assert_eq!(
        out.output.as_deref(),
        Some(png.as_slice()),
        "byte-identical"
    );
    assert_eq!(out.report.capture.status, "certified");
    assert!(out.report.capture.kept);
    assert!(out
        .report
        .capture
        .line
        .starts_with("Certified capture kept. Claim: `"));
    assert!(out
        .report
        .capture
        .line
        .ends_with("Signature status: not signature-verified."));
    assert!(out
        .report
        .kept
        .iter()
        .any(|k| k.reason.contains("certified capture")));
    assert!(out
        .report
        .actions
        .iter()
        .all(|a| a.outcome == "kept: certified capture"));

    let strip = Options {
        strip_capture: true,
        ..Default::default()
    };
    let out = clean(&png, &strip, &p).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK);
    assert!(!out.report.capture.kept);
    assert!(out.report.capture.line.contains("--strip-capture"));
    let after = unmark::detect::inspect(&out.output.unwrap());
    assert_eq!(
        after.get("c2pa").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
    assert!(out
        .report
        .stripped_and_proven_gone
        .iter()
        .any(|l| l == "C2PA manifest"));
}

#[test]
fn a_capture_claim_with_a_later_generative_action_is_stripped_by_default() {
    let png = build_png(&PngOpts {
        c2pa: true,
        c2pa_payload: Some(manifest_store(&[ManifestSpec::new("urn:uuid:gen-1")
            .action("c2pa.created", Some(DIGITAL_CAPTURE_URI), None)
            .action(
                "c2pa.edited",
                Some(TRAINED_MEDIA_URI),
                Some("Adobe Firefly"),
            )])),
        ..Default::default()
    });
    let out = clean(&png, &Options::default(), &pkg()).unwrap();
    assert_eq!(out.report.capture.status, "generative");
    assert!(!out.report.capture.kept);
    let after = unmark::detect::inspect(&out.output.unwrap());
    assert_eq!(
        after.get("c2pa").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
}

#[test]
fn a_publisher_manifest_without_a_capture_action_is_stripped_by_default() {
    let jpg = build_jpeg(&JpegOpts {
        c2pa: true,
        c2pa_payload: Some(manifest_store(&[ManifestSpec::new("urn:uuid:pub-1")
            .generator("Reuters Newsroom/2.0")
            .action("c2pa.published", None, None)])),
        ..Default::default()
    });
    let out = clean(&jpg, &Options::default(), &pkg()).unwrap();
    assert_eq!(out.report.capture.status, "none");
    assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
    let after = unmark::detect::inspect(&out.output.unwrap());
    assert_eq!(
        after.get("c2pa").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
    // --keep c2pa preserves it.
    let out = clean(&jpg, &keep(&["c2pa"]), &pkg()).unwrap();
    let after = unmark::detect::inspect(&out.output.unwrap());
    assert_eq!(
        after.get("c2pa").map(|d| d.state),
        Some(ScanState::ConfirmedPresent)
    );
}

#[test]
fn keyword_text_in_a_manifest_chunk_is_not_a_capture_claim_and_is_stripped() {
    // Bytes that carry every capture keyword but no JUMBF structure.
    let keyworded: &[&[u8]] = &[
        b"not-a-manifest c2pa.created digitalCapture",
        b"jumb c2pa c2pa.created digitalSourceType digitalCapture signer Leica Camera AG",
    ];
    for payload in keyworded {
        let png = build_png(&PngOpts {
            text: true,
            c2pa: true,
            c2pa_payload: Some(payload.to_vec()),
            ..Default::default()
        });
        let out = clean(&png, &Options::default(), &pkg()).unwrap();
        assert_eq!(out.report.capture.status, "none");
        assert!(!out.report.capture.kept);
        let after = unmark::detect::inspect(&out.output.unwrap());
        assert_eq!(
            after.get("c2pa").map(|d| d.state),
            Some(ScanState::ConfirmedAbsent),
            "keyword bytes were kept as a capture"
        );
    }
    // A store whose claim never references its actions assertion is not
    // well-formed, and neither is a truncated store.
    let unreferenced = manifest_store(&[ManifestSpec::new("urn:uuid:bad-1")
        .action("c2pa.created", Some(DIGITAL_CAPTURE_URI), None)
        .unreferenced_actions()]);
    let good = capture_store(Some("Leica Camera AG"));
    let truncated = good[..good.len() - 40].to_vec();
    for payload in [unreferenced, truncated] {
        let png = build_png(&PngOpts {
            c2pa: true,
            c2pa_payload: Some(payload),
            ..Default::default()
        });
        let out = clean(&png, &Options::default(), &pkg()).unwrap();
        assert_eq!(out.report.capture.status, "none");
        let after = unmark::detect::inspect(&out.output.unwrap());
        assert_eq!(
            after.get("c2pa").map(|d| d.state),
            Some(ScanState::ConfirmedAbsent)
        );
    }
}

#[test]
fn a_well_formed_capture_store_is_certified_in_every_carriage() {
    // The JPEG box carriage, the PNG chunk, and a signer-only claim.
    let jpg = build_jpeg(&JpegOpts {
        c2pa: true,
        c2pa_payload: Some(capture_store(None)),
        ..Default::default()
    });
    let out = clean(&jpg, &Options::default(), &pkg()).unwrap();
    assert_eq!(
        out.report.capture.status, "certified",
        "{:?}",
        out.report.capture
    );
    assert_eq!(out.output.as_deref(), Some(jpg.as_slice()));
    let signer_only = manifest_store(&[ManifestSpec::new("urn:uuid:cam-1")
        .action("c2pa.created", None, None)
        .signer("Leica Camera AG")]);
    let png = build_png(&PngOpts {
        c2pa: true,
        c2pa_payload: Some(signer_only),
        ..Default::default()
    });
    let out = clean(&png, &Options::default(), &pkg()).unwrap();
    assert_eq!(out.report.capture.status, "certified");
    assert!(out
        .report
        .capture
        .claim
        .as_deref()
        .unwrap_or("")
        .contains("Leica"));
    // The same signer with a later manifest that names a generative tool is
    // not certified.
    let chain = manifest_store(&[
        ManifestSpec::new("urn:uuid:cam-2")
            .action("c2pa.created", Some(DIGITAL_CAPTURE_URI), None)
            .signer("Leica Camera AG"),
        ManifestSpec::new("urn:uuid:edit-2")
            .action("c2pa.opened", None, None)
            .action("c2pa.edited", None, Some("Adobe Firefly")),
    ]);
    let png = build_png(&PngOpts {
        c2pa: true,
        c2pa_payload: Some(chain),
        ..Default::default()
    });
    let out = clean(&png, &Options::default(), &pkg()).unwrap();
    assert_eq!(out.report.capture.status, "generative");
    assert!(!out.report.capture.kept);
}

#[test]
fn camera_exif_without_a_claim_is_uncertain_and_stripped_with_the_hint() {
    let jpg = build_jpeg(&JpegOpts {
        exif: true,
        exif_payload: Some(camera_tiff()),
        ..Default::default()
    });
    let out = clean(&jpg, &Options::default(), &pkg()).unwrap();
    assert_eq!(out.report.capture.status, "uncertain");
    assert!(out
        .report
        .capture
        .line
        .starts_with("Capture uncertain. Hint: `"));
    assert!(out
        .report
        .capture
        .line
        .ends_with("Stripped by default. Use `--keep exif` to preserve EXIF."));
    let after = unmark::detect::inspect(&out.output.unwrap());
    assert_eq!(
        after.get("exif").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
}

#[test]
fn the_report_never_says_clean_or_weakened_and_names_every_survivor() {
    let png = build_png(&PngOpts {
        text: true,
        ..Default::default()
    });
    let out = clean(&png, &Options::default(), &pkg()).unwrap();
    let json = serde_json::to_string(&out.report)
        .unwrap()
        .to_ascii_lowercase();
    let text = unmark::report::render_text(&out.report).to_ascii_lowercase();
    for banned in [
        "weakened",
        "is clean",
        "clean verdict",
        "human-authored",
        "human authored",
    ] {
        assert!(!json.contains(banned), "report json says {banned}");
        assert!(!text.contains(banned), "report text says {banned}");
    }
    assert!(
        !out.report.survived.is_empty(),
        "blind classes are named as surviving"
    );
    for s in &out.report.survived {
        assert!(!s.evidence.is_empty(), "{} has no evidence", s.mark);
    }
    assert!(out
        .report
        .survived
        .iter()
        .any(|s| s.mark == "SynthID-Image" && s.evidence.starts_with("applied, survives")));
}

#[test]
fn plan_proposes_the_default_run_and_touches_nothing() {
    let png = build_png(&PngOpts {
        text: true,
        ..Default::default()
    });
    let r = plan(&png, &Options::default(), &pkg()).unwrap();
    assert!(r
        .actions
        .iter()
        .any(|a| a.transform == "MC04" && a.outcome == "proposed"));
    assert!(r.actions.iter().any(|a| a.transform == "PX02"));
    let r = plan(
        &png,
        &Options {
            no_degrade: true,
            ..Default::default()
        },
        &pkg(),
    )
    .unwrap();
    assert!(r
        .actions
        .iter()
        .any(|a| a.transform == "PX02" && a.outcome.contains("--no-degrade")));
}

#[test]
fn unknown_container_is_unsupported() {
    let blob = vec![0xC0u8; 40];
    let err = clean(&blob, &Options::default(), &pkg()).unwrap_err();
    assert!(matches!(err, UnmarkError::Unsupported(_)));
}

#[test]
fn verify_confirms_a_cleaned_output_and_catches_a_reintroduced_mark() {
    let png = build_png(&PngOpts {
        text: true,
        c2pa: true,
        ..Default::default()
    });
    let p = pkg();
    let out = clean(&png, &Options::default(), &p).unwrap();
    let report_json = serde_json::to_string(&out.report).unwrap();
    let bytes = out.output.unwrap();
    assert_eq!(
        unmark::verify(&bytes, &report_json, &p),
        VerifyOutcome::Verified
    );
    match unmark::verify(&png, &report_json, &p) {
        VerifyOutcome::Mismatch(problems) => {
            assert!(problems.iter().any(|m| m.contains("present again")))
        }
        VerifyOutcome::Verified => panic!("the original still carries the marks"),
    }
}

#[test]
fn the_usage_exit_is_distinct() {
    assert_eq!(EXIT_USAGE, 2);
    assert_ne!(
        unmark::report::EXIT_SANITY,
        unmark::report::EXIT_INSTRUMENTATION
    );
    let codes = [
        unmark::report::EXIT_OK,
        EXIT_USAGE,
        unmark::report::EXIT_MARK_REMAINS,
        unmark::report::EXIT_INSTRUMENTATION,
        unmark::report::EXIT_UNSUPPORTED,
        unmark::report::EXIT_SANITY,
    ];
    let mut sorted = codes.to_vec();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), codes.len(), "every exit is distinct");
}
