//! Skill snapshot generation from the policy package. The snapshot carries the
//! policy digest and the mark-class, transform, held, and guardrail reference
//! so the skill's rule reference cannot drift from the code.

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

    let _ = writeln!(w, "## What each mark kind means");
    let _ = writeln!(w);
    let _ = writeln!(w, "- A confirmable mark has a structural address or a public decoder. The tool removes it and re-inspection proves it gone.");
    let _ = writeln!(w, "- A blind mark is a keyed statistical signal. The default run is applied to the asset, and the report names the mark as surviving with its citation.");
    let _ = writeln!(
        w,
        "- An unaddressed mark is one the tool knows and leaves in place, and the report names it."
    );
    let _ = writeln!(w, "- confirmed_absent over the enumerated supported set is the one state that reads as absent.");
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
        if let Some(s) = &m.survives {
            let _ = writeln!(w, "- survival: {s}");
        }
        if let Some(c) = &m.citation {
            let _ = writeln!(w, "- citation: {c}");
        }
        let _ = writeln!(w);
    }

    let _ = writeln!(w, "## Transforms in the default run");
    let _ = writeln!(w);
    for t in &pkg.transforms {
        let _ = writeln!(
            w,
            "### {} {} ({}, on {})",
            t.id,
            t.name,
            t.kind,
            t.applies.join(" ")
        );
        let _ = writeln!(w);
        let _ = writeln!(w, "- target: {}", t.target);
        if !t.params.is_empty() {
            let _ = writeln!(w, "- parameters: {}", t.params_rendered());
        }
        if let Some(s) = &t.strength {
            let _ = writeln!(w, "- strength: {s}");
        }
        if let Some(e) = &t.cited_effect {
            let _ = writeln!(w, "- cited effect: {e}");
        }
        if let Some(c) = &t.citation {
            let _ = writeln!(w, "- citation: {c}");
        }
        let _ = writeln!(w, "- {}", t.guard);
        let _ = writeln!(w);
    }

    let _ = writeln!(w, "## Held transforms");
    let _ = writeln!(w);
    let _ = writeln!(w, "| Id | Transform | Reason |");
    let _ = writeln!(w, "|---|---|---|");
    for h in &pkg.held {
        let _ = writeln!(w, "| {} | {} | {} |", h.id, h.name, h.reason);
    }
    let _ = writeln!(w);

    let _ = writeln!(w, "## Sanity floor");
    let _ = writeln!(w);
    let _ = writeln!(
        w,
        "Status {}. An output scoring below PSNR {:.1} dB or SSIM {:.2} against its grid-matched reference is a broken encode: clean exits 50 and writes nothing.",
        pkg.sanity.status, pkg.sanity.psnr_floor_db, pkg.sanity.ssim_floor
    );
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
