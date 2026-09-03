//! Cells and derivation: nearest-rank percentiles, rounding, band merging,
//! the per-cell row, the split and ladder rows.

use super::evaluate::{round3, AssetResult, Score};
use super::manifest::Eligibility;
use crate::budget::Budget;
use crate::policy::PolicyPackage;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A per-metric sample: the score and the row key that breaks ties.
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    pub score: f64,
    pub key: String,
}

/// Nearest-rank percentile over samples sorted ascending by (score, key).
/// Rank is ceil(p / 100 * n), one-based; no interpolation.
pub fn nearest_rank(samples: &[Sample], percentile: u32) -> Option<Sample> {
    if samples.is_empty() {
        return None;
    }
    let mut v = samples.to_vec();
    v.sort_by(|a, b| {
        a.score
            .partial_cmp(&b.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.key.cmp(&b.key))
    });
    let n = v.len() as f64;
    let rank = ((percentile as f64 / 100.0) * n).ceil().max(1.0) as usize;
    Some(v[rank.min(v.len()) - 1].clone())
}

/// Round down to a permissive step for a floor.
pub fn floor_to_step(x: f64, step: f64) -> f64 {
    round3((x / step + 1e-9).floor() * step)
}

/// Round up to a permissive step for a ceiling.
pub fn ceil_to_step(x: f64, step: f64) -> f64 {
    round3((x / step - 1e-9).ceil() * step)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageStats {
    pub psnr_percentile_db: f64,
    pub psnr_median_db: f64,
    pub psnr_min_db: f64,
    pub ssim_percentile: f64,
    pub ssim_median: f64,
    pub ssim_min: f64,
    /// The percentile asset for each metric, by doc_id.
    pub psnr_percentile_doc_id: String,
    pub ssim_percentile_doc_id: String,
    /// The cell's own rounded numbers.
    pub psnr_floor_db: f64,
    pub ssim_floor: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioStats {
    pub lsd_percentile_db: f64,
    pub lsd_median_db: f64,
    pub lsd_max_db: f64,
    pub lsd_percentile_doc_id: String,
    pub lsd_ceiling_db: f64,
}

/// The next tier's medians on the same assets, for the separation property.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NextTier {
    pub plan: String,
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub psnr_median_db: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssim_median: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lsd_median_db: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CellRow {
    pub plan: String,
    pub format: String,
    pub band: String,
    /// `none` for native assets, `container` for a lossless derivation cell.
    pub derived: String,
    pub merged_from: Vec<String>,
    /// Scored assets.
    pub count: usize,
    pub held: usize,
    pub errors: usize,
    /// Distinct base assets among the scored ones; overlay variants collapse
    /// onto their base before counting.
    pub distinct_bases: usize,
    pub near_duplicates_collapsed: usize,
    pub generators: usize,
    /// The largest single generator's share of the scored assets.
    pub generator_max_share: f64,
    pub generator_cap_ok: bool,
    /// Scored assets per content class; `unclassified` counts toward n and
    /// toward no class.
    pub classes: Vec<(String, usize)>,
    /// Required classes under class_min.
    pub missing_classes: Vec<String>,
    /// Rows lost to the exact decoded-pixel dedupe.
    pub pixel_identical_collapsed: usize,
    /// `qualified`, `count-qualified` (n_min met, a required class short),
    /// `unqualified`, `generator-cap`, `held`, or `informational`.
    pub status: String,
    pub qualified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<ImageStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<AudioStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_tier: Option<NextTier>,
}

pub struct CellInput<'a> {
    pub plan: String,
    pub format: String,
    pub bands: Vec<String>,
    pub derived: String,
    pub assets: Vec<&'a AssetResult>,
    /// Near-duplicate rows that would have joined this cell, for the report.
    pub collapsed: usize,
    pub pixel_collapsed: usize,
}

pub fn plan_score<'a>(asset: &'a AssetResult, plan: &str) -> Option<&'a Score> {
    asset
        .plans
        .iter()
        .find(|p| p.plan == plan)
        .map(|p| &p.score)
}

