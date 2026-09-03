//! Packaging pins. These run against `CARGO_MANIFEST_DIR`, so inside an
//! unpacked `.crate` they read the published tree and not the repository: a
//! path that never reached the tarball fails here.

use std::path::Path;

const MANIFEST: &str = include_str!("../Cargo.toml");
const CHANGELOG: &str = include_str!("../CHANGELOG.md");

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn the_include_list_names_every_shipped_tree() {
    for entry in [
        "\"src/**\"",
        "\"examples/**\"",
        "\"policy/**\"",
        "\"skills/**\"",
        "\"tests/**\"",
        "\"fixtures/**\"",
        "\"README.md\"",
        "\"CHANGELOG.md\"",
        "\"LICENSE-MIT\"",
        "\"LICENSE-APACHE\"",
    ] {
        assert!(MANIFEST.contains(entry), "include list is missing {entry}");
    }
}

#[test]
fn the_published_tree_carries_the_skill_the_policy_and_the_suite() {
    for path in [
        "skills/unmark/SKILL.md",
        "skills/unmark/references/transforms.md",
        "policy/policy.toml",
        "policy/calibration.json",
        "policy/calibration.md",
        "src/calibrate/mod.rs",
        "src/calibrate/manifest.rs",
        "tools/ledger-annotations.py",
        "examples/calibrate.rs",
        "examples/calibration_fixtures.rs",
        "tests/calibration.rs",
        "fixtures/calibration/manifest.json",
        "fixtures/calibration/png-small.png",
        "fixtures/calibration/jpeg-large.jpg",
        "fixtures/calibration/webp-medium.webp",
        "fixtures/calibration/wav-short-stereo.wav",
        "fixtures/calibration/flac-long.flac",
        "policy/signatures/generator-strings.txt",
        "policy/signatures/camera-makes.txt",
        "tests/detect.rs",
        "tests/container_roundtrip.rs",
        "tests/clean_contract.rs",
        "tests/mc07_collision.rs",
        "tests/guard_prose.rs",
        "tests/policy.rs",
        "tests/fail_closed.rs",
        "tests/golden_fixtures.rs",
        "tests/malformed_walks.rs",
        "tests/residual_blocker_2.rs",
        "tests/report_consistency.rs",
        "tests/cli_exit_codes.rs",
        "fixtures/generated.png",
        "fixtures/tagged.wav",
        "fixtures/malformed.png",
        "fixtures/malformed.wav",
        "fixtures/malformed.jpg",
        "fixtures/malformed.flac",
        "fixtures/malformed.mp4",
        "fixtures/understated-size.wav",
        "fixtures/invisible-sample.txt",
        "README.md",
        "CHANGELOG.md",
    ] {
        assert!(root().join(path).is_file(), "{path} is not in the tree");
    }
}

#[test]
fn the_changelog_opens_on_the_crate_version() {
    let version = env!("CARGO_PKG_VERSION");
    let first = CHANGELOG
        .lines()
        .find(|l| l.starts_with("## "))
        .expect("the changelog has a release heading");
    assert!(
        first.contains(&format!("[{version}]")),
        "newest changelog entry is {first}, crate version is {version}"
    );
}
