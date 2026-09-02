//! unmark removes the confirmable marks a generative tool left on an asset the
//! user made, and reports honestly about the marks it cannot confirm.
//!
//! The pure core is small and named. `detect::inspect(bytes)` is a pure
//! function of the input. `plan` resolves detections and a profile into an
//! ordered transform list. `transform::apply(bytes, format, plan)` rewrites the
//! container without decoding a pixel or a sample. The binary does the I/O.
//!
//! The tool never emits a clean verdict, and it never renders an empty
//! detection list as human authorship. A finding of no marks is a statement
//! about this build's reach over an enumerated set of containers, never about
//! the asset's origin. Missing credentials never prove an asset was
//! uncredentialed, because copies and registry records persist.

pub mod asset;
pub mod budget;
#[cfg(feature = "image")]
pub mod calibrate;
pub mod codec;
pub mod container;
pub mod detect;
pub mod dsp;
pub mod guard;
pub mod policy;
pub mod report;
pub mod scan;
pub mod skill;
pub mod transform;

use asset::Format;
use policy::PolicyPackage;
use report::{Action, Finding, Report, ScanRow, Tier};
use scan::{Detections, Honesty, ScanState};

pub const SCHEMA_VERSION: &str = "1.0.0";
pub const TOOL_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The codec and resampler crates behind every encode, with their versions.
/// A metadata rewrite re-encodes nothing and reports the same string, so a
/// report and the calibration record name one fingerprint. The versions are
/// pinned here and checked against Cargo.lock by the test suite, so a codec
/// bump changes the fingerprint and fails CI until recalibration.
pub fn encoder_fingerprint() -> String {
    let parts: Vec<String> = CODEC_CRATES
        .iter()
        .map(|(name, version)| format!("{name} {version}"))
        .collect();
    format!(
        "unmark {TOOL_VERSION}; container-rewrite/no-reencode; {}; jpeg-encoder in-crate baseline 4:4:4; resample in-crate lanczos3 and kaiser-sinc scalar",
        parts.join("; ")
    )
}

/// The external codec crates and the versions this build was calibrated
/// against. Feature-independent on purpose: the fingerprint must read the
/// same in a default build and an all-features build.
pub const CODEC_CRATES: &[(&str, &str)] = &[
    ("png", "0.18.1"),
    ("jpeg-decoder", "0.3.2"),
    ("image-webp", "0.2.4"),
    ("claxon", "0.4.3"),
    ("flacenc", "0.5.1"),
];

/// The mutating and read-only verbs.
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

/// Per-run options set from CLI flags.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// G1: the ownership assertion, required on clean.
    pub i_generated_this: bool,
    /// The residual acknowledgment that lets a clean run reach exit 0.
    pub acknowledge_residual: bool,
    /// G2 override: strip a manifest carrying a capture or publisher signal.
    pub force_provenance_strip: bool,
    /// Opt-in transforms drawn from the profile's optional set, for example
    /// MC06.
    pub opt_in: Vec<String>,
}

/// Errors mapped to the exit contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnmarkError {
    Usage(String),
    Unsupported(String),
    /// The container's structure would not parse safely. Presence of every
    /// class past the break is unknown. Maps to exit 40 and writes nothing.
    Malformed(String),
    Instrumentation(String),
}

impl std::fmt::Display for UnmarkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnmarkError::Usage(m) => write!(f, "usage error: {m}"),
            UnmarkError::Unsupported(m) => write!(f, "unsupported input: {m}"),
            UnmarkError::Malformed(m) => write!(f, "malformed: {m}"),
            UnmarkError::Instrumentation(m) => write!(f, "instrumentation error: {m}"),
        }
    }
}

impl std::error::Error for UnmarkError {}

/// The output of a clean run: the report, plus the cleaned bytes when and only
/// when the run reached exit 0.
#[derive(Clone, Debug)]
pub struct CleanOutcome {
    pub report: Report,
    pub output: Option<Vec<u8>>,
}

