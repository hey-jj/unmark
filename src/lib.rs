//! unmark strips every mark it can find by default and keeps provenance only
//! where a well-formed C2PA claim identifies a camera or sensor capture with
//! no later generative action. Capture claims are read, not
//! signature-verified. Every policy flag turns a strip off. The report lists
//! what was stripped and proven gone, what was kept and why, and what
//! survived.
//!
//! The pure core is small and named. `detect::inspect(bytes)` is a pure
//! function of the input. `plan_run` resolves a container and the opt-outs
//! into an ordered transform list. `clean` applies it, re-inspects the
//! output, and measures the sanity floor. The binary does the I/O.
//!
//! The tool never emits a clean verdict, and it never renders an empty
//! detection list as human authorship. A finding of no marks is a statement
//! about this build's reach over an enumerated set of containers, never about
//! the asset's origin.

pub mod asset;
pub mod budget;
pub mod capture;
pub mod cbor;
pub mod codec;
pub mod container;
pub mod detect;
pub mod dsp;
pub mod jumbf;
pub mod mark;
pub mod policy;
pub mod report;
pub mod scan;
pub mod skill;
pub mod transform;

use asset::Format;
use capture::{CaptureReading, CaptureStatus, SIGNATURE_STATUS};
use policy::PolicyPackage;
use report::{Action, Capture, Finding, Kept, Report, ScanRow, Survivor};
use scan::{Detection, Detections, Honesty, ScanState};

pub const SCHEMA_VERSION: &str = "2.2.0";

/// The smallest image edge the pixel path accepts. The SSIM window is eight
/// pixels and the resize must leave a scorable grid, so an image narrower or
/// shorter than this is refused as unsupported input.
pub const MIN_IMAGE_EDGE: usize = 8;
pub const TOOL_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The codec and resampler crates behind every encode, with their versions,
/// and the in-crate encoder's behavior version. A metadata rewrite re-encodes
/// nothing and reports the same string.
pub fn encoder_fingerprint() -> String {
    let parts: Vec<String> = CODEC_CRATES
        .iter()
        .map(|(name, version)| format!("{name} {version}"))
        .collect();
    format!(
        "unmark {TOOL_VERSION}; container-rewrite/no-reencode; {}; jpeg-encoder in-crate {} baseline 4:4:4; resample in-crate lanczos3 scalar; trig in-crate libm-free",
        parts.join("; "),
        JPEG_ENCODER_VERSION
    )
}

#[cfg(feature = "image")]
const JPEG_ENCODER_VERSION: &str = codec::jpeg::ENCODER_VERSION;
#[cfg(not(feature = "image"))]
const JPEG_ENCODER_VERSION: &str = "v1";

/// The external codec crates and their versions, feature-independent so the
/// fingerprint reads the same in every build.
pub const CODEC_CRATES: &[(&str, &str)] = &[
    ("png", "0.18.1"),
    ("jpeg-decoder", "0.3.2"),
    ("image-webp", "0.2.4"),
    ("claxon", "0.4.3"),
    ("flacenc", "0.5.1"),
];

/// The verbs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Verb {
    Inspect,
    Plan,
    Clean,
    Verify,
}

impl Verb {
    pub fn as_str(self) -> &'static str {
        match self {
            Verb::Inspect => "inspect",
            Verb::Plan => "plan",
            Verb::Clean => "clean",
            Verb::Verify => "verify",
        }
    }
}

/// The opt-outs. Every one turns a strip off.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// Transform ids or mark classes to preserve.
    pub keep: Vec<String>,
    /// Disable every pixel and audio transform; metadata strips still run.
    pub no_degrade: bool,
    /// Strip a certified capture too.
    pub strip_capture: bool,
}

/// Errors mapped to the exit contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnmarkError {
    Usage(String),
    Unsupported(String),
    /// A required inspection or decode failed; presence past the break is
    /// unknown. Maps to exit 30 and writes nothing.
    Inspection(String),
}

impl std::fmt::Display for UnmarkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnmarkError::Usage(m) => write!(f, "usage error: {m}"),
            UnmarkError::Unsupported(m) => write!(f, "unsupported input: {m}"),
            UnmarkError::Inspection(m) => write!(f, "inspection failed: {m}"),
        }
    }
}

impl std::error::Error for UnmarkError {}

/// The output of a clean run: the report, plus the cleaned bytes when and only
/// when the run may write.
#[derive(Clone, Debug)]
pub struct CleanOutcome {
    pub report: Report,
    pub output: Option<Vec<u8>>,
}

/// The policy's container name for a sniffed format.
fn container_name(format: Format) -> &'static str {
    format.as_str()
}

/// The keep names a `--keep` value may use: a transform id, a mark class, or
/// a transform name.
fn keep_matches(pkg: &PolicyPackage, keep: &str, id: &str) -> bool {
    let k = keep.to_ascii_lowercase();
    if k == id.to_ascii_lowercase() {
        return true;
    }
    if let Some(t) = pkg.transform(id) {
        if k == t.name {
            return true;
        }
    }
    transform::targeted_classes(id)
        .iter()
        .any(|c| c.eq_ignore_ascii_case(&k))
}