#[allow(clippy::type_complexity)]
pub fn samples(
    assets: &[&AssetResult],
    plan: &str,
) -> (Vec<Sample>, Vec<Sample>, Vec<Sample>, usize, usize) {
    let (mut psnr, mut ssim, mut lsd) = (Vec::new(), Vec::new(), Vec::new());
    let (mut held, mut errors) = (0, 0);
    for a in assets {
        match plan_score(a, plan) {
            Some(Score::Image { psnr_db, ssim: s }) => {
                psnr.push(Sample {
                    score: *psnr_db,
                    key: a.doc_id.clone(),
                });
                ssim.push(Sample {
                    score: *s,
                    key: a.doc_id.clone(),
                });
            }
            Some(Score::Audio { lsd_db }) => lsd.push(Sample {
                score: *lsd_db,
                key: a.doc_id.clone(),
            }),
            Some(Score::Held { .. }) => held += 1,
            Some(Score::Error { .. }) | Some(Score::Identity { .. }) | None => errors += 1,
        }
    }
    (psnr, ssim, lsd, held, errors)
}

pub fn median(samples: &[Sample]) -> Option<f64> {
    nearest_rank(samples, 50).map(|s| s.score)
}

fn scored(a: &AssetResult, plan: &str) -> bool {
    matches!(
        plan_score(a, plan),
        Some(Score::Image { .. }) | Some(Score::Audio { .. })
    )
}

/// Build one cell row. `informational` rows set nothing and never qualify.
pub fn build_cell(
    input: &CellInput<'_>,
    pkg: &PolicyPackage,
    informational: bool,
    next_tier_plan: Option<&str>,
) -> CellRow {
    let c = &pkg.calibration;
    let (psnr, ssim, lsd, held, errors) = samples(&input.assets, &input.plan);
    let count = psnr.len().max(lsd.len());
    let is_audio = input.plan.starts_with("audio") || input.plan.starts_with("AU");
    let image = if !is_audio && !psnr.is_empty() {
        let pp = nearest_rank(&psnr, c.percentile_image).unwrap();
        let sp = nearest_rank(&ssim, c.percentile_image).unwrap();
        Some(ImageStats {
            psnr_percentile_db: pp.score,
            psnr_median_db: median(&psnr).unwrap(),
            psnr_min_db: psnr.iter().map(|s| s.score).fold(f64::INFINITY, f64::min),
            ssim_percentile: sp.score,
            ssim_median: median(&ssim).unwrap(),
            ssim_min: ssim.iter().map(|s| s.score).fold(f64::INFINITY, f64::min),
            psnr_percentile_doc_id: pp.key,
            ssim_percentile_doc_id: sp.key,
            psnr_floor_db: floor_to_step(pp.score, c.psnr_step_db),
            ssim_floor: floor_to_step(sp.score, c.ssim_step),
        })
    } else {
        None
    };
    let audio = if is_audio && !lsd.is_empty() {
        let lp = nearest_rank(&lsd, c.percentile_audio).unwrap();
        Some(AudioStats {
            lsd_percentile_db: lp.score,
            lsd_median_db: median(&lsd).unwrap(),
            lsd_max_db: lsd
                .iter()
                .map(|s| s.score)
                .fold(f64::NEG_INFINITY, f64::max),
            lsd_percentile_doc_id: lp.key,
            lsd_ceiling_db: ceil_to_step(lp.score, c.lsd_step_db),
        })
    } else {
        None
    };
    let next_tier = next_tier_plan.map(|np| {
        let (np_psnr, np_ssim, np_lsd, _, _) = samples(&input.assets, np);
        NextTier {
            plan: np.to_string(),
            count: np_psnr.len().max(np_lsd.len()),
            psnr_median_db: median(&np_psnr),
            ssim_median: median(&np_ssim),
            lsd_median_db: median(&np_lsd),
        }
    });
    // Generator shares over the scored assets.
    let mut gens: BTreeMap<&str, usize> = BTreeMap::new();
    for a in input.assets.iter().filter(|a| scored(a, &input.plan)) {
        *gens.entry(a.generator.as_str()).or_default() += 1;
    }
    let max_share = if count == 0 {
        0.0
    } else {
        round3(gens.values().copied().max().unwrap_or(0) as f64 / count as f64)
    };
    // A cell with one generator only is capped by definition; a cell with
    // no scored asset has nothing to cap.
    let cap_ok = count == 0 || max_share <= 0.5;
    // Content classes over the scored assets.
    let mut classes: BTreeMap<String, usize> = BTreeMap::new();
    for a in input.assets.iter().filter(|a| scored(a, &input.plan)) {
        let class = if a.content_class.is_empty() {
            "unclassified".to_string()
        } else {
            a.content_class.clone()
        };
        *classes.entry(class).or_default() += 1;
    }
    let media = if is_audio {
        crate::asset::Media::Audio
    } else {
        crate::asset::Media::Image
    };
    let missing: Vec<String> = super::manifest::required_classes(media)
        .iter()
        .filter(|cl| classes.get(**cl).copied().unwrap_or(0) < c.class_min)
        .map(|cl| cl.to_string())
        .collect();
    let qualified = !informational && count >= c.n_min && cap_ok && missing.is_empty();
    let status = if informational {
        "informational"
    } else if qualified {
        "qualified"
    } else if count >= c.n_min && !cap_ok {
        "generator-cap"
    } else if count >= c.n_min {
        "count-qualified"
    } else if count == 0 && held > 0 {
        "held"
    } else {
        "unqualified"
    };
    CellRow {
        plan: input.plan.clone(),
        format: input.format.clone(),
        band: input.bands.join("+"),
        derived: input.derived.clone(),
        merged_from: if input.bands.len() > 1 {
            input.bands.clone()
        } else {
            Vec::new()
        },
        count,
        held,
        errors,
        distinct_bases: count,
        near_duplicates_collapsed: input.collapsed,
        generators: gens.len(),
        generator_max_share: max_share,
        generator_cap_ok: cap_ok,
        classes: classes.into_iter().collect(),
        missing_classes: missing,
        pixel_identical_collapsed: input.pixel_collapsed,
        status: status.to_string(),
        qualified,
        image,
        audio,
        next_tier,
    }
}

