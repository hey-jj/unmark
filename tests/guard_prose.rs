//! The guard-prose gate. Every guard and judge string in the policy package
//! ships to a reader inside the skill reference, so this test reads them all and
//! fails the run on the mechanical writing classes: the dash family, semicolons,
//! banned filler, and contrast scaffolding.
//!
//! The scope is the prose fields only. Signature terms, ids, and match tables
//! are never read, so a data entry cannot raise a finding.

use unmark::policy;

const BANNED_FILLER: &[&str] = &[
    "it's important to note",
    "it is important to note",
    "in summary",
    "in conclusion",
    "overall",
    "delve",
    "leverage",
    "seamless",
    "game-changer",
    "at its core",
    "needless to say",
];

const SCAFFOLDING: &[&str] = &[
    "on one hand",
    "on the one hand",
    "on the other hand",
    "best of both worlds",
    "without sacrificing",
    "no silver bullet",
    "one-size-fits-all",
    "strikes a balance",
    "all while",
];

fn word_bounded(hay: &str, at: usize, len: usize) -> bool {
    let before = hay[..at].chars().next_back();
    let after = hay[at + len..].chars().next();
    !before.is_some_and(|c| c.is_alphanumeric()) && !after.is_some_and(|c| c.is_alphanumeric())
}

fn scan(id: &str, field: &str, text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (class, mark) in [
        ("em dash", '\u{2014}'),
        ("en dash", '\u{2013}'),
        ("semicolon", ';'),
    ] {
        if text.contains(mark) {
            out.push(format!("{id} {field}: {class}"));
        }
    }
    let lower = text.to_lowercase();
    for phrase in BANNED_FILLER.iter().chain(SCAFFOLDING) {
        let mut at = 0;
        while let Some(pos) = lower[at..].find(phrase) {
            let s = at + pos;
            if word_bounded(&lower, s, phrase.len()) {
                out.push(format!("{id} {field}: {phrase:?}"));
            }
            at = s + 1;
        }
    }
    out
}

#[test]
fn every_guard_and_judge_string_passes_the_mechanical_classes() {
    let pkg = policy::load().unwrap();
    let mut found = Vec::new();
    for m in &pkg.mark_classes {
        found.extend(scan(&m.id, "guard", &m.guard));
        if let Some(s) = &m.survives {
            found.extend(scan(&m.id, "survives", s));
        }
    }
    for t in &pkg.transforms {
        found.extend(scan(&t.id, "guard", &t.guard));
        if let Some(e) = &t.cited_effect {
            found.extend(scan(&t.id, "cited_effect", e));
        }
    }
    for h in &pkg.held {
        found.extend(scan(&h.id, "reason", &h.reason));
    }
    for g in &pkg.guards {
        found.extend(scan(&g.id, "guard", &g.guard));
    }
    assert!(
        found.is_empty(),
        "guard prose carries {} mechanical violation(s):\n  {}",
        found.len(),
        found.join("\n  ")
    );
}

/// The negative control. A synthetic string carrying one instance of each class
/// comes back with all of them, so a green run above means the gate looked.
#[test]
fn the_gate_catches_a_synthetic_string() {
    let bad = "It fires here \u{2014} and there \u{2013} on one hand; it delves.";
    let found = scan("TEST", "guard", bad);
    for want in ["em dash", "en dash", "semicolon", "\"on one hand\""] {
        assert!(found.iter().any(|f| f.contains(want)), "missed {want}");
    }
    // A clean string produces nothing.
    assert!(scan("TEST", "guard", "It reads the chunk and reports the span.").is_empty());
}
