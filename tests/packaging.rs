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
        "src/mark/dwtdct.rs",
        "src/capture.rs",
        "tools/oracle.py",
        "tools/render_fixtures.py",
        "examples/metrics.rs",
        "tests/efficacy.rs",
        "fixtures/efficacy/flat-illustration-sdxl.png",
        "fixtures/efficacy/flat-illustration-compvis.png",
        "fixtures/efficacy/text-ui-base.png",
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
        "tests/audio_and_mp4.rs",
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
    // The first versioned heading; an [Unreleased] section may precede it.
    let first = CHANGELOG
        .lines()
        .find(|l| l.starts_with("## [") && l[4..].starts_with(|c: char| c.is_ascii_digit()))
        .expect("the changelog has a release heading");
    assert!(
        first.contains(&format!("[{version}]")),
        "newest changelog entry is {first}, crate version is {version}"
    );
}
