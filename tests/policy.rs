//! Policy package integrity: the digest is stable and embedded, the snapshot
//! generates, and the supported-container list mirrors the code constant.

use unmark::asset::SUPPORTED_CONTAINERS;
use unmark::{policy, skill};

#[test]
fn the_digest_is_deterministic_and_embedded() {
    let a = policy::compute_digest();
    let b = policy::compute_digest();
    assert_eq!(a, b);
    assert!(a.starts_with("sha256:"));
    let pkg = policy::load().unwrap();
    assert_eq!(pkg.digest, a);
}

#[test]
fn the_package_carries_the_default_run_and_the_held_list() {
    let pkg = policy::load().unwrap();
    for id in [
        "MC01", "MC02", "MC03", "MC04", "MC05", "MC06", "MC07", "MC08", "MC09", "MC10", "PX01",
        "PX02", "AU06",
    ] {
        assert!(pkg.transform(id).is_some(), "missing transform {id}");
    }
    for id in ["PX04", "PX05", "AU01", "AU02", "AU04", "AU05", "AU03"] {
        assert!(pkg.held.iter().any(|h| h.id == id), "{id} is not held");
        assert!(pkg.transform(id).is_none(), "{id} must not be runnable");
    }
    assert_eq!(pkg.guards.len(), 5);
    assert!(pkg.transform("PX02").unwrap().cited_effect.is_some());
    assert!(pkg.mark_class("dwtdct").is_some());
    assert_eq!(pkg.sanity.status, "proposed");
}

#[test]
fn the_shipped_policy_carries_no_calibration_or_profile_vocabulary() {
    let text = policy::POLICY_TOML.to_ascii_lowercase();
    for word in [
        "[profile",
        "[budget",
        "band",
        "cell",
        "percentile",
        "residual",
        "content_class",
        "i-generated-this",
        "acknowledge",
    ] {
        assert!(!text.contains(word), "policy.toml carries {word:?}");
    }
}

#[test]
fn the_supported_list_mirrors_the_code_constant() {
    let pkg = policy::load().unwrap();
    let code: Vec<String> = SUPPORTED_CONTAINERS
        .iter()
        .map(|f| f.as_str().to_string())
        .collect();
    assert_eq!(
        pkg.supported_containers, code,
        "policy list drifted from the code constant"
    );
}

#[test]
fn the_snapshot_generates_and_names_the_digest() {
    let pkg = policy::load().unwrap();
    let snap = skill::generate(&pkg);
    assert!(snap.contains(&pkg.digest));
    assert!(snap.contains("MC04"));
    assert!(snap.contains("Held, not runnable"));
    assert!(snap.contains("never emits a clean verdict"));
}

#[test]
fn the_committed_snapshot_is_current() {
    // The shipped reference must match a fresh generation, so the skill's
    // reference cannot drift from the code.
    let pkg = policy::load().unwrap();
    let generated = skill::generate(&pkg);
    let committed = include_str!("../skills/unmark/references/transforms.md");
    assert_eq!(
        generated, committed,
        "run `unmark policy snapshot --out skills/unmark/references/transforms.md`"
    );
}