/// Group a plan's assets of one format into bands, merging a short band
/// once with its adjacent band of larger count (ties go to the lower band).
/// A band that is still short after the merge is reported unqualified.
pub fn band_groups<'a>(
    assets: &[&'a AssetResult],
    plan: &str,
    bands: &[&str],
    n_min: usize,
) -> Vec<(Vec<String>, Vec<&'a AssetResult>)> {
    let mut groups: Vec<(Vec<String>, Vec<&AssetResult>)> = bands
        .iter()
        .map(|b| {
            (
                vec![b.to_string()],
                assets
                    .iter()
                    .copied()
                    .filter(|a| a.band == *b)
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    let count = |g: &[&AssetResult]| -> usize { g.iter().filter(|a| scored(a, plan)).count() };
    let mut merged = vec![false; groups.len()];
    let mut i = 0;
    while i < groups.len() {
        if merged[i] || count(&groups[i].1) >= n_min || groups[i].1.is_empty() {
            i += 1;
            continue;
        }
        let prev = if i > 0 && !merged[i - 1] {
            Some(i - 1)
        } else {
            None
        };
        let next = if i + 1 < groups.len() && !merged[i + 1] {
            Some(i + 1)
        } else {
            None
        };
        let pick = match (prev, next) {
            (Some(p), Some(n)) => {
                if count(&groups[n].1) > count(&groups[p].1) {
                    Some(n)
                } else {
                    Some(p)
                }
            }
            (Some(p), None) => Some(p),
            (None, Some(n)) => Some(n),
            (None, None) => None,
        };
        let Some(j) = pick else {
            i += 1;
            continue;
        };
        let (lo, hi) = if j < i { (j, i) } else { (i, j) };
        let (hi_names, hi_assets) = groups.remove(hi);
        groups[lo].0.extend(hi_names);
        groups[lo].1.extend(hi_assets);
        merged.remove(hi);
        merged[lo] = true;
        i = lo + 1;
    }
    groups
        .into_iter()
        .filter(|(_, assets)| !assets.is_empty())
        .collect()
}

/// One row of the round-4 ladder table: pass and refuse rates of a plan on
/// post-processed assets at one degradation step, against the budget in
/// force. Report only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LadderRow {
    pub plan: String,
    pub format: String,
    pub step: String,
    pub scored: usize,
    pub pass: usize,
    pub refuse: usize,
    pub held: usize,
    pub errors: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub psnr_median_db: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssim_median: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lsd_median_db: Option<f64>,
}

/// Whether a score passes the budget.
pub fn passes(score: &Score, budget: &Budget) -> Option<bool> {
    match (score, budget) {
        (
            Score::Image { psnr_db, ssim },
            Budget::Image {
                psnr_floor_db,
                ssim_floor,
                ..
            },
        ) => Some(psnr_db >= psnr_floor_db && ssim >= ssim_floor),
        (Score::Audio { lsd_db }, Budget::Audio { lsd_ceiling_db, .. }) => {
            Some(lsd_db <= lsd_ceiling_db)
        }
        _ => None,
    }
}

/// Build the ladder rows over the post-processed assets for the safe and
/// aggressive plans, per format and step.
pub fn ladder_rows(
    pkg: &PolicyPackage,
    assets: &[&AssetResult],
    plans: &[(String, String)],
) -> Vec<LadderRow> {
    let mut rows = Vec::new();
    let mut groups: BTreeMap<(String, String), Vec<&AssetResult>> = BTreeMap::new();
    for a in assets
        .iter()
        .filter(|a| a.eligibility == Eligibility::Ladder)
    {
        let step = if a.ladder_step.is_empty() {
            "unlabeled".to_string()
        } else {
            a.ladder_step.clone()
        };
        groups
            .entry((a.container.clone(), step))
            .or_default()
            .push(a);
    }
    for ((format, step), group) in groups {
        for (plan, budget_name) in plans {
            let budget = pkg.budget_for_format(budget_name, &format);
            let (mut pass, mut refuse, mut held, mut errors) = (0, 0, 0, 0);
            for a in &group {
                match plan_score(a, plan) {
                    Some(s @ (Score::Image { .. } | Score::Audio { .. })) => {
                        match passes(s, &budget) {
                            Some(true) => pass += 1,
                            Some(false) => refuse += 1,
                            None => errors += 1,
                        }
                    }
                    Some(Score::Held { .. }) => held += 1,
                    _ => errors += 1,
                }
            }
            if pass + refuse + held + errors == 0 {
                continue;
            }
            let (psnr, ssim, lsd, _, _) = samples(&group, plan);
            rows.push(LadderRow {
                plan: plan.clone(),
                format: format.clone(),
                step: step.clone(),
                scored: pass + refuse,
                pass,
                refuse,
                held,
                errors,
                psnr_median_db: median(&psnr),
                ssim_median: median(&ssim),
                lsd_median_db: median(&lsd),
            });
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(score: f64, key: &str) -> Sample {
        Sample {
            score,
            key: key.to_string(),
        }
    }

    #[test]
    fn nearest_rank_takes_the_ceiling_rank_without_interpolation() {
        let v: Vec<Sample> = (1..=40).map(|i| s(i as f64, &format!("p{i:02}"))).collect();
        assert_eq!(nearest_rank(&v, 5).unwrap().score, 2.0);
        assert_eq!(nearest_rank(&v, 95).unwrap().score, 38.0);
        assert_eq!(nearest_rank(&v, 50).unwrap().score, 20.0);
        let one = vec![s(7.0, "a")];
        assert_eq!(nearest_rank(&one, 5).unwrap().score, 7.0);
    }

    #[test]
    fn ties_break_on_the_row_key() {
        let v = vec![s(1.0, "b"), s(1.0, "a"), s(2.0, "c")];
        assert_eq!(nearest_rank(&v, 5).unwrap().key, "a");
    }

    #[test]
    fn rounding_moves_toward_the_permissive_side() {
        assert_eq!(floor_to_step(41.37, 0.5), 41.0);
        assert_eq!(floor_to_step(41.5, 0.5), 41.5);
        assert_eq!(floor_to_step(0.9874, 0.005), 0.985);
        assert_eq!(floor_to_step(0.985, 0.005), 0.985);
        assert_eq!(ceil_to_step(0.731, 0.05), 0.75);
        assert_eq!(ceil_to_step(0.75, 0.05), 0.75);
    }

    #[test]
    fn passes_reads_the_budget_direction() {
        let img = Budget::Image {
            psnr_floor_db: 38.0,
            ssim_floor: 0.98,
            resample_ratio_min: 0.75,
            crop_area_min: 1.0,
        };
        assert_eq!(
            passes(
                &Score::Image {
                    psnr_db: 38.0,
                    ssim: 0.98
                },
                &img
            ),
            Some(true)
        );
        assert_eq!(
            passes(
                &Score::Image {
                    psnr_db: 37.9,
                    ssim: 0.99
                },
                &img
            ),
            Some(false)
        );
        let aud = Budget::Audio {
            lsd_ceiling_db: 1.0,
            resample_ratio_min: 0.9,
            time_stretch_min: 1.0,
            time_stretch_max: 1.0,
        };
        assert_eq!(passes(&Score::Audio { lsd_db: 1.2 }, &aud), Some(false));
        assert_eq!(passes(&Score::Audio { lsd_db: 1.2 }, &img), None);
    }
}
