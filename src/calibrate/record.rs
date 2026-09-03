//! The calibration record: its shape, its construction from per-asset
//! results, the acceptance properties, and the recheck the test suite runs
//! against the committed record.

use super::cells::{band_groups, build_cell, ladder_rows, CellInput, CellRow, LadderRow};
use super::evaluate::{AssetResult, PlanScore, PlanSet, Score};
use super::manifest::{Eligibility, Exclusion, Filters, SourceInventory};
use super::{
    AUDIO_BANDS, IMAGE_BANDS, PROVISIONAL_AUDIO_AGGRESSIVE, PROVISIONAL_AUDIO_SAFE,
    PROVISIONAL_IMAGE_AGGRESSIVE, PROVISIONAL_IMAGE_SAFE, RECORD_SCHEMA_VERSION,
};
use crate::asset::Media;
use crate::policy::PolicyPackage;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FormatOverride {
    pub format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub psnr_floor_db: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssim_floor: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lsd_ceiling_db: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DerivedImage {
    pub psnr_floor_db: f64,
    pub ssim_floor: f64,
    /// `calibrated` or `provisional`.
    pub source: String,
    pub provisional_psnr_floor_db: f64,
    pub provisional_ssim_floor: f64,
    pub overrides: Vec<FormatOverride>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DerivedAudio {
    pub lsd_ceiling_db: f64,
    pub source: String,
    pub provisional_lsd_ceiling_db: f64,
    pub overrides: Vec<FormatOverride>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Derived {
    #[serde(rename = "image-safe")]
    pub image_safe: DerivedImage,
    #[serde(rename = "image-aggressive")]
    pub image_aggressive: DerivedImage,
    #[serde(rename = "audio-safe")]
    pub audio_safe: DerivedAudio,
    #[serde(rename = "audio-aggressive")]
    pub audio_aggressive: DerivedAudio,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Property {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell: Option<String>,
    pub pass: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IdentityRow {
    pub plan: String,
    pub format: String,
    pub count: usize,
    pub identical: bool,
    /// doc_ids of the assets whose decoded stream changed.
    pub failures: Vec<String>,
    /// Assets whose strip was declined, so nothing was written.
    pub declined: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Corpus {
    pub manifest: String,
    pub manifest_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_inventory: Option<SourceInventory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prepare: Option<super::manifest::PrepareInfo>,
    pub filters: Filters,
    pub assets_listed: usize,
    pub assets_scored: usize,
    pub assets_excluded: usize,
    /// Scored assets by eligibility class.
    pub by_eligibility: Vec<(String, usize)>,
    pub exclusion_counts: Vec<(String, usize)>,
    pub exclusions: Vec<Exclusion>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub key: String,
    pub value: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransformPin {
    pub id: String,
    pub params: Vec<Param>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Determinism {
    pub checked: bool,
    pub identical: bool,
}

/// A per-asset row of the record, keyed by doc_id and sha256.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AssetRecord {
    pub doc_id: String,
    pub sha256: String,
    pub container: String,
    pub band: String,
    pub derived: String,
    pub eligibility: Eligibility,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub documented_marks: Vec<String>,
    pub generator: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub content_class: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ladder_step: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excluded: Option<String>,
    pub plans: Vec<PlanScore>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FixtureRow {
    pub doc_id: String,
    pub sha256: String,
    pub plan: String,
    #[serde(flatten)]
    pub score: Score,
}

/// The calibration record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub schema_version: String,
    pub tool_version: String,
    pub policy_version: String,
    pub date: String,
    pub corpus: Corpus,
    pub encoder_fingerprint: String,
    pub base_seed: String,
    pub percentile_image: u32,
    pub percentile_audio: u32,
    pub n_min: usize,
    /// Every required content class must reach this count in a cell.
    pub class_min: usize,
    pub psnr_step_db: f64,
    pub ssim_step: f64,
    pub lsd_step_db: f64,
    /// How separation is judged: it holds when either metric's next-tier
    /// median fails the number, since the gate refuses on either.
    pub separation_rule: String,
    pub metrics: Vec<Param>,
    pub transforms: Vec<TransformPin>,
    /// Floor-setting cells: native and container-derived.
    pub cells: Vec<CellRow>,
    /// Report-only rows: controls, strata, report-only groups by reason, and
    /// the documented-marks splits.
    pub strata: Vec<CellRow>,
    /// The post-processed ladder, pass and refuse per step. Report only.
    pub ladder: Vec<LadderRow>,
    pub identity: Vec<IdentityRow>,
    pub derived: Derived,
    pub properties: Vec<Property>,
    pub determinism: Determinism,
    pub accepted: bool,
    pub assets: Vec<AssetRecord>,
    pub fixture: Vec<FixtureRow>,
}

pub fn toml_to_json(v: &toml::Value) -> serde_json::Value {
    match v {
        toml::Value::String(s) => serde_json::Value::String(s.clone()),
        toml::Value::Integer(i) => serde_json::Value::from(*i),
        toml::Value::Float(f) => serde_json::Value::from(*f),
        toml::Value::Boolean(b) => serde_json::Value::Bool(*b),
        toml::Value::Array(a) => serde_json::Value::Array(a.iter().map(toml_to_json).collect()),
        toml::Value::Table(t) => serde_json::Value::Object(
            t.iter()
                .map(|(k, v)| (k.clone(), toml_to_json(v)))
                .collect(),
        ),
        toml::Value::Datetime(d) => serde_json::Value::String(d.to_string()),
    }
}

fn image_fails(psnr_med: Option<f64>, ssim_med: Option<f64>, floor: (f64, f64)) -> Option<bool> {
    match (psnr_med, ssim_med) {
        (Some(p), Some(s)) => Some(p < floor.0 || s < floor.1),
        _ => None,
    }
}

/// The separation rule as recorded. The worst-stack score takes each metric
/// independently, and the next tier fails the number when either metric's
/// median misses it, because the gate refuses on either.
pub const SEPARATION_RULE: &str =
    "the next tier fails when either metric's median misses the number; the gate refuses on either";

/// The reason class a report-only row groups under.
pub fn reason_class(reason: &str) -> String {
    let r = reason
        .split(" of ")
        .next()
        .unwrap_or(reason)
        .split(':')
        .next()
        .unwrap_or(reason)
        .trim();
    r.to_string()
}

/// The cell formats per medium. Alpha images are their own cells.
pub fn formats_for(media: Media) -> Vec<&'static str> {
    match media {
        Media::Image => vec!["png", "png-alpha", "jpeg", "webp", "webp-alpha"],
        _ => vec!["wav", "flac"],
    }
}

/// Build the record from per-asset results. `manifest_name` is the manifest
/// file's basename; the date is an input, never the clock.
#[allow(clippy::too_many_arguments)]
pub fn build_record(
    pkg: &PolicyPackage,
    results: &[AssetResult],
    manifest_name: &str,
    manifest_sha256: &str,
    source_inventory: Option<SourceInventory>,
    prepare: Option<super::manifest::PrepareInfo>,
    filters: &Filters,
    date: &str,
) -> Result<Record, String> {
    let c = &pkg.calibration;
    let image_plans = PlanSet::from_policy(pkg, Media::Image)?;
    let audio_plans = PlanSet::from_policy(pkg, Media::Audio)?;
    let scored: Vec<&AssetResult> = results.iter().filter(|r| r.excluded.is_none()).collect();
    let exclusions: Vec<Exclusion> = results
        .iter()
        .filter_map(|r| {
            r.excluded.as_ref().map(|reason| Exclusion {
                doc_id: r.doc_id.clone(),
                reason: reason.clone(),
            })
        })
        .collect();
    let mut exclusion_counts: BTreeMap<String, usize> = BTreeMap::new();
    for e in &exclusions {
        *exclusion_counts.entry(reason_class(&e.reason)).or_default() += 1;
    }
    let mut by_eligibility: BTreeMap<String, usize> = BTreeMap::new();
    for r in &scored {
        *by_eligibility
            .entry(r.eligibility.as_str().to_string())
            .or_default() += 1;
    }

    let mut cells = Vec::new();
    let mut strata = Vec::new();
    let mut identity = Vec::new();
    let mut ladder = Vec::new();

    for (media, plans, bands) in [
        (Media::Image, &image_plans, &IMAGE_BANDS[..]),
        (Media::Audio, &audio_plans, &AUDIO_BANDS[..]),
    ] {
        let two = plans.two_opt_in_name();
        let mut plan_list: Vec<(String, bool, Option<String>)> = vec![
            (
                plans.safe_name.clone(),
                false,
                Some(plans.aggressive_name.clone()),
            ),
            (plans.aggressive_name.clone(), false, Some(two.clone())),
        ];
        for opt in &plans.opt_ins {
            plan_list.push((format!("{}+{opt}", plans.aggressive_name), true, None));
        }
        plan_list.push((two.clone(), true, None));
        for id in &plans.alone {
            plan_list.push((id.clone(), true, None));
        }
        let budget_of = |name: &str| {
            pkg.profile(name)
                .map(|p| p.budget.clone())
                .unwrap_or_else(|| name.to_string())
        };
        let ladder_plans = vec![
            (plans.safe_name.clone(), budget_of(&plans.safe_name)),
            (
                plans.aggressive_name.clone(),
                budget_of(&plans.aggressive_name),
            ),
        ];
        let media_assets: Vec<&AssetResult> = scored
            .iter()
            .copied()
            .filter(|a| formats_for(media).contains(&a.container.as_str()))
            .collect();
        ladder.extend(ladder_rows(pkg, &media_assets, &ladder_plans));

        for format in formats_for(media) {
            let of_format: Vec<&AssetResult> = scored
                .iter()
                .copied()
                .filter(|a| a.container == format)
                .collect();
            if of_format.is_empty() {
                continue;
            }
            // Identity cell over every scored asset of the format.
            let failures: Vec<String> = of_format
                .iter()
                .filter(|a| {
                    !matches!(
                        super::cells::plan_score(a, &plans.metadata_name),
                        Some(Score::Identity {
                            identical: true,
                            ..
                        })
                    )
                })
                .map(|a| a.doc_id.clone())
                .collect();
            identity.push(IdentityRow {
                plan: plans.metadata_name.clone(),
                format: format.to_string(),
                count: of_format.len(),
                identical: failures.is_empty(),
                failures,
                declined: 0,
            });

            let native: Vec<&AssetResult> = of_format
                .iter()
                .copied()
                .filter(|a| a.eligibility == Eligibility::Floor)
                .collect();
            let container: Vec<&AssetResult> = of_format
                .iter()
                .copied()
                .filter(|a| a.eligibility == Eligibility::Container)
                .collect();
            let controls: Vec<&AssetResult> = of_format
                .iter()
                .copied()
                .filter(|a| a.eligibility == Eligibility::Control)
                .collect();
            let report_only: Vec<&AssetResult> = of_format
                .iter()
                .copied()
                .filter(|a| a.eligibility == Eligibility::ReportOnly)
                .collect();
            let collapsed_in = |band_names: &[String], pixel: bool| -> usize {
                report_only
                    .iter()
                    .filter(|a| {
                        band_names.contains(&a.band)
                            && a.reason
                                .as_deref()
                                .is_some_and(|r| r.starts_with("near-duplicate"))
                            && (a.near_duplicate_rule == "pixel-sha256") == pixel
                    })
                    .count()
            };

            for (plan, informational, next) in &plan_list {
                for (kind, set) in [("none", &native), ("container", &container)] {
                    if set.is_empty() {
                        continue;
                    }
                    for (band_names, assets) in band_groups(set, plan, bands, c.n_min) {
                        let collapsed = collapsed_in(&band_names, false);
                        let pixel_collapsed = collapsed_in(&band_names, true);
                        let input = CellInput {
                            plan: plan.clone(),
                            format: format.to_string(),
                            bands: band_names,
                            derived: kind.to_string(),
                            assets,
                            collapsed,
                            pixel_collapsed,
                        };
                        cells.push(build_cell(&input, pkg, *informational, next.as_deref()));
                    }
                }
                if *informational {
                    continue;
                }
                // Report-only rows for the two calibrated plans.
                let mut push_stratum = |label: String, assets: Vec<&AssetResult>, derived: &str| {
                    if assets.is_empty() {
                        return;
                    }
                    let input = CellInput {
                        plan: plan.clone(),
                        format: format.to_string(),
                        bands: vec![label],
                        derived: derived.to_string(),
                        assets,
                        collapsed: 0,
                        pixel_collapsed: 0,
                    };
                    strata.push(build_cell(&input, pkg, true, next.as_deref()));
                };
                push_stratum("control".to_string(), controls.clone(), "none");
                for stratum in ["mcu-padded", "short-clip"] {
                    let in_stratum: Vec<&AssetResult> = native
                        .iter()
                        .copied()
                        .filter(|a| a.strata.iter().any(|s| s == stratum))
                        .collect();
                    push_stratum(stratum.to_string(), in_stratum, "none");
                }
                // Report-only groups by reason class and band.
                let mut groups: BTreeMap<(String, String, String), Vec<&AssetResult>> =
                    BTreeMap::new();
                for a in &report_only {
                    let class = reason_class(a.reason.as_deref().unwrap_or("report-only"));
                    groups
                        .entry((class, a.band.clone(), a.derived.clone()))
                        .or_default()
                        .push(a);
                }
                for ((class, band, derived), assets) in groups {
                    push_stratum(format!("report:{class}/{band}"), assets, &derived);
                }
                // Content-class splits over the floor-setting rows.
                let mut by_class: BTreeMap<String, Vec<&AssetResult>> = BTreeMap::new();
                for a in native.iter().chain(container.iter()) {
                    let key = if a.content_class.is_empty() {
                        "unclassified".to_string()
                    } else {
                        a.content_class.clone()
                    };
                    by_class.entry(key).or_default().push(a);
                }
                if by_class.len() > 1 {
                    for (key, assets) in by_class {
                        push_stratum(format!("class:{key}"), assets, "none");
                    }
                }
                // Documented-marks splits over the floor-setting rows.
                let mut marks: BTreeMap<String, Vec<&AssetResult>> = BTreeMap::new();
                for a in native.iter().chain(container.iter()) {
                    let key = if a.documented_marks.is_empty() {
                        "unmarked".to_string()
                    } else {
                        let mut m = a.documented_marks.clone();
                        m.sort();
                        m.dedup();
                        m.join("+")
                    };
                    marks.entry(key).or_default().push(a);
                }
                if marks.len() > 1 {
                    for (key, assets) in marks {
                        push_stratum(format!("marks:{key}"), assets, "none");
                    }
                }
            }
        }
    }

    // Identity-only containers: rewritable, never decoded.
    for format in ["mp4", "mp3"] {
        let rows: Vec<&AssetResult> = scored
            .iter()
            .copied()
            .filter(|a| a.container == format)
            .collect();
        if rows.is_empty() {
            continue;
        }
        let mut failures = Vec::new();
        let mut declined = 0;
        for a in &rows {
            match super::cells::plan_score(a, &audio_plans.metadata_name) {
                Some(Score::Identity {
                    identical: true,
                    detail,
                }) => {
                    if detail.starts_with("declined") {
                        declined += 1;
                    }
                }
                _ => failures.push(a.doc_id.clone()),
            }
        }
        identity.push(IdentityRow {
            plan: audio_plans.metadata_name.clone(),
            format: format.to_string(),
            count: rows.len(),
            identical: failures.is_empty(),
            failures,
            declined,
        });
    }

    // Derived numbers: the most permissive value across qualified cells.
    let derive_image = |plan: &str, provisional: (f64, f64)| -> DerivedImage {
        let q: Vec<&super::cells::ImageStats> = cells
            .iter()
            .filter(|r| r.plan == plan && r.qualified)
            .filter_map(|r| r.image.as_ref())
            .collect();
        if q.is_empty() {
            DerivedImage {
                psnr_floor_db: provisional.0,
                ssim_floor: provisional.1,
                source: "provisional".to_string(),
                provisional_psnr_floor_db: provisional.0,
                provisional_ssim_floor: provisional.1,
                overrides: Vec::new(),
            }
        } else {
            DerivedImage {
                psnr_floor_db: q
                    .iter()
                    .map(|s| s.psnr_floor_db)
                    .fold(f64::INFINITY, f64::min),
                ssim_floor: q.iter().map(|s| s.ssim_floor).fold(f64::INFINITY, f64::min),
                source: "calibrated".to_string(),
                provisional_psnr_floor_db: provisional.0,
                provisional_ssim_floor: provisional.1,
                overrides: Vec::new(),
            }
        }
    };
    let derive_audio = |plan: &str, provisional: f64| -> DerivedAudio {
        let q: Vec<&super::cells::AudioStats> = cells
            .iter()
            .filter(|r| r.plan == plan && r.qualified)
            .filter_map(|r| r.audio.as_ref())
            .collect();
        if q.is_empty() {
            DerivedAudio {
                lsd_ceiling_db: provisional,
                source: "provisional".to_string(),
                provisional_lsd_ceiling_db: provisional,
                overrides: Vec::new(),
            }
        } else {
            DerivedAudio {
                lsd_ceiling_db: q
                    .iter()
                    .map(|s| s.lsd_ceiling_db)
                    .fold(f64::NEG_INFINITY, f64::max),
                source: "calibrated".to_string(),
                provisional_lsd_ceiling_db: provisional,
                overrides: Vec::new(),
            }
        }
    };
    let mut derived = Derived {
        image_safe: derive_image(&image_plans.safe_name, PROVISIONAL_IMAGE_SAFE),
        image_aggressive: derive_image(&image_plans.aggressive_name, PROVISIONAL_IMAGE_AGGRESSIVE),
        audio_safe: derive_audio(&audio_plans.safe_name, PROVISIONAL_AUDIO_SAFE),
        audio_aggressive: derive_audio(&audio_plans.aggressive_name, PROVISIONAL_AUDIO_AGGRESSIVE),
    };

    let mut properties = Vec::new();
    let qualified_present = cells.iter().any(|r| r.qualified);
    properties.push(Property {
        name: "qualified-cells-present".to_string(),
        cell: None,
        pass: qualified_present,
        detail: if qualified_present {
            format!(
                "{} qualified cells",
                cells.iter().filter(|r| r.qualified).count()
            )
        } else {
            "no cell reached n_min with every required class at class_min under the generator cap; every number stays provisional"
                .to_string()
        },
    });

    for (plan, derived_plan) in [
        (&image_plans.safe_name, &mut derived.image_safe),
        (&image_plans.aggressive_name, &mut derived.image_aggressive),
    ] {
        let profile = (derived_plan.psnr_floor_db, derived_plan.ssim_floor);
        let rows: Vec<&CellRow> = cells
            .iter()
            .filter(|r| &r.plan == plan && r.qualified)
            .collect();
        let mut failing_formats: Vec<String> = Vec::new();
        for r in &rows {
            let name = format!("{}/{}/{}/{}", r.plan, r.format, r.band, r.derived);
            let s = r.image.as_ref().unwrap();
            let covers = s.psnr_percentile_db >= profile.0 && s.ssim_percentile >= profile.1;
            properties.push(Property {
                name: "coverage".to_string(),
                cell: Some(name.clone()),
                pass: covers,
                detail: format!(
                    "percentile asset scores {:.3} dB and {:.3} against {:.1} dB and {:.3}",
                    s.psnr_percentile_db, s.ssim_percentile, profile.0, profile.1
                ),
            });
            let nt = r.next_tier.as_ref().unwrap();
            match image_fails(nt.psnr_median_db, nt.ssim_median, profile) {
                Some(true) => properties.push(Property {
                    name: "separation".to_string(),
                    cell: Some(name),
                    pass: true,
                    detail: format!(
                        "{} median {:.3} dB and {:.3} fails {:.1} dB and {:.3}",
                        nt.plan,
                        nt.psnr_median_db.unwrap(),
                        nt.ssim_median.unwrap(),
                        profile.0,
                        profile.1
                    ),
                }),
                Some(false) => {
                    if !failing_formats.contains(&r.format) {
                        failing_formats.push(r.format.clone());
                    }
                }
                None => properties.push(Property {
                    name: "separation".to_string(),
                    cell: Some(name),
                    pass: false,
                    detail: format!("{} has no scored assets in this cell", nt.plan),
                }),
            }
        }
        for format in failing_formats {
            let fmt_rows: Vec<&&CellRow> = rows.iter().filter(|r| r.format == format).collect();
            let fmt_number = (
                fmt_rows
                    .iter()
                    .map(|r| r.image.as_ref().unwrap().psnr_floor_db)
                    .fold(f64::INFINITY, f64::min),
                fmt_rows
                    .iter()
                    .map(|r| r.image.as_ref().unwrap().ssim_floor)
                    .fold(f64::INFINITY, f64::min),
            );
            let all_pass = fmt_rows.iter().all(|r| {
                let nt = r.next_tier.as_ref().unwrap();
                image_fails(nt.psnr_median_db, nt.ssim_median, fmt_number) == Some(true)
            });
            for r in &fmt_rows {
                let nt = r.next_tier.as_ref().unwrap();
                properties.push(Property {
                    name: "separation".to_string(),
                    cell: Some(format!("{}/{}/{}/{}", r.plan, r.format, r.band, r.derived)),
                    pass: all_pass,
                    detail: if all_pass {
                        format!(
                            "fails at the profile-wide number, so a per-format override {:.1} dB and {:.3} is emitted; {} median {:.3} dB and {:.3}",
                            fmt_number.0, fmt_number.1, nt.plan,
                            nt.psnr_median_db.unwrap_or(f64::NAN), nt.ssim_median.unwrap_or(f64::NAN)
                        )
                    } else {
                        format!(
                            "{} median {:.3} dB and {:.3} passes the profile-wide {:.1} dB and {:.3} and the per-format {:.1} dB and {:.3}; no override separates this format",
                            nt.plan,
                            nt.psnr_median_db.unwrap_or(f64::NAN), nt.ssim_median.unwrap_or(f64::NAN),
                            profile.0, profile.1, fmt_number.0, fmt_number.1
                        )
                    },
                });
            }
            if all_pass {
                derived_plan.overrides.push(FormatOverride {
                    format,
                    psnr_floor_db: Some(fmt_number.0),
                    ssim_floor: Some(fmt_number.1),
                    lsd_ceiling_db: None,
                });
            }
        }
    }

    for (plan, derived_plan) in [
        (&audio_plans.safe_name, &mut derived.audio_safe),
        (&audio_plans.aggressive_name, &mut derived.audio_aggressive),
    ] {
        let profile = derived_plan.lsd_ceiling_db;
        let rows: Vec<&CellRow> = cells
            .iter()
            .filter(|r| &r.plan == plan && r.qualified)
            .collect();
        let mut failing_formats: Vec<String> = Vec::new();
        for r in &rows {
            let name = format!("{}/{}/{}/{}", r.plan, r.format, r.band, r.derived);
            let s = r.audio.as_ref().unwrap();
            properties.push(Property {
                name: "coverage".to_string(),
                cell: Some(name.clone()),
                pass: s.lsd_percentile_db <= profile,
                detail: format!(
                    "percentile asset scores {:.3} dB against the {:.2} dB ceiling",
                    s.lsd_percentile_db, profile
                ),
            });
            let nt = r.next_tier.as_ref().unwrap();
            match nt.lsd_median_db {
                Some(m) if m > profile => properties.push(Property {
                    name: "separation".to_string(),
                    cell: Some(name),
                    pass: true,
                    detail: format!("{} median {m:.3} dB exceeds {profile:.2} dB", nt.plan),
                }),
                Some(_) => {
                    if !failing_formats.contains(&r.format) {
                        failing_formats.push(r.format.clone());
                    }
                }
                None => properties.push(Property {
                    name: "separation".to_string(),
                    cell: Some(name),
                    pass: false,
                    detail: format!("{} has no scored assets in this cell", nt.plan),
                }),
            }
        }
        for format in failing_formats {
            let fmt_rows: Vec<&&CellRow> = rows.iter().filter(|r| r.format == format).collect();
            let fmt_number = fmt_rows
                .iter()
                .map(|r| r.audio.as_ref().unwrap().lsd_ceiling_db)
                .fold(f64::NEG_INFINITY, f64::max);
            let all_pass = fmt_rows.iter().all(|r| {
                r.next_tier
                    .as_ref()
                    .unwrap()
                    .lsd_median_db
                    .is_some_and(|m| m > fmt_number)
            });
            for r in &fmt_rows {
                let nt = r.next_tier.as_ref().unwrap();
                properties.push(Property {
                    name: "separation".to_string(),
                    cell: Some(format!("{}/{}/{}/{}", r.plan, r.format, r.band, r.derived)),
                    pass: all_pass,
                    detail: if all_pass {
                        format!(
                            "fails at the profile-wide number, so a per-format override {fmt_number:.2} dB is emitted; {} median {:.3} dB",
                            nt.plan, nt.lsd_median_db.unwrap_or(f64::NAN)
                        )
                    } else {
                        format!(
                            "{} median {:.3} dB is within the profile-wide {profile:.2} dB and the per-format {fmt_number:.2} dB; no override separates this format",
                            nt.plan, nt.lsd_median_db.unwrap_or(f64::NAN)
                        )
                    },
                });
            }
            if all_pass {
                derived_plan.overrides.push(FormatOverride {
                    format,
                    psnr_floor_db: None,
                    ssim_floor: None,
                    lsd_ceiling_db: Some(fmt_number),
                });
            }
        }
    }

    let img_effective = |d: &DerivedImage, format: &str| -> (f64, f64) {
        d.overrides
            .iter()
            .find(|o| o.format == format)
            .map(|o| {
                (
                    o.psnr_floor_db.unwrap_or(d.psnr_floor_db),
                    o.ssim_floor.unwrap_or(d.ssim_floor),
                )
            })
            .unwrap_or((d.psnr_floor_db, d.ssim_floor))
    };
    let aud_effective = |d: &DerivedAudio, format: &str| -> f64 {
        d.overrides
            .iter()
            .find(|o| o.format == format)
            .and_then(|o| o.lsd_ceiling_db)
            .unwrap_or(d.lsd_ceiling_db)
    };
    let mut monotonic = true;
    let mut detail = Vec::new();
    for format in formats_for(Media::Image) {
        let s = img_effective(&derived.image_safe, format);
        let a = img_effective(&derived.image_aggressive, format);
        if a.0 > s.0 || a.1 > s.1 {
            monotonic = false;
            detail.push(format!(
                "{format}: aggressive {:.1} dB and {:.3} is stricter than safe {:.1} dB and {:.3}",
                a.0, a.1, s.0, s.1
            ));
        }
    }
    for format in ["wav", "flac"] {
        let s = aud_effective(&derived.audio_safe, format);
        let a = aud_effective(&derived.audio_aggressive, format);
        if a < s {
            monotonic = false;
            detail.push(format!(
                "{format}: aggressive {a:.2} dB is stricter than safe {s:.2} dB"
            ));
        }
    }
    properties.push(Property {
        name: "aggressive-never-stricter-than-safe".to_string(),
        cell: None,
        pass: monotonic,
        detail: if monotonic {
            "holds for every format".to_string()
        } else {
            detail.join("; ")
        },
    });

    let identity_ok = identity.iter().all(|r| r.identical);
    properties.push(Property {
        name: "metadata-byte-identity".to_string(),
        cell: None,
        pass: identity_ok,
        detail: if identity_ok {
            format!(
                "{} identity cells, every decoded stream identical",
                identity.len()
            )
        } else {
            identity
                .iter()
                .filter(|r| !r.identical)
                .map(|r| format!("{}/{}: {}", r.plan, r.format, r.failures.join(", ")))
                .collect::<Vec<_>>()
                .join("; ")
        },
    });
    let fingerprint = crate::encoder_fingerprint();
    properties.push(Property {
        name: "encoder-fingerprint".to_string(),
        cell: None,
        pass: true,
        detail: fingerprint.clone(),
    });
    properties.push(Property {
        name: "determinism".to_string(),
        cell: None,
        pass: false,
        detail: "not checked".to_string(),
    });

    let assets: Vec<AssetRecord> = results
        .iter()
        .map(|a| AssetRecord {
            doc_id: a.doc_id.clone(),
            sha256: a.sha256.clone(),
            container: a.container.clone(),
            band: a.band.clone(),
            derived: a.derived.clone(),
            eligibility: a.eligibility.clone(),
            reason: a.reason.clone(),
            documented_marks: a.documented_marks.clone(),
            generator: a.generator.clone(),
            content_class: a.content_class.clone(),
            ladder_step: a.ladder_step.clone(),
            excluded: a.excluded.clone(),
            plans: a.plans.clone(),
        })
        .collect();
    let fixture: Vec<FixtureRow> = scored
        .iter()
        .filter(|a| a.fixture)
        .flat_map(|a| {
            a.plans.iter().map(|p| FixtureRow {
                doc_id: a.doc_id.clone(),
                sha256: a.sha256.clone(),
                plan: p.plan.clone(),
                score: p.score.clone(),
            })
        })
        .collect();

    let metrics = c
        .metrics
        .iter()
        .map(|(k, v)| Param {
            key: k.clone(),
            value: toml_to_json(v),
        })
        .collect();
    let transforms = pkg
        .transforms
        .iter()
        .filter(|t| t.milestone == 2)
        .map(|t| TransformPin {
            id: t.id.clone(),
            params: t
                .params
                .iter()
                .map(|(k, v)| Param {
                    key: k.clone(),
                    value: toml_to_json(v),
                })
                .collect(),
        })
        .collect();

    let mut record = Record {
        schema_version: RECORD_SCHEMA_VERSION.to_string(),
        tool_version: crate::TOOL_VERSION.to_string(),
        policy_version: pkg.version.clone(),
        date: date.to_string(),
        corpus: Corpus {
            manifest: manifest_name.to_string(),
            manifest_sha256: manifest_sha256.to_string(),
            source_inventory,
            prepare,
            filters: filters.clone(),
            assets_listed: results.len(),
            assets_scored: scored.len(),
            assets_excluded: exclusions.len(),
            by_eligibility: by_eligibility.into_iter().collect(),
            exclusion_counts: exclusion_counts.into_iter().collect(),
            exclusions,
        },
        encoder_fingerprint: fingerprint,
        base_seed: format!("0x{:016x}", c.base_seed),
        percentile_image: c.percentile_image,
        percentile_audio: c.percentile_audio,
        n_min: c.n_min,
        class_min: c.class_min,
        psnr_step_db: c.psnr_step_db,
        ssim_step: c.ssim_step,
        lsd_step_db: c.lsd_step_db,
        separation_rule: SEPARATION_RULE.to_string(),
        metrics,
        transforms,
        cells,
        strata,
        ladder,
        identity,
        derived,
        properties,
        determinism: Determinism {
            checked: false,
            identical: false,
        },
        accepted: false,
        assets,
        fixture,
    };
    record.accepted = record.properties.iter().all(|p| p.pass);
    Ok(record)
}

/// Set the determinism result and recompute acceptance.
pub fn set_determinism(record: &mut Record, checked: bool, identical: bool) {
    record.determinism = Determinism { checked, identical };
    if let Some(p) = record
        .properties
        .iter_mut()
        .find(|p| p.name == "determinism")
    {
        p.pass = checked && identical;
        p.detail = match (checked, identical) {
            (true, true) => {
                "two passes over the manifest produced byte-identical records".to_string()
            }
            (true, false) => "the second pass produced a different record".to_string(),
            _ => "not checked; run without --single-pass to check".to_string(),
        };
    }
    record.accepted = record.properties.iter().all(|p| p.pass);
}

/// The record as committed: pretty JSON with a trailing newline. serde
/// serializes struct fields in declaration order, so the key order is fixed.
pub fn record_json(record: &Record) -> String {
    let mut s = serde_json::to_string_pretty(record).expect("record serializes");
    s.push('\n');
    s
}

/// Recompute the acceptance properties from a record's own rows and derived
/// numbers, ignoring the stored property list. The test suite uses this to
/// confirm the committed record is internally consistent.
pub fn recheck_properties(record: &Record) -> Vec<Property> {
    let mut out = Vec::new();
    let qualified: Vec<&CellRow> = record.cells.iter().filter(|r| r.qualified).collect();
    for r in &qualified {
        let name = format!("{}/{}/{}/{}", r.plan, r.format, r.band, r.derived);
        if let Some(s) = &r.image {
            let d = if r.plan == "image-safe" {
                &record.derived.image_safe
            } else {
                &record.derived.image_aggressive
            };
            let number = d
                .overrides
                .iter()
                .find(|o| o.format == r.format)
                .map(|o| {
                    (
                        o.psnr_floor_db.unwrap_or(d.psnr_floor_db),
                        o.ssim_floor.unwrap_or(d.ssim_floor),
                    )
                })
                .unwrap_or((d.psnr_floor_db, d.ssim_floor));
            out.push(Property {
                name: "coverage".to_string(),
                cell: Some(name.clone()),
                pass: s.psnr_percentile_db >= number.0 && s.ssim_percentile >= number.1,
                detail: String::new(),
            });
            out.push(Property {
                name: "separation".to_string(),
                cell: Some(name.clone()),
                pass: r
                    .next_tier
                    .as_ref()
                    .and_then(|nt| image_fails(nt.psnr_median_db, nt.ssim_median, number))
                    .unwrap_or(false),
                detail: String::new(),
            });
        }
        if let Some(s) = &r.audio {
            let d = if r.plan == "audio-safe" {
                &record.derived.audio_safe
            } else {
                &record.derived.audio_aggressive
            };
            let number = d
                .overrides
                .iter()
                .find(|o| o.format == r.format)
                .and_then(|o| o.lsd_ceiling_db)
                .unwrap_or(d.lsd_ceiling_db);
            out.push(Property {
                name: "coverage".to_string(),
                cell: Some(name.clone()),
                pass: s.lsd_percentile_db <= number,
                detail: String::new(),
            });
            out.push(Property {
                name: "separation".to_string(),
                cell: Some(name),
                pass: r
                    .next_tier
                    .as_ref()
                    .and_then(|nt| nt.lsd_median_db)
                    .is_some_and(|m| m > number),
                detail: String::new(),
            });
        }
    }
    let d = &record.derived;
    out.push(Property {
        name: "aggressive-never-stricter-than-safe".to_string(),
        cell: None,
        pass: d.image_aggressive.psnr_floor_db <= d.image_safe.psnr_floor_db
            && d.image_aggressive.ssim_floor <= d.image_safe.ssim_floor
            && d.audio_aggressive.lsd_ceiling_db >= d.audio_safe.lsd_ceiling_db,
        detail: String::new(),
    });
    out.push(Property {
        name: "metadata-byte-identity".to_string(),
        cell: None,
        pass: record.identity.iter().all(|r| r.identical),
        detail: String::new(),
    });
    out.push(Property {
        name: "encoder-fingerprint".to_string(),
        cell: None,
        pass: record.encoder_fingerprint == crate::encoder_fingerprint(),
        detail: String::new(),
    });
    out.push(Property {
        name: "determinism".to_string(),
        cell: None,
        pass: record.determinism.checked && record.determinism.identical,
        detail: String::new(),
    });
    out.push(Property {
        name: "qualified-cells-present".to_string(),
        cell: None,
        pass: !qualified.is_empty(),
        detail: String::new(),
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reason_classes_group_the_report_only_rows() {
        assert_eq!(reason_class("near-duplicate of d0"), "near-duplicate");
        assert_eq!(reason_class("near-blank: luma stddev 1.00"), "near-blank");
        assert_eq!(reason_class("native_rate not true"), "native_rate not true");
        assert_eq!(reason_class("derived=synth"), "derived=synth");
    }
}