/// Validate every `--keep` value against the catalog: a value that names no
/// transform, class, or name is a usage error rather than a silent no-op.
fn validate_keeps(pkg: &PolicyPackage, opts: &Options) -> Result<(), UnmarkError> {
    for k in &opts.keep {
        let known = pkg.transforms.iter().any(|t| keep_matches(pkg, k, &t.id))
            || pkg
                .mark_classes
                .iter()
                .any(|m| m.id.eq_ignore_ascii_case(k));
        if !known {
            return Err(UnmarkError::Usage(format!(
                "--keep {k} names no transform, transform name, or mark class"
            )));
        }
    }
    Ok(())
}

/// The plan for a container under the opt-outs: the ordered transform ids
/// that run, and the items kept by flag with the flag named.
pub fn plan_run(pkg: &PolicyPackage, format: Format, opts: &Options) -> (Vec<String>, Vec<Kept>) {
    let mut run = Vec::new();
    let mut kept = Vec::new();
    for id in pkg.default_run(container_name(format)) {
        let t = pkg
            .transform(&id)
            .expect("default run names catalog entries");
        if opts.no_degrade && t.is_degrade() {
            kept.push(Kept {
                item: format!("{} {}", t.id, t.name),
                reason: "kept by flag --no-degrade".to_string(),
            });
            continue;
        }
        if let Some(k) = opts.keep.iter().find(|k| keep_matches(pkg, k, &id)) {
            kept.push(Kept {
                item: format!("{} {}", t.id, t.name),
                reason: format!("kept by flag --keep {k}"),
            });
            continue;
        }
        run.push(id);
    }
    (run, kept)
}

/// Classes the run strips.
fn targeted_for(run: &[String]) -> Vec<&'static str> {
    let mut out = Vec::new();
    for id in run {
        for c in transform::targeted_classes(id) {
            if !out.contains(c) {
                out.push(*c);
            }
        }
    }
    out
}

/// The dwtDct detection over a decoded image, as a detection row.
#[cfg(feature = "image")]
fn dwtdct_detection(img: Option<&codec::Image>) -> (Detection, Vec<mark::dwtdct::Decision>) {
    let label = "dwtDct pixel mark";
    let Some(img) = img else {
        return (
            Detection::with_state("dwtdct", label, Honesty::Confirmable, ScanState::Malformed),
            Vec::new(),
        );
    };
    let decisions = mark::dwtdct::detect(img);
    let mut det = if decisions.iter().any(|d| d.present) {
        Detection::present("dwtdct", label, Honesty::Confirmable)
    } else {
        Detection::with_state(
            "dwtdct",
            label,
            Honesty::Confirmable,
            ScanState::ConfirmedAbsent,
        )
    };
    for d in &decisions {
        det.evidence.push(format!(
            "payload {} agreement {:.3} against threshold {:.2} over {} blocks: {}",
            d.payload,
            d.agreement,
            d.threshold,
            d.blocks,
            if d.present { "present" } else { "absent" }
        ));
    }
    (det, decisions)
}

/// Decode an image for the pixel path and the dwtDct read. None when the
/// build or the container cannot decode it.
#[cfg(feature = "image")]
fn decode_image(bytes: &[u8], format: Format) -> Option<codec::Image> {
    if format.media() != asset::Media::Image {
        return None;
    }
    codec::decode_image(bytes, format).ok()
}

/// The full read of an asset: the container detections plus the dwtDct
/// decision for an image.
fn full_inspect(bytes: &[u8]) -> Detections {
    let mut det = detect::inspect(bytes);
    #[cfg(feature = "image")]
    if det.format.media() == asset::Media::Image {
        let img = decode_image(bytes, det.format);
        let (mut row, _) = dwtdct_detection(img.as_ref());
        // Chroma subsampling halves the U channel the mark lives in, and
        // decoders upsample it differently, so an absent read there is the
        // unsupported state rather than a confirmed absence.
        if det.format == Format::Jpeg
            && detect::jpeg::chroma_subsampled(bytes) == Some(true)
            && row.state == ScanState::ConfirmedAbsent
        {
            row.state = ScanState::UnsupportedFormat;
        }
        det.items.push(row);
    }
    det
}

fn capture_line(reading: &CaptureReading) -> String {
    match reading.status {
        CaptureStatus::Certified => format!(
            "Certified capture kept. Claim: `{}`. Signature status: {SIGNATURE_STATUS}.",
            reading.claim.as_deref().unwrap_or("")
        ),
        CaptureStatus::Generative => format!(
            "Capture claim followed by a generative action: `{}`. Stripped by default. Signature status: {SIGNATURE_STATUS}.",
            reading.claim.as_deref().unwrap_or("")
        ),
        CaptureStatus::Uncertain => format!(
            "Capture uncertain. Hint: `{}`. Stripped by default. Use `--keep exif` to preserve EXIF.",
            reading.hint.as_deref().unwrap_or("")
        ),
        CaptureStatus::None => String::new(),
    }
}