/// Resolve a profile name against the package, enforcing v1 availability and
/// the feature gate.
fn resolve_profile<'a>(
    pkg: &'a PolicyPackage,
    name: &str,
) -> Result<&'a policy::Profile, UnmarkError> {
    let p = pkg
        .profile(name)
        .ok_or_else(|| UnmarkError::Usage(format!("unknown profile {name}")))?;
    if p.milestone != 1 {
        return Err(UnmarkError::Usage(format!(
            "profile {name} is not available in this build. The pixel and audio degrade transforms it needs ship in 0.2.0"
        )));
    }
    if p.feature.as_deref() == Some("audio") && !cfg!(feature = "audio") {
        return Err(UnmarkError::Usage(format!(
            "profile {name} needs the audio feature; install with --features audio"
        )));
    }
    Ok(p)
}

/// The ordered transform ids a profile applies, including any opted-in optional
/// transforms, in canonical order.
fn resolved_transforms(
    pkg: &PolicyPackage,
    profile: &policy::Profile,
    opts: &Options,
) -> Vec<String> {
    let mut ids: Vec<String> = profile.transforms.clone();
    for o in &opts.opt_in {
        if profile.optional_transforms.contains(o) && !ids.contains(o) {
            ids.push(o.clone());
        }
    }
    // Canonical order by the transform catalog order field.
    ids.sort_by_key(|id| pkg.transform(id).map(|t| t.order).unwrap_or(i64::MAX));
    ids
}

/// The confirmable mark classes a transform set strips.
fn targeted_classes(transforms: &[String]) -> Vec<&'static str> {
    let mut out = Vec::new();
    for t in transforms {
        match t.as_str() {
            "MC01" => out.push("c2pa"),
            "MC02" => {
                out.push("xmp");
                out.push("iptc");
            }
            "MC03" => out.push("exif"),
            "MC04" => out.push("png_text"),
            "MC05" => {
                out.push("id3");
                out.push("ilst");
            }
            "MC06" => out.push("vorbis"),
            "MC07" => out.push("invisibles"),
            "MC09" => {
                out.push("riff_ancillary");
                out.push("id3");
            }
            _ => {}
        }
    }
    out
}

/// The planned transform that targets a class. A refusal or a decline is
/// attributed through this lookup rather than to a hardcoded id, so a new
/// class in a later milestone reports against its own catalog entry.
fn transform_for_class<'a>(
    pkg: &'a PolicyPackage,
    transforms: &[String],
    class: &str,
) -> Option<&'a policy::Transform> {
    transforms
        .iter()
        .find(|id| targeted_classes(std::slice::from_ref(id)).contains(&class))
        .and_then(|id| pkg.transform(id))
}

/// Evidence that the asset is generative, read from the asset itself. This is
/// what keys the residual tier on `inspect`, which makes no ownership demand.
fn generative_evidence(bytes: &[u8], det: &Detections) -> Vec<String> {
    let mut ev = Vec::new();
    let gens: Vec<String> = include_str!("../policy/signatures/generator-strings.txt")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_ascii_lowercase)
        .collect();
    if let Some(pt) = det.get("png_text") {
        if pt.state == ScanState::ConfirmedPresent {
            ev.push("a PNG text chunk carries generation data".to_string());
        }
    }
    if let Some(ex) = det.get("exif") {
        for e in &ex.evidence {
            let low = e.to_ascii_lowercase();
            if gens.iter().any(|g| low.contains(g)) {
                ev.push(format!("EXIF names a generative tool ({e})"));
            }
        }
    }
    if let Some(x) = det.get("xmp") {
        for e in &x.evidence {
            let low = e.to_ascii_lowercase();
            if gens.iter().any(|g| low.contains(g)) {
                ev.push(format!("XMP CreatorTool names a generative tool ({e})"));
            }
        }
    }
    if let Some(c) = det.get("c2pa") {
        if c.state == ScanState::ConfirmedPresent {
            for loc in &c.locations {
                let region = bytes
                    .get(loc.offset..loc.offset + loc.length)
                    .unwrap_or(&[]);
                let low: Vec<u8> = region.iter().map(u8::to_ascii_lowercase).collect();
                if gens
                    .iter()
                    .any(|g| detect::c2pa::contains(&low, g.as_bytes()))
                {
                    ev.push("a C2PA manifest names a generative tool".to_string());
                    break;
                }
            }
        }
    }
    ev
}

