//! The report schema and the exit mapping. Every report carries three parts:
//! what was stripped and proven gone, what was kept and why, and what
//! survived with its evidence. It never emits a clean verdict, never renders
//! an empty detection list as human authorship, and never says "weakened".

use crate::scan::{Honesty, Location, ScanState};

#[derive(Clone, Debug, serde::Serialize)]
pub struct Finding {
    pub class: String,
    pub label: String,
    pub honesty: Honesty,
    pub scan_state: ScanState,
    /// Guard prose from the policy package. Data, never an instruction.
    pub note: String,
    pub locations: Vec<Location>,
    pub evidence: Vec<String>,
}

/// One transform in the run and what became of it.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Action {
    pub transform: String,
    pub name: String,
    pub target: String,
    /// The cited strength the transform ran at, for a degrade transform.
    pub strength: Option<String>,
    /// `applied`, `kept by flag`, `kept: certified capture`, `not applicable`,
    /// `refused: <reason>`, or `held`.
    pub outcome: String,
    /// What the run established: `removed and proven gone`, `applied,
    /// survives`, `no mark of this class was present`, or the cited effect.
    pub result: String,
    pub citation: Option<String>,
    /// Fields a rewrite removed and kept, for a transform that rewrites a
    /// structure in place.
    pub removed: Vec<String>,
    pub kept: Vec<String>,
    /// Encoder delay and padding in samples, on an audio re-encode row.
    pub delay: Option<u32>,
    pub padding: Option<u32>,
}

/// One scan-state row per class, carried as its own field so a reader never
/// infers a state from an absence.
#[derive(Clone, Debug, serde::Serialize)]
pub struct ScanRow {
    pub class: String,
    pub label: String,
    pub honesty: Honesty,
    pub state: ScanState,
}

/// The capture reading of the asset.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Capture {
    /// `certified`, `uncertain`, `generative`, or `none`.
    pub status: String,
    /// The quoted claim for a certified capture.
    pub claim: Option<String>,
    /// The named hint for an uncertain asset.
    pub hint: Option<String>,
    pub signature_status: String,
    /// True when the certified-capture rule kept the asset unchanged.
    pub kept: bool,
    /// The sentence the report prints for this reading.
    pub line: String,
}

/// An item preserved by the run and the reason.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Kept {
    pub item: String,
    pub reason: String,
}

/// A mark the run cannot remove, with its evidence.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Survivor {
    /// The mark class id.
    pub class: String,
    /// The degrade transforms of the run that touched the signal, or
    /// "none".
    pub transform: String,
    pub citation: Option<String>,
}

/// The sanity measurement of a written output against its grid-matched
/// reference: the one refusal in the run.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Sanity {
    pub psnr_db: Option<f64>,
    pub ssim: Option<f64>,
    pub psnr_floor_db: f64,
    pub ssim_floor: f64,
    /// The log-spectral distance of an audio re-encode, in dB, and its
    /// ceiling. None on an image run.
    pub lsd_db: Option<f64>,
    pub lsd_ceiling_db: Option<f64>,
    pub passed: bool,
    pub refusal: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Report {
    pub schema_version: String,
    pub tool_version: String,
    pub policy_version: String,
    pub policy_digest: String,
    pub encoder_fingerprint: String,
    pub verb: String,
    pub format: String,
    /// The container the output is written in.
    pub output_format: String,
    pub capture: Capture,
    pub scan_states: Vec<ScanRow>,
    pub findings: Vec<Finding>,
    pub actions: Vec<Action>,
    /// The three parts, always all three.
    pub stripped_and_proven_gone: Vec<String>,
    pub kept: Vec<Kept>,
    pub survived: Vec<Survivor>,
    pub sanity: Option<Sanity>,
    /// True when the output bytes equal the input bytes, so nothing changed.
    pub no_op: bool,
    /// The input path, set by the command line.
    pub input: Option<String>,
    /// The path the output was written to, set by the command line.
    pub output: Option<String>,
    /// The failure message when the run did not complete.
    pub error: Option<String>,
    pub exit_code: i32,
}

