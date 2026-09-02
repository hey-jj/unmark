//! The calibration record pins. These run in CI without the corpus: the
//! committed fixture subset recomputes to the record's `fixture` section at
//! three decimals, the `[budget.*]` numbers equal the record's derived
//! numbers, the acceptance properties recompute from the record's own rows,
//! the record's encoder fingerprint equals the current one so a codec bump
//! fails CI until recalibration, and the owner table regenerates identically.

use std::path::Path;
use unmark::calibrate::{self, Context, Property, Record, Score};
use unmark::policy;

const RECORD: &str = include_str!("../policy/calibration.json");
const TABLE: &str = include_str!("../policy/calibration.md");
const LOCK: &str = include_str!("../Cargo.lock");

fn record() -> Record {
    serde_json::from_str(RECORD).expect("the committed record parses")
}

fn fixture_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/calibration")
}

#[test]
fn the_record_carries_this_schema_and_policy() {
    let r = record();
    let pkg = policy::load().unwrap();
    assert_eq!(r.schema_version, calibrate::RECORD_SCHEMA_VERSION);
    assert_eq!(r.tool_version, unmark::TOOL_VERSION);
    assert_eq!(r.policy_version, pkg.version);
    assert_eq!(r.n_min, pkg.calibration.n_min);
    assert_eq!(r.percentile_image, pkg.calibration.percentile_image);
    assert_eq!(r.percentile_audio, pkg.calibration.percentile_audio);
    assert_eq!(r.base_seed, format!("0x{:016x}", pkg.calibration.base_seed));
}

#[test]
fn the_policy_names_the_record_transitively() {
    let r = record();
    let pkg = policy::load().unwrap();
    let c = &pkg.calibration;
    assert_eq!(
        c.record_sha256,
        calibrate::sha256_hex(RECORD.as_bytes()),
        "policy.toml [calibration].record_sha256 is not the committed record's hash"
    );
    assert_eq!(c.corpus_manifest_sha256, r.corpus.manifest_sha256);
    assert_eq!(c.date, r.date);
    let expected = if r.accepted {
        "calibrated"
    } else {
        "provisional"
    };
    assert_eq!(c.status, expected);
    // Every degrade transform's pinned parameters are the record's.
    for pin in &r.transforms {
        let t = pkg.transform(&pin.id).expect("pinned transform exists");
        let keys: Vec<&str> = t.params.iter().map(|(k, _)| k.as_str()).collect();
        let pinned: Vec<&str> = pin.params.iter().map(|p| p.key.as_str()).collect();
        assert_eq!(
            keys, pinned,
            "{} parameter keys drifted from the record",
            pin.id
        );
        for (k, v) in &t.params {
            let rec = &pin.params.iter().find(|p| &p.key == k).unwrap().value;
            let now = match v {
                toml::Value::String(s) => serde_json::Value::String(s.clone()),
                toml::Value::Integer(i) => serde_json::Value::from(*i),
                toml::Value::Float(f) => serde_json::Value::from(*f),
                toml::Value::Boolean(b) => serde_json::Value::Bool(*b),
                other => serde_json::Value::String(other.to_string()),
            };
            assert_eq!(&now, rec, "{} parameter {k} changed; recalibrate", pin.id);
        }
    }
}

#[test]
fn fixture_scores_recompute_to_three_decimals() {
    let r = record();
    let pkg = policy::load().unwrap();
    let ctx = Context::new(&pkg).unwrap();
    let dir = fixture_dir();
    let manifest_text = std::fs::read_to_string(dir.join("manifest.json")).unwrap();
    let manifest = calibrate::parse_manifest(&manifest_text).unwrap();
    assert!(!r.fixture.is_empty(), "the record has a fixture section");
    let mut compared = 0;
    for a in &manifest.assets {
        let bytes = std::fs::read(dir.join(&a.path)).unwrap();
        let result = calibrate::evaluate_asset(&bytes, a, &ctx);
        if !cfg!(feature = "audio") && matches!(a.format.as_str(), "wav" | "flac") {
            assert_eq!(
                result.excluded.as_deref(),
                Some("audio feature not built"),
                "{}",
                a.path
            );
            continue;
        }
        assert!(
            result.excluded.is_none(),
            "{} excluded: {:?}",
            a.path,
            result.excluded
        );
        for p in &result.plans {
            let pinned = r
                .fixture
                .iter()
                .find(|f| f.path == a.path && f.plan == p.plan)
                .unwrap_or_else(|| panic!("record has no fixture row for {} {}", a.path, p.plan));
            assert_eq!(
                p.score, pinned.score,
                "{} {} recomputed differently",
                a.path, p.plan
            );
            compared += 1;
        }
    }
    assert!(compared > 0);
}

#[test]
fn no_fixture_row_is_an_error() {
    let r = record();
    for f in &r.fixture {
        assert!(
            !matches!(f.score, Score::Error { .. }),
            "{} {} is an error row: {:?}",
            f.path,
            f.plan,
            f.score
        );
    }
}

