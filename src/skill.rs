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
            "### {} {} (tier {}, fidelity {}, default {})",
            t.id, t.name, t.tier, t.fidelity, t.default
        );
        let _ = writeln!(w);
        let _ = writeln!(w, "- target: {}", t.target);
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