fn capture_report(reading: &CaptureReading, kept: bool) -> Capture {
    let mut c = Capture {
        status: reading.status.as_str().to_string(),
        claim: reading.claim.clone(),
        hint: reading.hint.clone(),
        signature_status: SIGNATURE_STATUS.to_string(),
        kept,
        line: capture_line(reading),
    };
    if reading.status == CaptureStatus::Certified && !kept {
        c.line = format!(
            "Certified capture stripped under --strip-capture. Claim: `{}`. Signature status: {SIGNATURE_STATUS}.",
            reading.claim.as_deref().unwrap_or("")
        );
    }
    c
}

fn rows_and_findings(pkg: &PolicyPackage, det: &Detections) -> (Vec<ScanRow>, Vec<Finding>) {
    let mut rows = Vec::new();
    let mut findings = Vec::new();
    for d in &det.items {
        rows.push(ScanRow {
            class: d.class.clone(),
            label: d.label.clone(),
            honesty: d.honesty,
            state: d.state,
        });
        let note = pkg
            .mark_class(&d.class)
            .map(|m| m.guard.clone())
            .unwrap_or_default();
        let reportable = d.honesty == Honesty::Confirmable
            && matches!(d.state, ScanState::ConfirmedPresent | ScanState::Malformed);
        if reportable {
            findings.push(Finding {
                class: d.class.clone(),
                label: d.label.clone(),
                honesty: d.honesty,
                scan_state: d.state,
                note,
                locations: d.locations.clone(),
                evidence: d.evidence.clone(),
            });
        }
    }
    (rows, findings)
}

/// The survivors: every blind class for the medium, with its cited evidence,
/// and the note of what the run did against it.
fn survivors(pkg: &PolicyPackage, det: &Detections, run: &[String]) -> Vec<Survivor> {
    let degrade: Vec<&str> = run
        .iter()
        .filter(|id| pkg.transform(id).is_some_and(|t| t.is_degrade()))
        .map(String::as_str)
        .collect();
    let transform = if degrade.is_empty() {
        "none".to_string()
    } else {
        degrade.join(", ")
    };
    det.items
        .iter()
        .filter(|d| d.honesty == Honesty::Blind)
        .map(|d| Survivor {
            class: d.class.clone(),
            transform: transform.clone(),
            citation: pkg.mark_class(&d.class).and_then(|m| m.citation.clone()),
        })
        .collect()
}

fn base_report(pkg: &PolicyPackage, verb: Verb, format: Format, capture: Capture) -> Report {
    Report {
        schema_version: SCHEMA_VERSION.to_string(),
        tool_version: TOOL_VERSION.to_string(),
        policy_version: pkg.version.clone(),
        policy_digest: pkg.digest.clone(),
        encoder_fingerprint: encoder_fingerprint(),
        verb: verb.as_str().to_string(),
        format: format.as_str().to_string(),
        output_format: format.as_str().to_string(),
        capture,
        scan_states: Vec::new(),
        findings: Vec::new(),
        actions: Vec::new(),
        stripped_and_proven_gone: Vec::new(),
        kept: Vec::new(),
        survived: Vec::new(),
        sanity: None,
        no_op: false,
        input: None,
        output: None,
        error: None,
        exit_code: report::EXIT_OK,
    }
}

/// A report for a run that did not complete: the verb, the sniffed format,
/// the failure message, and the exit code, so a batch carries one report per
/// input whatever happened to it.
pub fn failure_report(
    pkg: &PolicyPackage,
    verb: Verb,
    bytes: &[u8],
    message: &str,
    exit_code: i32,
) -> Report {
    let none = CaptureReading {
        status: CaptureStatus::None,
        claim: None,
        hint: None,
    };
    let mut r = base_report(pkg, verb, asset::sniff(bytes), capture_report(&none, false));
    r.error = Some(message.to_string());
    r.exit_code = exit_code;
    r
}

fn action_for(pkg: &PolicyPackage, id: &str, outcome: &str, result: &str) -> Action {
    let t = pkg.transform(id);
    Action {
        transform: id.to_string(),
        name: t.map(|t| t.name.clone()).unwrap_or_default(),
        target: t.map(|t| t.target.clone()).unwrap_or_default(),
        strength: t.and_then(|t| t.strength.clone()),
        outcome: outcome.to_string(),
        result: result.to_string(),
        citation: t.and_then(|t| t.citation.clone()),
    }
}

/// The read-only survey: the detections, the capture reading, the plan the
/// default run would take, and the survivors.
pub fn inspect(bytes: &[u8], opts: &Options, pkg: &PolicyPackage) -> Result<Report, UnmarkError> {
    validate_keeps(pkg, opts)?;
    survey(bytes, opts, pkg, Verb::Inspect)
}

/// The dry-run proposal: the ordered transforms clean would apply and what
/// each would do. Touches nothing.
pub fn plan(bytes: &[u8], opts: &Options, pkg: &PolicyPackage) -> Result<Report, UnmarkError> {
    validate_keeps(pkg, opts)?;
    survey(bytes, opts, pkg, Verb::Plan)
}