/// Build the scan-state rows and the tiered findings shared by every verb.
fn build_findings(
    pkg: &PolicyPackage,
    det: &Detections,
    targeted: &[&str],
    force_residual: bool,
    generative: bool,
) -> (Vec<ScanRow>, Vec<Finding>) {
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
        match d.honesty {
            Honesty::Confirmable => match d.state {
                ScanState::ConfirmedPresent => {
                    let tier = if targeted.contains(&d.class.as_str()) {
                        Tier::Removable
                    } else {
                        Tier::Note
                    };
                    findings.push(finding(tier, d, note));
                }
                ScanState::Malformed => {
                    findings.push(finding(Tier::Note, d, note));
                }
                _ => {}
            },
            Honesty::Blind => {
                // The residual tier fires when the run carries positive evidence
                // of generative origin, which is the ownership assertion on
                // clean or asset-intrinsic evidence on inspect.
                let tier = if force_residual || generative {
                    Tier::Residual
                } else {
                    Tier::Note
                };
                findings.push(finding(tier, d, note));
            }
            Honesty::Unaddressed => {
                findings.push(finding(Tier::Note, d, note));
            }
        }
    }
    (rows, findings)
}

fn finding(tier: Tier, d: &scan::Detection, note: String) -> Finding {
    Finding {
        tier,
        class: d.class.clone(),
        label: d.label.clone(),
        honesty: d.honesty,
        scan_state: d.state,
        note,
        locations: d.locations.clone(),
        evidence: d.evidence.clone(),
    }
}

/// The three summary parts. `removed` is the set of classes proven gone.
fn three_parts(det: &Detections, removed: &[String]) -> (Vec<String>, Vec<String>, Vec<String>) {
    let removed_and_proven = removed.to_vec();
    // No degrade transforms ship in this milestone, so nothing is degraded
    // without proof yet.
    let degraded_without_proof: Vec<String> = Vec::new();
    let mut not_addressed = Vec::new();
    for d in &det.items {
        match d.honesty {
            Honesty::Blind => not_addressed.push(format!(
                "{} (keyed mark, not visible to this offline build)",
                d.label
            )),
            Honesty::Unaddressed if d.state == ScanState::NotAttempted => {
                not_addressed.push(d.label.clone())
            }
            _ => {}
        }
    }
    (removed_and_proven, degraded_without_proof, not_addressed)
}

fn base_report(pkg: &PolicyPackage, verb: Verb, profile: &str, format: Format) -> Report {
    Report {
        schema_version: SCHEMA_VERSION.to_string(),
        tool_version: TOOL_VERSION.to_string(),
        policy_version: pkg.version.clone(),
        policy_digest: pkg.digest.clone(),
        encoder_fingerprint: encoder_fingerprint(),
        verb: verb.as_str().to_string(),
        profile: profile.to_string(),
        format: format.as_str().to_string(),
        scan_states: Vec::new(),
        findings: Vec::new(),
        actions: Vec::new(),
        removed_and_proven: Vec::new(),
        degraded_without_proof: Vec::new(),
        not_addressed: Vec::new(),
        residual_acknowledgment_required: false,
        residual_acknowledged: false,
        camera_origin_warning: None,
        exit_code: report::EXIT_OK,
    }
}