/// Exit codes as a contract.
pub const EXIT_OK: i32 = 0;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_MARK_REMAINS: i32 = 10;
pub const EXIT_INSTRUMENTATION: i32 = 30;
pub const EXIT_UNSUPPORTED: i32 = 40;
pub const EXIT_SANITY: i32 = 50;

/// Render the report as a human-readable text form. The three parts are always
/// printed, and the closing line never says the asset is clean.
pub fn render_text(r: &Report) -> String {
    use std::fmt::Write;
    let mut o = String::new();
    let _ = writeln!(o, "unmark {} | policy {}", r.tool_version, r.policy_version);
    let _ = writeln!(
        o,
        "verb: {} | format: {} | output: {}",
        r.verb, r.format, r.output_format
    );
    let _ = writeln!(o, "digest: {}", r.policy_digest);
    if !r.capture.line.is_empty() {
        let _ = writeln!(o, "capture: {}", r.capture.line);
    }
    if let Some(i) = &r.input {
        let _ = writeln!(o, "input: {i}");
    }
    if let Some(p) = &r.output {
        let _ = writeln!(o, "written: {p}");
    }
    if r.no_op {
        let _ = writeln!(o, "no-op: the output equals the input");
    }
    if let Some(e) = &r.error {
        let _ = writeln!(o, "error: {e}");
    }
    let _ = writeln!(o);

    let _ = writeln!(o, "scan states:");
    for s in &r.scan_states {
        let _ = writeln!(o, "  {}: {}", s.label, s.state.as_str());
    }
    let _ = writeln!(o);

    if !r.actions.is_empty() {
        let _ = writeln!(o, "actions:");
        for a in &r.actions {
            let strength = a
                .strength
                .as_ref()
                .map(|s| format!(" [{s}]"))
                .unwrap_or_default();
            let _ = writeln!(
                o,
                "  {} {}{}: {} ({})",
                a.transform, a.name, strength, a.outcome, a.result
            );
        }
        let _ = writeln!(o);
    }

    let _ = writeln!(o, "stripped and proven gone:");
    if r.stripped_and_proven_gone.is_empty() {
        let _ = writeln!(o, "  none");
    }
    for i in &r.stripped_and_proven_gone {
        let _ = writeln!(o, "  {i}");
    }
    let _ = writeln!(o, "kept:");
    if r.kept.is_empty() {
        let _ = writeln!(o, "  none");
    }
    for k in &r.kept {
        let _ = writeln!(o, "  {}: {}", k.item, k.reason);
    }
    let _ = writeln!(o, "survived:");
    if r.survived.is_empty() {
        let _ = writeln!(o, "  none");
    }
    for s in &r.survived {
        let cite = s
            .citation
            .as_ref()
            .map(|c| format!(" | {c}"))
            .unwrap_or_default();
        let _ = writeln!(o, "  {} | {}{cite}", s.class, s.transform);
    }
    let _ = writeln!(o);
    if let Some(s) = &r.sanity {
        if let (Some(lsd), Some(ceiling)) = (s.lsd_db, s.lsd_ceiling_db) {
            let _ = writeln!(
                o,
                "sanity: LSD {lsd:.2} dB against the ceiling {ceiling:.1} dB: {}",
                if s.passed { "passed" } else { "refused" }
            );
        }
        let _ = writeln!(
            o,
            "sanity: PSNR {} dB, SSIM {} against floors {:.1} dB and {:.2}: {}",
            s.psnr_db
                .map(|v| format!("{v:.2}"))
                .unwrap_or_else(|| "n/a".to_string()),
            s.ssim
                .map(|v| format!("{v:.4}"))
                .unwrap_or_else(|| "n/a".to_string()),
            s.psnr_floor_db,
            s.ssim_floor,
            if s.passed {
                "passed".to_string()
            } else {
                format!(
                    "refused: {}",
                    s.refusal.as_deref().unwrap_or("floor missed")
                )
            }
        );
    }
    let _ = writeln!(o, "exit: {}", r.exit_code);
    o
}

/// JSON-escape a string as a complete quoted literal.
pub fn escape_json_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string())
}