fn survey(
    bytes: &[u8],
    opts: &Options,
    pkg: &PolicyPackage,
    verb: Verb,
) -> Result<Report, UnmarkError> {
    if bytes.is_empty() {
        return Err(UnmarkError::Unsupported("the input is empty".to_string()));
    }
    let det = full_inspect(bytes);
    let format = det.format;
    let reading = capture::read_capture(bytes, &det, format);
    let capture_kept = reading.status == CaptureStatus::Certified && !opts.strip_capture;
    let (run, kept) = if capture_kept {
        (Vec::new(), Vec::new())
    } else {
        plan_run(pkg, format, opts)
    };
    let mut r = base_report(pkg, verb, format, capture_report(&reading, capture_kept));
    let (rows, findings) = rows_and_findings(pkg, &det);
    r.scan_states = rows;
    r.findings = findings;
    r.kept = kept;
    if capture_kept {
        r.kept.push(Kept {
            item: "the whole asset".to_string(),
            reason: "certified capture; use --strip-capture to override".to_string(),
        });
        for id in pkg.default_run(container_name(format)) {
            r.actions.push(action_for(
                pkg,
                &id,
                "kept: certified capture",
                "not applied",
            ));
        }
    } else {
        for id in &run {
            let classes = transform::targeted_classes(id);
            let present = classes
                .iter()
                .any(|c| det.get(c).map(|d| d.state) == Some(ScanState::ConfirmedPresent));
            let result = if classes.is_empty() {
                pkg.transform(id)
                    .and_then(|t| t.cited_effect.clone())
                    .unwrap_or_else(|| "runs by default".to_string())
            } else if present {
                "a mark of this class is present and will be removed".to_string()
            } else {
                "no mark of this class was present".to_string()
            };
            r.actions.push(action_for(pkg, id, "proposed", &result));
        }
        for k in &r.kept {
            if let Some(id) = k.item.split(' ').next() {
                r.actions
                    .push(action_for(pkg, id, &k.reason, "not applied"));
            }
        }
        #[cfg(feature = "image")]
        if format.media() == asset::Media::Image && run.iter().any(|t| t == "PX01") {
            r.output_format = Format::Jpeg.as_str().to_string();
        }
    }
    r.survived = survivors(pkg, &det, &run);
    // A walker that did not complete leaves a confirmable class malformed,
    // so the inspection failed and the verb says so.
    if det
        .items
        .iter()
        .any(|d| d.honesty == Honesty::Confirmable && d.state == ScanState::Malformed)
    {
        r.exit_code = report::EXIT_INSTRUMENTATION;
    }
    if format == Format::Unknown {
        r.exit_code = report::EXIT_UNSUPPORTED;
    }
    Ok(r)
}