/// The read-only survey. It keys the residual tier only on asset-intrinsic
/// evidence and makes no ownership demand.
pub fn inspect(
    bytes: &[u8],
    profile_name: &str,
    pkg: &PolicyPackage,
) -> Result<Report, UnmarkError> {
    let profile = resolve_profile(pkg, profile_name)?;
    let det = detect::inspect(bytes);
    let transforms = resolved_transforms(pkg, profile, &Options::default());
    let targeted = targeted_classes(&transforms);
    let generative = !generative_evidence(bytes, &det).is_empty();
    let (rows, findings) = build_findings(pkg, &det, &targeted, false, generative);
    let (rp, dg, na) = three_parts(&det, &[]);
    let mut r = base_report(pkg, Verb::Inspect, profile_name, det.format);
    r.camera_origin_warning = guard::scan_g3(bytes, det.format);
    r.scan_states = rows;
    r.findings = findings;
    r.removed_and_proven = rp;
    r.degraded_without_proof = dg;
    r.not_addressed = na;
    Ok(r)
}

/// The dry-run proposal. It shows the ordered transforms clean would apply and
/// touches nothing.
pub fn plan(
    bytes: &[u8],
    profile_name: &str,
    opts: &Options,
    pkg: &PolicyPackage,
) -> Result<Report, UnmarkError> {
    let profile = resolve_profile(pkg, profile_name)?;
    let det = detect::inspect(bytes);
    let transforms = resolved_transforms(pkg, profile, opts);
    let targeted = targeted_classes(&transforms);
    let generative = !generative_evidence(bytes, &det).is_empty();
    let (rows, findings) = build_findings(pkg, &det, &targeted, false, generative);
    let (rp, dg, na) = three_parts(&det, &[]);
    let mut r = base_report(pkg, Verb::Plan, profile_name, det.format);
    r.camera_origin_warning = guard::scan_g3(bytes, det.format);
    r.scan_states = rows;
    r.findings = findings;
    r.removed_and_proven = rp;
    r.degraded_without_proof = dg;
    r.not_addressed = na;
    // The proposed actions, none applied.
    for id in &transforms {
        if let Some(t) = pkg.transform(id) {
            r.actions.push(Action {
                transform: t.id.clone(),
                name: t.name.clone(),
                target: t.target.clone(),
                outcome: "proposed".to_string(),
            });
        }
    }
    Ok(r)
}

