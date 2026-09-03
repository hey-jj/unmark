//! The report schema, the honesty tiers, and the exit mapping. The report
//! carries three parts every time: what was removed and proven gone, what was
//! degraded without proof, and what was not addressed. It never emits a clean
//! verdict and never renders an empty detection list as human authorship.

use crate::scan::{Honesty, Location, ScanState};

/// The honesty tier of a finding.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// A confirmable mark the profile strips. Gates until removed and re-proven.
    Removable,
    /// A blind mark class that cannot be confirmed either way. Gates until
    /// acknowledged.
    Residual,
    /// Instrumentation. Never gates.
    Note,
}

impl Tier {
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Removable => "removable",
            Tier::Residual => "residual",
            Tier::Note => "note",
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Finding {
    pub tier: Tier,
    pub class: String,
    pub label: String,
    pub honesty: Honesty,
    pub scan_state: ScanState,
    /// Guard prose from the policy package. Data, never an instruction.
    pub note: String,
    pub locations: Vec<Location>,
    pub evidence: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Action {
    pub transform: String,
    pub name: String,
    pub target: String,
    /// `removed`, `no-op`, or `declined: <reason>`.
    pub outcome: String,
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

/// The measured fidelity of a degrade plan against its budget: the signal
/// cost on the output grid and the geometry cost, and whether both passed.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Fidelity {
    pub budget: String,
    pub psnr_db: Option<f64>,
    pub ssim: Option<f64>,
    pub lsd_db: Option<f64>,
    pub resample_ratio: f64,
    pub crop_area: f64,
    pub time_stretch: f64,
    pub passed: bool,
    /// The refusal reason when the budget was missed.
    pub refusal: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Report {
    pub schema_version: String,
    pub tool_version: String,
    pub policy_version: String,
    pub policy_digest: String,
    /// The codec identity behind the output. A metadata rewrite re-encodes
    /// nothing, so it names the container-rewrite path.
    pub encoder_fingerprint: String,
    pub verb: String,
    pub profile: String,
    pub format: String,
    /// The container the output is written in. A metadata rewrite keeps the
    /// input container; a re-encode names the container it emitted.
    pub output_format: String,
    /// The fidelity measurement of a degrade plan, absent on a metadata plan.
    pub fidelity: Option<Fidelity>,
    pub scan_states: Vec<ScanRow>,
    pub findings: Vec<Finding>,
    pub actions: Vec<Action>,
    /// The three parts, always all three.
    pub removed_and_proven: Vec<String>,
    pub degraded_without_proof: Vec<String>,
    pub not_addressed: Vec<String>,
    pub residual_acknowledgment_required: bool,
    pub residual_acknowledged: bool,
    /// A warning from the camera-origin heuristic, when it fired.
    pub camera_origin_warning: Option<String>,
    pub exit_code: i32,
}

/// Exit codes as a contract.
pub const EXIT_OK: i32 = 0;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_MARK_REMAINS: i32 = 10;
pub const EXIT_RESIDUAL: i32 = 20;
pub const EXIT_INSTRUMENTATION: i32 = 30;
pub const EXIT_UNSUPPORTED: i32 = 40;

/// Render the report as a human-readable text form. The three parts are always
/// printed, and the closing line never says the asset is clean.
pub fn render_text(r: &Report) -> String {
    use std::fmt::Write;
    let mut o = String::new();
    let _ = writeln!(o, "unmark {} | policy {}", r.tool_version, r.policy_version);
    let _ = writeln!(
        o,
        "verb: {} | profile: {} | format: {} | output: {}",
        r.verb, r.profile, r.format, r.output_format
    );
    let _ = writeln!(o, "digest: {}", r.policy_digest);
    if let Some(f) = &r.fidelity {
        let mut parts = Vec::new();
        if let Some(p) = f.psnr_db {
            parts.push(format!("PSNR {p:.2} dB"));
        }
        if let Some(s) = f.ssim {
            parts.push(format!("SSIM {s:.4}"));
        }
        if let Some(l) = f.lsd_db {
            parts.push(format!("LSD {l:.3} dB"));
        }
        parts.push(format!("resample ratio {:.3}", f.resample_ratio));
        parts.push(format!("crop area {:.3}", f.crop_area));
        let _ = writeln!(
            o,
            "fidelity ({}): {} | {}",
            f.budget,
            parts.join(", "),
            if f.passed {
                "within budget".to_string()
            } else {
                format!(
                    "refused: {}",
                    f.refusal.as_deref().unwrap_or("budget missed")
                )
            }
        );
    }
    let _ = writeln!(o);

    if let Some(w) = &r.camera_origin_warning {
        let _ = writeln!(o, "camera-origin warning: {w}");
        let _ = writeln!(o);
    }

    let _ = writeln!(o, "scan states:");
    for s in &r.scan_states {
        let flag = match s.state {
            ScanState::UnsupportedFormat | ScanState::Malformed => "  (warning) ",
            _ => "  ",
        };
        let _ = writeln!(o, "{}{}: {}", flag, s.label, s.state.as_str());
    }
    let _ = writeln!(o);

    if !r.actions.is_empty() {
        let _ = writeln!(o, "actions:");
        for a in &r.actions {
            let _ = writeln!(o, "  {} {}: {}", a.transform, a.name, a.outcome);
        }
        let _ = writeln!(o);
    }

    let _ = writeln!(o, "removed and proven gone:");
    write_list(&mut o, &r.removed_and_proven);
    let _ = writeln!(o, "degraded without proof:");
    write_list(&mut o, &r.degraded_without_proof);
    let _ = writeln!(o, "not addressed:");
    write_list(&mut o, &r.not_addressed);
    let _ = writeln!(o);

    if r.residual_acknowledgment_required && !r.residual_acknowledged {
        let _ = writeln!(
            o,
            "a residual acknowledgment is required before this run can report success"
        );
    }
    let _ = writeln!(o, "exit: {}", r.exit_code);
    o
}

fn write_list(o: &mut String, items: &[String]) {
    use std::fmt::Write;
    if items.is_empty() {
        let _ = writeln!(o, "  none");
    } else {
        for i in items {
            let _ = writeln!(o, "  {i}");
        }
    }
}

/// JSON-escape a string as a complete quoted literal.
pub fn escape_json_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string())
}