/// The mutating verb: apply the default run under the opt-outs, re-inspect
/// the output to prove the confirmable marks gone, and measure the sanity
/// floor. Writes nothing itself; the caller writes `output` when it is Some.
pub fn clean(
    bytes: &[u8],
    opts: &Options,
    pkg: &PolicyPackage,
) -> Result<CleanOutcome, UnmarkError> {
    validate_keeps(pkg, opts)?;
    if bytes.is_empty() {
        return Err(UnmarkError::Unsupported("the input is empty".to_string()));
    }
    let det = full_inspect(bytes);
    let format = det.format;
    if format == Format::Unknown {
        return Err(UnmarkError::Unsupported(
            "this build does not recognize the container".to_string(),
        ));
    }
    if !format.is_supported_container() {
        return Err(UnmarkError::Unsupported(format!(
            "{} is sniffed but not in the supported set",
            format.as_str()
        )));
    }
    let reading = capture::read_capture(bytes, &det, format);
    let capture_kept = reading.status == CaptureStatus::Certified && !opts.strip_capture;
    let mut r = base_report(
        pkg,
        Verb::Clean,
        format,
        capture_report(&reading, capture_kept),
    );
    let (rows, findings) = rows_and_findings(pkg, &det);
    r.scan_states = rows;
    r.findings = findings;

    // The certified-capture no-op: the input bytes come back unchanged.
    if capture_kept {
        r.kept.push(Kept {
            item: "the whole asset".to_string(),
            reason: "certified capture; use --strip-capture to override".to_string(),
        });
        for id in pkg.default_run(container_name(format)) {
            r.actions.push(action_for(
                pkg,
                &id,
                "kept: certified capture",
                "not applied",
            ));
        }
        r.survived = survivors(pkg, &det, &[]);
        return Ok(CleanOutcome {
            report: r,
            output: Some(bytes.to_vec()),
        });
    }

    let (run, kept) = plan_run(pkg, format, opts);
    r.kept = kept;
    let targeted = targeted_for(&run);

    // A targeted class whose scan state is malformed has unknown presence;
    // the required inspection failed. Nothing is written.
    let unknown: Vec<&str> = targeted
        .iter()
        .copied()
        .filter(|c| det.get(c).map(|d| d.state) == Some(ScanState::Malformed))
        .collect();
    if !unknown.is_empty() {
        return Err(UnmarkError::Inspection(format!(
            "the scan state of {} is malformed, so presence is unknown and nothing is written",
            unknown.join(", ")
        )));
    }

    // Metadata strips.
    let applied = match transform::apply(bytes, format, &run) {
        Ok(a) => a,
        Err(container::RewriteError::Unsupported(m)) => return Err(UnmarkError::Unsupported(m)),
        Err(container::RewriteError::Malformed(m)) => return Err(UnmarkError::Inspection(m)),
        Err(container::RewriteError::Declined { class, reason }) => {
            return Err(UnmarkError::Inspection(format!(
                "declined stripping {class}: {reason}"
            )))
        }
    };
    let mut out_bytes = applied.bytes;
    let mut actions: Vec<Action> = Vec::new();
    let mut run = run;
    // An information frame the next audio frame draws reservoir bits from
    // stays, and the strip that targets it is reported kept with that value.
    let mut info_kept_for_reservoir = false;
    if format == Format::Mp3 && run.iter().any(|t| t == "MC14") {
        if let Some(info) = detect::mp3::layout(&out_bytes).info {
            if !info.droppable() {
                info_kept_for_reservoir = true;
                r.kept.push(Kept {
                    item: "MC14 strip-info-frame".to_string(),
                    reason: format!(
                        "kept: next frame main_data_begin {}",
                        info.next_main_data_begin
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "unread".to_string())
                    ),
                });
                run.retain(|t| t != "MC14");
            }
        }
    }
    #[cfg(not(feature = "audio"))]
    if format == Format::Mp3 && run.iter().any(|t| t == "AU06") {
        actions.push(action_for(
            pkg,
            "AU06",
            "not_attempted",
            "mp3 decode is not in this build",
        ));
    }
    let _ = &info_kept_for_reservoir;
    let mut dwtdct_before: Option<bool> = None;
    let mut dwtdct_after: Option<bool> = None;

    // The pixel path.
    #[cfg(feature = "image")]
    if format.media() == asset::Media::Image {
        let pixel_ids: Vec<String> = run
            .iter()
            .filter(|id| pkg.transform(id).is_some_and(|t| t.kind == "pixel"))
            .cloned()
            .collect();
        if let Some(d) = det.get("dwtdct") {
            dwtdct_before = match d.state {
                ScanState::ConfirmedPresent => Some(true),
                ScanState::ConfirmedAbsent => Some(false),
                _ => None,
            };
        }
        // A kept C2PA chunk on WebP cannot ride through the re-encode, so the
        // pixel path stands down and says why.
        let uncarried = format == Format::WebP
            && !pixel_ids.is_empty()
            && container::webp_metadata(&out_bytes).uncarried_c2pa;
        if uncarried {
            for id in &pixel_ids {
                actions.push(action_for(
                    pkg,
                    id,
                    "not applied",
                    "a kept C2PA chunk cannot be carried through the WebP re-encode, so the pixel path stands down",
                ));
            }
        }
        if !pixel_ids.is_empty() && !uncarried {
            let img = codec::decode_image(&out_bytes, format)
                .map_err(|e| UnmarkError::Inspection(format!("the image would not decode: {e}")))?;
            if img.width < MIN_IMAGE_EDGE || img.height < MIN_IMAGE_EDGE {
                return Err(UnmarkError::Unsupported(format!(
                    "the image is {}x{}, under the {MIN_IMAGE_EDGE}-pixel minimum edge the fidelity metrics need",
                    img.width, img.height
                )));
            }
            let p = transform::pixel::PixelParams::from_policy(pkg, &pixel_ids)
                .map_err(|e| UnmarkError::Inspection(format!("pixel plan: {e}")))?;
            let outcome = transform::pixel::apply(&img, format, &p)
                .map_err(|e| UnmarkError::Inspection(e.to_string()))?;
            // Sanity floor: the encode against its grid-matched reference.
            let (reference, output) = if outcome.reference.channels != outcome.output.channels {
                (outcome.reference.with_alpha(), outcome.output.with_alpha())
            } else {
                (outcome.reference.clone(), outcome.output.clone())
            };
            let psnr = budget::psnr(&reference.data, &output.data);
            let ssim = budget::ssim(
                &reference.data,
                &output.data,
                reference.width,
                reference.height,
                reference.channels,
            );
            // A fidelity check that did not run is a failed measurement, so
            // it can never pass.
            if psnr.is_none() || ssim.is_none() {
                return Err(UnmarkError::Inspection(
                    "the fidelity measurement did not run over the output".to_string(),
                ));
            }
            let passed = psnr.is_some_and(|p| p >= pkg.sanity.psnr_floor_db)
                && ssim.is_some_and(|s| s >= pkg.sanity.ssim_floor);
            r.sanity = Some(report::Sanity {
                psnr_db: psnr,
                ssim,
                psnr_floor_db: pkg.sanity.psnr_floor_db,
                ssim_floor: pkg.sanity.ssim_floor,
                lsd_db: None,
                lsd_ceiling_db: None,
                passed,
                refusal: if passed {
                    None
                } else {
                    Some(format!(
                        "the encode scores PSNR {} dB and SSIM {} against the floors {:.1} dB and {:.2}, which only a broken encode misses",
                        psnr.map(|v| format!("{v:.2}")).unwrap_or_else(|| "n/a".to_string()),
                        ssim.map(|v| format!("{v:.4}")).unwrap_or_else(|| "n/a".to_string()),
                        pkg.sanity.psnr_floor_db,
                        pkg.sanity.ssim_floor
                    ))
                },
            });
            let (row_after, _) = dwtdct_detection(Some(&outcome.output));
            dwtdct_after = Some(row_after.state == ScanState::ConfirmedPresent);
            r.output_format = outcome.emitted.format.as_str().to_string();
            // Whatever metadata survived the strips (the kept items) rides
            // into the fresh container.
            let mut fresh = outcome.emitted.bytes.clone();
            match outcome.emitted.format {
                Format::Png | Format::Jpeg => {
                    fresh = container::carry_ancillary(&out_bytes, &fresh, outcome.emitted.format);
                }
                Format::WebP => {
                    let m = container::webp_metadata(&out_bytes);
                    if m.any_carried() {
                        fresh = codec::webp::encode_lossless_with(
                            &outcome.output,
                            m.exif,
                            m.xmp,
                            m.icc,
                        )
                        .map_err(|e| UnmarkError::Inspection(e.to_string()))?;
                    }
                }
                _ => {}
            }
            for id in &pixel_ids {
                match id.as_str() {
                    "PX03" => {
                        let (cut_w, cut_h) = p
                            .crop
                            .map(|(px, cap)| {
                                (
                                    px.min((img.width as f64 * cap).floor() as usize),
                                    px.min((img.height as f64 * cap).floor() as usize),
                                )
                            })
                            .unwrap_or((0, 0));
                        actions.push(action_for(
                            pkg,
                            id,
                            "applied",
                            &format!(
                                "applied; {cut_w} pixels off each side and {cut_h} off top and bottom, {}x{} kept",
                                img.width.saturating_sub(2 * cut_w).max(1),
                                img.height.saturating_sub(2 * cut_h).max(1)
                            ),
                        ));
                    }
                    "PX02" => {
                        let result = {
                            match (dwtdct_before, dwtdct_after) {
                            (Some(true), Some(false)) => {
                                "removed and proven gone: the dwtDct payload decoded before and fails the presence rule after".to_string()
                            }
                            (Some(true), Some(true)) => {
                                "the dwtDct payload still decodes after the resize".to_string()
                            }
                            _ => "applied; no dwtDct payload decoded before or after".to_string(),
                        }
                        };
                        actions.push(action_for(pkg, id, "applied", &result));
                    }
                    "PX01" => {
                        let pin = outcome
                            .emitted
                            .pin
                            .clone()
                            .unwrap_or_else(|| "lossless write".to_string());
                        actions.push(action_for(
                            pkg,
                            id,
                            "applied",
                            &format!("same-container path, {pin}"),
                        ));
                    }
                    other => actions.push(action_for(pkg, other, "applied", "applied")),
                }
            }
            if !passed {
                r.actions = actions;
                r.actions.extend(metadata_actions(pkg, &run, &det, None));
                r.survived = survivors(pkg, &det, &run);
                r.exit_code = report::EXIT_SANITY;
                return Ok(CleanOutcome {
                    report: r,
                    output: None,
                });
            }
            out_bytes = fresh;
        }
    }

    // The audio path.
    #[cfg(feature = "audio")]
    if matches!(format, Format::RiffWav | Format::Flac) && run.iter().any(|t| t == "AU06") {
        let audio = codec::decode_audio(&out_bytes, format)
            .map_err(|e| UnmarkError::Inspection(format!("the audio would not decode: {e}")))?;
        if audio.frames() == 0 {
            return Err(UnmarkError::Unsupported(
                "the audio carries no samples".to_string(),
            ));
        }
        let p = transform::audio::AudioParams::from_policy(pkg, &run)
            .map_err(|e| UnmarkError::Inspection(format!("audio plan: {e}")))?;
        let processed = transform::audio::apply(&audio, &p);
        let written = codec::encode_audio(&processed, format, audio.bits)
            .map_err(|e| UnmarkError::Inspection(format!("the audio would not encode: {e}")))?;
        // The written stream must decode back to the same length: the only
        // sanity an audio write needs, since the highpass changes the
        // spectrum by design.
        let back = codec::decode_audio(&written, format).map_err(|e| {
            UnmarkError::Inspection(format!("the written audio would not decode: {e}"))
        })?;
        if back.frames() != audio.frames() || back.channels.len() != audio.channels.len() {
            return Err(UnmarkError::Inspection(
                "the written audio changed length or channel count".to_string(),
            ));
        }
        let effect = pkg
            .transform("AU06")
            .and_then(|t| t.cited_effect.clone())
            .unwrap_or_default();
        actions.push(action_for(
            pkg,
            "AU06",
            "applied",
            &format!("applied, survives: {effect}"),
        ));
        // The kept metadata blocks ride into the fresh container.
        out_bytes = container::carry_ancillary(&out_bytes, &written, format);
    }

    // The MP3 audio path: decode, highpass, re-encode at the input bitrate,
    // drop the encoder's own information frame, measure the log-spectral
    // distance of the decoded re-encode against the highpassed samples.
    #[cfg(feature = "audio")]
    let mp3_ancillary_kept = format == Format::Mp3
        && r.kept
            .iter()
            .any(|k| k.item.starts_with("MC15") && k.reason.contains("kept by flag"));
    // A kept ancillary byte range cannot ride through a re-encode, so the
    // audio path stands down and says why, as the pixel path does for a
    // kept C2PA chunk on WebP.
    #[cfg(feature = "audio")]
    if mp3_ancillary_kept && run.iter().any(|t| t == "AU06") {
        for id in ["AU06", "AU03"] {
            actions.push(action_for(
                pkg,
                id,
                "not applied",
                "the kept ancillary bytes cannot be carried through the re-encode, so the audio path stands down",
            ));
        }
    }
    #[cfg(feature = "audio")]
    if format == Format::Mp3 && !mp3_ancillary_kept && run.iter().any(|t| t == "AU06") {
        let (audio, input_kbps) = codec::mp3::decode_with_bitrate(&out_bytes)
            .map_err(|e| UnmarkError::Inspection(format!("the audio would not decode: {e}")))?;
        if audio.frames() == 0 {
            return Err(UnmarkError::Unsupported(
                "the audio carries no samples".to_string(),
            ));
        }
        let p = transform::audio::AudioParams::from_policy(pkg, &run)
            .map_err(|e| UnmarkError::Inspection(format!("audio plan: {e}")))?;
        let processed = transform::audio::apply(&audio, &p);
        let (mut written, used_kbps) = codec::mp3::encode(&processed, input_kbps)
            .map_err(|e| UnmarkError::Inspection(format!("the audio would not encode: {e}")))?;
        // The re-encode replaces every frame, so an information frame kept
        // for its reservoir bits is gone with the rest and the strip of the
        // encoder's own frame goes ahead; only --keep keeps one.
        if info_kept_for_reservoir {
            r.kept.retain(|k| !k.item.starts_with("MC14"));
            run.push("MC14".to_string());
        }
        if run.iter().any(|t| t == "MC14") {
            let spec = container::DropSpec {
                xing: true,
                ..Default::default()
            };
            written = container::mp3::rewrite(&written, &spec).map_err(|e| {
                UnmarkError::Inspection(format!("the re-encode would not rewrite: {e}"))
            })?;
        }
        let (back, _) = codec::mp3::decode_with_bitrate(&written).map_err(|e| {
            UnmarkError::Inspection(format!("the written audio would not decode: {e}"))
        })?;
        // An encoder adds its delay and tail padding, so the length may grow
        // by a few frames and never shrink.
        if back.channels.len() != audio.channels.len()
            || back.frames() < audio.frames()
            || back.frames() > audio.frames() + 4 * 1152
        {
            return Err(UnmarkError::Inspection(format!(
                "the written audio changed from {} to {} frames",
                audio.frames(),
                back.frames()
            )));
        }
        // The decoded re-encode carries the encoder and decoder delay at
        // its head and padding at its tail, so the two signals are aligned
        // by the measured lag and trimmed to the common length first.
        let lag = budget::align_lag(&processed.channels[0], &back.channels[0], 2304, 16384);
        let n = processed.channels[0]
            .len()
            .min(back.channels[0].len().saturating_sub(lag));
        let lsd = budget::lsd(
            &processed.channels[0][..n],
            &back.channels[0][lag..lag + n],
            &budget::LsdParams::PINNED,
        );
        let Some(lsd_db) = lsd else {
            return Err(UnmarkError::Inspection(
                "the fidelity measurement did not run over the output".to_string(),
            ));
        };
        let passed = lsd_db <= pkg.sanity.lsd_ceiling_db;
        r.sanity = Some(report::Sanity {
            psnr_db: None,
            ssim: None,
            psnr_floor_db: pkg.sanity.psnr_floor_db,
            ssim_floor: pkg.sanity.ssim_floor,
            lsd_db: Some(lsd_db),
            lsd_ceiling_db: Some(pkg.sanity.lsd_ceiling_db),
            passed,
            refusal: if passed {
                None
            } else {
                Some(format!(
                    "the re-encode scores LSD {lsd_db:.2} dB against the ceiling {:.1} dB",
                    pkg.sanity.lsd_ceiling_db
                ))
            },
        });
        actions.push(action_for(
            pkg,
            "AU06",
            "applied",
            &format!("applied at 1500 Hz, LSD {lsd_db:.2} dB at lag {lag}"),
        ));
        actions.push(action_for(
            pkg,
            "AU03",
            "applied",
            &format!("same-container path, {used_kbps} kbps CBR, second lossy stage"),
        ));
        // The kept tags ride around the fresh frames.
        written = container::carry_ancillary(&out_bytes, &written, format);
        if !passed {
            r.actions = actions;
            r.actions.extend(metadata_actions(pkg, &run, &det, None));
            r.survived = survivors(pkg, &det, &run);
            r.exit_code = report::EXIT_SANITY;
            return Ok(CleanOutcome {
                report: r,
                output: None,
            });
        }
        out_bytes = written;
    }

    // Re-inspect the output and prove the targeted confirmable marks gone.
    let targeted = targeted_for(&run);
    let det_after = full_inspect(&out_bytes);
    let mut remaining = Vec::new();
    for class in &targeted {
        if *class == "dwtdct" {
            continue;
        }
        let before = det.get(class).map(|d| d.state) == Some(ScanState::ConfirmedPresent);
        if !before {
            continue;
        }
        let label = det
            .get(class)
            .map(|d| d.label.clone())
            .unwrap_or_else(|| class.to_string());
        let after = det_after.get(class).map(|d| d.state) == Some(ScanState::ConfirmedPresent);
        if after {
            remaining.push(label);
        } else {
            r.stripped_and_proven_gone.push(label);
        }
    }
    if dwtdct_before == Some(true) && run.iter().any(|t| t == "PX02") {
        match dwtdct_after {
            Some(false) => r
                .stripped_and_proven_gone
                .push("dwtDct pixel mark".to_string()),
            _ => remaining.push("dwtDct pixel mark".to_string()),
        }
    }

    r.actions = actions;
    r.actions
        .extend(metadata_actions(pkg, &run, &det, Some(&det_after)));
    for k in &r.kept {
        if let Some(id) = k.item.split(' ').next() {
            r.actions
                .push(action_for(pkg, id, &k.reason, "not applied"));
        }
    }
    // The output's scan states are the file the user now has.
    let (rows, findings) = rows_and_findings(pkg, &det_after);
    r.scan_states = rows;
    r.findings = findings;
    r.survived = survivors(pkg, &det, &run);
    if !remaining.is_empty() {
        r.exit_code = report::EXIT_MARK_REMAINS;
        r.stripped_and_proven_gone.clear();
        return Ok(CleanOutcome {
            report: r,
            output: None,
        });
    }
    if out_bytes == bytes {
        r.no_op = true;
        for a in r.actions.iter_mut() {
            if a.outcome == "applied" {
                a.outcome = "no-op".to_string();
                a.result = "the output equals the input, nothing changed".to_string();
            }
        }
    }
    Ok(CleanOutcome {
        report: r,
        output: Some(out_bytes),
    })
}