/// The mutating verb. It applies the plan, re-inspects the output to prove the
/// confirmable marks are gone, checks the fidelity budget, and requires the
/// residual acknowledgment before it reports success.
pub fn clean(
    bytes: &[u8],
    profile_name: &str,
    opts: &Options,
    pkg: &PolicyPackage,
) -> Result<CleanOutcome, UnmarkError> {
    let profile = resolve_profile(pkg, profile_name)?;
    if !opts.i_generated_this {
        return Err(UnmarkError::Usage(
            "clean requires --i-generated-this. unmark cleans an asset the user made, not one taken from someone else".to_string(),
        ));
    }
    let det = detect::inspect(bytes);
    if det.format == Format::Unknown {
        return Err(UnmarkError::Unsupported(
            "this build does not recognize the container".to_string(),
        ));
    }

    let transforms = resolved_transforms(pkg, profile, opts);
    let targeted = targeted_classes(&transforms);
    let mut r = base_report(pkg, Verb::Clean, profile_name, det.format);
    r.camera_origin_warning = guard::scan_g3(bytes, det.format);

    // A targeted class whose scan state is malformed has unknown presence, and
    // a clean can never report success over it. Fail closed before any rewrite
    // and write nothing. This holds across every container, including the ones
    // whose rewriter copies bytes verbatim and would otherwise "succeed".
    let mut unknown: Vec<String> = targeted
        .iter()
        .filter(|c| det.get(c).map(|d| d.state) == Some(ScanState::Malformed))
        .map(|c| c.to_string())
        .collect();
    unknown.sort();
    unknown.dedup();
    if !unknown.is_empty() {
        let (rows, findings) = build_findings(pkg, &det, &targeted, true, true);
        r.scan_states = rows;
        r.findings = findings;
        let (rp, dg, na) = three_parts(&det, &[]);
        r.removed_and_proven = rp;
        r.degraded_without_proof = dg;
        r.not_addressed = na;
        r.residual_acknowledgment_required = true;
        r.exit_code = report::EXIT_UNSUPPORTED;
        for class in &unknown {
            let label = det
                .get(class)
                .map(|d| d.label.clone())
                .unwrap_or_else(|| class.clone());
            let outcome = format!(
                "refused: {label} scan state is malformed and presence is unknown. Nothing written"
            );
            let action = match transform_for_class(pkg, &transforms, class) {
                Some(t) => Action {
                    transform: t.id.clone(),
                    name: t.name.clone(),
                    target: t.target.clone(),
                    outcome,
                },
                None => Action {
                    transform: String::new(),
                    name: format!("strip {class}"),
                    target: label,
                    outcome,
                },
            };
            r.actions.push(action);
        }
        return Ok(CleanOutcome {
            report: r,
            output: None,
        });
    }

    // G2: the third-party-provenance refusal. It governs every C2PA removal.
    let strips_c2pa = transforms.iter().any(|t| t == "MC01");
    if strips_c2pa && !opts.force_provenance_strip {
        if let Some(sig) = guard::scan_g2(bytes, &det) {
            let (rows, findings) = build_findings(pkg, &det, &targeted, true, true);
            r.scan_states = rows;
            r.findings = findings;
            let (rp, dg, na) = three_parts(&det, &[]);
            r.removed_and_proven = rp;
            r.degraded_without_proof = dg;
            r.not_addressed = na;
            r.residual_acknowledgment_required = true;
            r.exit_code = report::EXIT_UNSUPPORTED;
            r.actions.push(Action {
                transform: "MC01".to_string(),
                name: "strip-c2pa-manifest".to_string(),
                target: "C2PA manifest".to_string(),
                outcome: format!(
                    "refused: {} signal present ({}); pass --force-provenance-strip only for your own asset",
                    sig.kind, sig.detail
                ),
            });
            return Ok(CleanOutcome {
                report: r,
                output: None,
            });
        }
    }

    // Apply the plan.
    let applied = match transform::apply(bytes, det.format, &transforms) {
        Ok(a) => a,
        Err(container::RewriteError::Unsupported(m)) => return Err(UnmarkError::Unsupported(m)),
        // A malformed container is bad input, not a tooling fault. Fail closed at
        // exit 40 and write nothing rather than rewriting a truncated prefix.
        Err(container::RewriteError::Malformed(m)) => return Err(UnmarkError::Malformed(m)),
        Err(container::RewriteError::Declined { class, reason }) => {
            // A declined removable strip fails the run closed, attributed to
            // whichever planned transform targets the declined class.
            let (rows, findings) = build_findings(pkg, &det, &targeted, true, true);
            r.scan_states = rows;
            r.findings = findings;
            let (rp, dg, na) = three_parts(&det, &[]);
            r.removed_and_proven = rp;
            r.degraded_without_proof = dg;
            r.not_addressed = na;
            r.residual_acknowledgment_required = true;
            r.exit_code = report::EXIT_UNSUPPORTED;
            let action = match transform_for_class(pkg, &transforms, &class) {
                Some(t) => Action {
                    transform: t.id.clone(),
                    name: t.name.clone(),
                    target: t.target.clone(),
                    outcome: format!("declined stripping {class}: {reason}"),
                },
                None => Action {
                    transform: String::new(),
                    name: format!("strip {class}"),
                    target: class.clone(),
                    outcome: format!("declined: {reason}"),
                },
            };
            r.actions.push(action);
            return Ok(CleanOutcome {
                report: r,
                output: None,
            });
        }
    };

    // Re-inspect the output and prove the targeted confirmable marks are gone.
    let det_after = detect::inspect(&applied.bytes);
    let mut removed = Vec::new();
    let mut still_present = Vec::new();
    for class in &targeted {
        let before = det.get(class).map(|d| d.state) == Some(ScanState::ConfirmedPresent);
        if !before {
            continue;
        }
        let after = det_after.get(class).map(|d| d.state) == Some(ScanState::ConfirmedPresent);
        let label = det
            .get(class)
            .map(|d| d.label.clone())
            .unwrap_or_else(|| class.to_string());
        if after {
            still_present.push(label);
        } else {
            removed.push(label);
        }
    }

    // Record the actions taken.
    for id in &transforms {
        if let Some(t) = pkg.transform(id) {
            r.actions.push(Action {
                transform: t.id.clone(),
                name: t.name.clone(),
                target: t.target.clone(),
                outcome: "applied".to_string(),
            });
        }
    }

    let (rows, findings) = build_findings(pkg, &det_after, &targeted, true, true);
    r.scan_states = rows;
    r.findings = findings;
    let (rp, dg, na) = three_parts(&det_after, &removed);
    r.removed_and_proven = rp;
    r.degraded_without_proof = dg;
    r.not_addressed = na;
    r.residual_acknowledgment_required = true;
    r.residual_acknowledged = opts.acknowledge_residual;

    // The metadata-tier byte-identity budget: the encoded signal stream must be
    // unchanged. A deviation is a bug and fails closed.
    let mut exit = report::EXIT_OK;
    if profile.budget == "exact" {
        let before_sig = container::signal_stream(bytes, det.format);
        let after_sig = container::signal_stream(&applied.bytes, det_after.format);
        if budget::check_byte_identity(&before_sig, &after_sig).is_err() {
            exit = worst(exit, report::EXIT_INSTRUMENTATION);
        }
    }
    if !still_present.is_empty() {
        exit = worst(exit, report::EXIT_MARK_REMAINS);
    }
    if r.residual_acknowledgment_required && !r.residual_acknowledged {
        exit = worst(exit, report::EXIT_RESIDUAL);
    }
    r.exit_code = exit;

    let output = if exit == report::EXIT_OK {
        Some(applied.bytes)
    } else {
        None
    };
    // Nothing written means nothing was proven removed in a delivered output.
    // The in-memory re-inspection stays in the scan states, but the removed
    // list describes the file the user now has, and there is none.
    if output.is_none() {
        r.removed_and_proven.clear();
    }
    Ok(CleanOutcome { report: r, output })
}

