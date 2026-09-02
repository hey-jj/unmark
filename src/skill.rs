//! Skill snapshot generation from the policy package. The snapshot carries the
//! policy digest and the transform, profile, and guardrail reference so the
//! skill's rule reference cannot drift from the code.

use crate::policy::PolicyPackage;
use std::fmt::Write;

pub fn generate(pkg: &PolicyPackage) -> String {
    let mut out = String::new();
    let w = &mut out;
    let _ = writeln!(w, "# unmark transform reference");
    let _ = writeln!(w);
    let _ = writeln!(
        w,
        "Regenerate with `unmark policy snapshot` after any policy change. Edits here are overwritten."
    );
    let _ = writeln!(w);
    let _ = writeln!(w, "- policy version: {}", pkg.version);
    let _ = writeln!(w, "- policy digest: {}", pkg.digest);
    let _ = writeln!(w);

    let _ = writeln!(w, "## What the tool may say about each mark");
    let _ = writeln!(w);
    let _ = writeln!(w, "- A confirmable mark has a structural address. The tool removes it and re-inspection proves it gone.");
    let _ = writeln!(w, "- A blind mark is a keyed statistical signal this offline build cannot see, so it reports the mark as not_attempted, the scan state for a class this build cannot examine.");
    let _ = writeln!(w, "- An unaddressed mark is one the tool knows and does nothing to, named so its absence from the action list is not read as its absence from the asset.");
    let _ = writeln!(w, "- Only confirmed_absent over the enumerated supported set licenses the word absent. The build never emits a clean verdict and never renders no marks found as human authorship.");
    let _ = writeln!(w);

    let _ = writeln!(w, "## Supported containers");
    let _ = writeln!(w);
    let _ = writeln!(w, "A scan reports confirmed_absent only over these. Everything else reports unsupported_format.");
    let _ = writeln!(w);
    for c in &pkg.supported_containers {
        let _ = writeln!(w, "- {c}");
    }
    let _ = writeln!(w);

    let _ = writeln!(w, "## Mark classes");
    let _ = writeln!(w);
    for m in &pkg.mark_classes {
        let _ = writeln!(w, "### {} ({})", m.id, m.honesty);
        let _ = writeln!(w);
        let _ = writeln!(w, "- {}: {}", m.label, m.guard);
        if let Some(j) = &m.judge {
            let _ = writeln!(w, "- judge: {j}");
        }
        let _ = writeln!(w);
    }

    let _ = writeln!(w, "## Transforms");
    let _ = writeln!(w);
    for t in &pkg.transforms {
        let _ = writeln!(
            w,
            "### {} {} (tier {}, fidelity {}, default {}, milestone {})",
            t.id, t.name, t.tier, t.fidelity, t.default, t.milestone
        );
        let _ = writeln!(w);
        let _ = writeln!(w, "- target: {}", t.target);
        if !t.params.is_empty() {
            let _ = writeln!(w, "- parameters: {}", t.params_rendered());
        }
        let _ = writeln!(w, "- {}", t.guard);
        let _ = writeln!(w);
    }

    let _ = writeln!(w, "## Profiles");
    let _ = writeln!(w);
    let _ = writeln!(
        w,
        "| Profile | Media | Tier | Milestone | Transforms | Budget |"
    );
    let _ = writeln!(w, "|---|---|---|---|---|---|");
    for p in &pkg.profiles {
        let _ = writeln!(
            w,
            "| {} | {} | {} | {} | {} | {} |",
            p.name,
            p.media,
            p.tier,
            p.milestone,
            p.transforms.join(" "),
            p.budget
        );
    }
    let _ = writeln!(w);

    let _ = writeln!(w, "## Fidelity budgets");
    let _ = writeln!(w);
    let c = &pkg.calibration;
    let _ = writeln!(
        w,
        "Calibration status: {}. Record: {}. Record sha256: {}. Corpus manifest sha256: {}. Date: {}.",
        c.status,
        c.record,
        if c.record_sha256.is_empty() { "none" } else { &c.record_sha256 },
        if c.corpus_manifest_sha256.is_empty() { "none" } else { &c.corpus_manifest_sha256 },
        if c.date.is_empty() { "none" } else { &c.date }
    );
    let _ = writeln!(w);
    let _ = writeln!(w, "| Budget | Kind | Signal floor or ceiling | Geometry |");
    let _ = writeln!(w, "|---|---|---|---|");
    for (name, b) in &pkg.budgets {
        match b {
            crate::budget::Budget::Exact => {
                let _ = writeln!(w, "| {name} | exact | byte identity | none |");
            }
            crate::budget::Budget::Image {
                psnr_floor_db,
                ssim_floor,
                resample_ratio_min,
                crop_area_min,
            } => {
                let _ = writeln!(
                    w,
                    "| {name} | image | PSNR at or above {psnr_floor_db:.1} dB, SSIM at or above {ssim_floor:.3} | resample ratio at or above {resample_ratio_min:.2}, crop keeps at or above {crop_area_min:.2} of the area |"
                );
            }
            crate::budget::Budget::Audio {
                lsd_ceiling_db,
                resample_ratio_min,
                time_stretch_min,
                time_stretch_max,
            } => {
                let _ = writeln!(
                    w,
                    "| {name} | audio | log-spectral distance at or below {lsd_ceiling_db:.2} dB | resample ratio at or above {resample_ratio_min:.2}, time-stretch within {time_stretch_min:.2} to {time_stretch_max:.2} |"
                );
            }
        }
    }
    let _ = writeln!(w);

    let _ = writeln!(w, "## Guardrails");
    let _ = writeln!(w);
    for g in &pkg.guards {
        let _ = writeln!(w, "### {} {}", g.id, g.name);
        let _ = writeln!(w);
        let _ = writeln!(w, "- {}", g.guard);
        let _ = writeln!(w);
    }
    out
}