/// Actions for the metadata and text transforms in the run, with each class
/// judged present before and gone after.
fn metadata_actions(
    pkg: &PolicyPackage,
    run: &[String],
    before: &Detections,
    after: Option<&Detections>,
) -> Vec<Action> {
    let mut out = Vec::new();
    for id in run {
        let Some(t) = pkg.transform(id) else { continue };
        if t.is_degrade() {
            continue;
        }
        let classes = transform::targeted_classes(id);
        let present: Vec<&str> = classes
            .iter()
            .copied()
            .filter(|c| before.get(c).map(|d| d.state) == Some(ScanState::ConfirmedPresent))
            .collect();
        let result = if classes.is_empty() {
            "applied".to_string()
        } else if present.is_empty() {
            "no mark of this class was present".to_string()
        } else {
            match after {
                Some(a) => {
                    let gone = present
                        .iter()
                        .all(|c| a.get(c).map(|d| d.state) != Some(ScanState::ConfirmedPresent));
                    if gone {
                        "removed and proven gone".to_string()
                    } else {
                        "a mark of this class remains".to_string()
                    }
                }
                None => "not written".to_string(),
            }
        };
        out.push(action_for(pkg, id, "applied", &result));
    }
    out
}

/// The result of a verify run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerifyOutcome {
    Verified,
    Mismatch(Vec<String>),
}