/// Combine two exit codes, keeping the more serious. Fail-closed states above
/// the reporting states.
fn worst(a: i32, b: i32) -> i32 {
    fn rank(c: i32) -> i32 {
        match c {
            report::EXIT_UNSUPPORTED => 5,
            report::EXIT_INSTRUMENTATION => 4,
            report::EXIT_MARK_REMAINS => 3,
            report::EXIT_RESIDUAL => 2,
            _ => 0,
        }
    }
    if rank(b) > rank(a) {
        b
    } else {
        a
    }
}

/// The result of a verify run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerifyOutcome {
    Verified,
    Mismatch(Vec<String>),
}

/// Re-inspect an output against the report that produced it. It confirms the
/// policy digest matches and that every class the report proved gone is now
/// absent.
pub fn verify(output: &[u8], report_json: &str, pkg: &PolicyPackage) -> VerifyOutcome {
    let parsed: serde_json::Value = match serde_json::from_str(report_json) {
        Ok(v) => v,
        Err(e) => return VerifyOutcome::Mismatch(vec![format!("report parse: {e}")]),
    };
    let mut problems = Vec::new();
    if parsed.get("policy_digest").and_then(|v| v.as_str()) != Some(pkg.digest.as_str()) {
        problems.push("policy digest does not match this build".to_string());
    }
    let det = detect::inspect(output);
    if let Some(list) = parsed.get("removed_and_proven").and_then(|v| v.as_array()) {
        for item in list {
            let Some(label) = item.as_str() else { continue };
            // Match the label back to a class and confirm it is absent.
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