#[test]
fn budget_numbers_equal_the_derived_numbers() {
    let r = record();
    let pkg = policy::load().unwrap();
    let image = |name: &str| match pkg.budget_for(name) {
        unmark::budget::Budget::Image {
            psnr_floor_db,
            ssim_floor,
            ..
        } => (psnr_floor_db, ssim_floor),
        other => panic!("{name} is not an image budget: {other:?}"),
    };
    let audio = |name: &str| match pkg.budget_for(name) {
        unmark::budget::Budget::Audio { lsd_ceiling_db, .. } => lsd_ceiling_db,
        other => panic!("{name} is not an audio budget: {other:?}"),
    };
    let d = &r.derived;
    assert_eq!(
        image("image-safe"),
        (d.image_safe.psnr_floor_db, d.image_safe.ssim_floor)
    );
    assert_eq!(
        image("image-aggressive"),
        (
            d.image_aggressive.psnr_floor_db,
            d.image_aggressive.ssim_floor
        )
    );
    assert_eq!(audio("audio-safe"), d.audio_safe.lsd_ceiling_db);
    assert_eq!(audio("audio-aggressive"), d.audio_aggressive.lsd_ceiling_db);
    // Overrides match one for one.
    let mut expected: Vec<OverrideRow> = Vec::new();
    for (name, di) in [
        ("image-safe", &d.image_safe),
        ("image-aggressive", &d.image_aggressive),
    ] {
        for o in &di.overrides {
            expected.push((
                name.to_string(),
                o.format.clone(),
                o.psnr_floor_db,
                o.ssim_floor,
                None,
            ));
        }
    }
    for (name, da) in [
        ("audio-safe", &d.audio_safe),
        ("audio-aggressive", &d.audio_aggressive),
    ] {
        for o in &da.overrides {
            expected.push((
                name.to_string(),
                o.format.clone(),
                None,
                None,
                o.lsd_ceiling_db,
            ));
        }
    }
    let mut actual: Vec<OverrideRow> = pkg
        .overrides
        .iter()
        .map(|(n, f, b)| match b {
            unmark::budget::Budget::Image {
                psnr_floor_db,
                ssim_floor,
                ..
            } => (
                n.clone(),
                f.clone(),
                Some(*psnr_floor_db),
                Some(*ssim_floor),
                None,
            ),
            unmark::budget::Budget::Audio { lsd_ceiling_db, .. } => {
                (n.clone(), f.clone(), None, None, Some(*lsd_ceiling_db))
            }
            unmark::budget::Budget::Exact => (n.clone(), f.clone(), None, None, None),
        })
        .collect();
    expected.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    actual.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    assert_eq!(actual, expected, "policy overrides drifted from the record");
}

/// (budget, format, psnr floor, ssim floor, lsd ceiling)
type OverrideRow = (String, String, Option<f64>, Option<f64>, Option<f64>);

fn by_key(props: &[Property]) -> Vec<(String, Option<String>, bool)> {
    let mut v: Vec<(String, Option<String>, bool)> = props
        .iter()
        .map(|p| (p.name.clone(), p.cell.clone(), p.pass))
        .collect();
    v.sort();
    v
}

#[test]
fn the_properties_recompute_from_the_record() {
    let r = record();
    let recomputed = calibrate::recheck_properties(&r);
    assert_eq!(
        by_key(&recomputed),
        by_key(&r.properties),
        "stored properties disagree with the record's own rows"
    );
    assert_eq!(r.accepted, r.properties.iter().all(|p| p.pass));
    // Every qualified cell carries a next-tier median and enough assets.
    for c in r.cells.iter().filter(|c| c.qualified) {
        assert!(c.count >= r.n_min);
        assert!(
            c.next_tier.is_some(),
            "{}/{}/{} has no next tier",
            c.plan,
            c.format,
            c.band
        );
    }
}

#[test]
fn the_fingerprint_matches_the_current_build_and_a_mismatch_fails() {
    let r = record();
    assert_eq!(
        r.encoder_fingerprint,
        unmark::encoder_fingerprint(),
        "a codec crate or the in-crate encoder changed; recalibrate"
    );
    let mut tampered = r.clone();
    tampered.encoder_fingerprint = "png 0.0.0".to_string();
    let props = calibrate::recheck_properties(&tampered);
    let fp = props
        .iter()
        .find(|p| p.name == "encoder-fingerprint")
        .unwrap();
    assert!(!fp.pass, "a fingerprint mismatch must fail the property");
}

#[test]
fn the_pinned_codec_versions_match_cargo_lock() {
    for (name, version) in unmark::CODEC_CRATES {
        let needle = format!("name = \"{name}\"\nversion = \"{version}\"");
        assert!(
            LOCK.contains(&needle),
            "Cargo.lock does not pin {name} {version}; update CODEC_CRATES and recalibrate"
        );
    }
}

#[test]
fn every_metadata_cell_reports_byte_identity() {
    let r = record();
    assert!(!r.identity.is_empty());
    for i in &r.identity {
        assert!(
            i.identical,
            "{}/{} failed identity: {:?}",
            i.plan, i.format, i.failures
        );
    }
}

#[test]
fn the_owner_table_regenerates_identically() {
    let r = record();
    assert_eq!(
        calibrate::render_markdown(&r),
        TABLE,
        "policy/calibration.md drifted from the record; rerun the harness"
    );
}

#[test]
fn the_record_serializes_back_to_its_committed_bytes() {
    let r = record();
    assert_eq!(
        calibrate::record_json(&r),
        RECORD,
        "the record's key order or number formatting changed"
    );
}

#[test]
fn the_policy_rewrite_is_idempotent_on_the_committed_package() {
    let r = record();
    let text = policy::POLICY_TOML;
    let rewritten =
        calibrate::rewrite_policy(text, &r, &calibrate::sha256_hex(RECORD.as_bytes())).unwrap();
    assert_eq!(
        rewritten, text,
        "the committed policy is not what the harness would write"
    );
}