/// Re-inspect an output against the report that produced it. It confirms the
/// policy digest matches and that every item the report stripped and proved
/// gone is now absent.
pub fn verify(output: &[u8], report_json: &str, pkg: &PolicyPackage) -> VerifyOutcome {
    let parsed: serde_json::Value = match serde_json::from_str(report_json) {
        Ok(v) => v,
        Err(e) => return VerifyOutcome::Mismatch(vec![format!("report parse: {e}")]),
    };
    let mut problems = Vec::new();
    if parsed.get("policy_digest").and_then(|v| v.as_str()) != Some(pkg.digest.as_str()) {
        problems.push("policy digest does not match this build".to_string());
    }
    let det = full_inspect(output);
    if det.format == Format::Unknown || !det.format.is_supported_container() {
        problems.push(format!(
            "the output is {}, which this build does not verify",
            det.format.as_str()
        ));
    }
    if let Some(emitted) = parsed.get("output_format").and_then(|v| v.as_str()) {
        if emitted != det.format.as_str() {
            problems.push(format!(
                "the output is {} and the report emitted {emitted}",
                det.format.as_str()
            ));
        }
    }
    for d in &det.items {
        if d.state == ScanState::Malformed {
            problems.push(format!("{} is malformed in the output", d.label));
        }
    }
    if let Some(list) = parsed
        .get("stripped_and_proven_gone")
        .and_then(|v| v.as_array())
    {
        for item in list {
            let Some(label) = item.as_str() else { continue };
            if let Some(d) = det.items.iter().find(|d| d.label == label) {
                if d.state == ScanState::ConfirmedPresent {
                    problems.push(format!("{label} is present again in the output"));
                }
            }
        }
    }
    if problems.is_empty() {
        VerifyOutcome::Verified
    } else {
        VerifyOutcome::Mismatch(problems)
    }
}
