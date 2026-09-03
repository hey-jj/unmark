//! The owner's table, generated from the record. Cell rows only: no
//! per-asset row, no path, no content.

use super::cells::CellRow;
use super::evaluate::Score;
use super::record::{DerivedAudio, DerivedImage, Record};
use std::fmt::Write as _;

fn opt3(v: Option<f64>) -> String {
    v.map(|x| format!("{x:.3}"))
        .unwrap_or_else(|| "n/a".to_string())
}

fn image_rows(w: &mut String, r: &Record, rows: &[CellRow]) {
    let _ = writeln!(
        w,
        "| Plan | Format | Band | Derived | n | Bases | Pixel-identical lost | Gen. max share | Classes | Missing classes | Held | Status | P{} PSNR | P{} SSIM | Median PSNR | Median SSIM | Cell floor | Next tier median |",
        r.percentile_image, r.percentile_image
    );
    let _ = writeln!(
        w,
        "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|"
    );
    for c in rows {
        let s = c.image.as_ref();
        let nt = c
            .next_tier
            .as_ref()
            .map(|n| {
                format!(
                    "{}: {} dB, {}",
                    n.plan,
                    opt3(n.psnr_median_db),
                    opt3(n.ssim_median)
                )
            })
            .unwrap_or_else(|| "n/a".to_string());
        let _ = writeln!(
            w,
            "| {} | {} | {} | {} | {} | {} | {} | {:.2} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            c.plan,
            c.format,
            c.band,
            c.derived,
            c.count,
            c.distinct_bases,
            c.pixel_identical_collapsed,
            c.generator_max_share,
            classes_text(c),
            missing_text(c),
            c.held,
            c.status,
            opt3(s.map(|s| s.psnr_percentile_db)),
            opt3(s.map(|s| s.ssim_percentile)),
            opt3(s.map(|s| s.psnr_median_db)),
            opt3(s.map(|s| s.ssim_median)),
            s.map(|s| format!("{:.1} dB, {:.3}", s.psnr_floor_db, s.ssim_floor))
                .unwrap_or_else(|| "n/a".to_string()),
            nt
        );
    }
}

fn audio_rows(w: &mut String, r: &Record, rows: &[CellRow]) {
    let _ = writeln!(
        w,
        "| Plan | Format | Band | Derived | n | Bases | Pixel-identical lost | Gen. max share | Classes | Missing classes | Held | Status | P{} LSD | Median LSD | Max LSD | Cell ceiling | Next tier median |",
        r.percentile_audio
    );
    let _ = writeln!(
        w,
        "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|"
    );
    for c in rows {
        let s = c.audio.as_ref();
        let nt = c
            .next_tier
            .as_ref()
            .map(|n| format!("{}: {} dB", n.plan, opt3(n.lsd_median_db)))
            .unwrap_or_else(|| "n/a".to_string());
        let _ = writeln!(
            w,
            "| {} | {} | {} | {} | {} | {} | {} | {:.2} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            c.plan,
            c.format,
            c.band,
            c.derived,
            c.count,
            c.distinct_bases,
            c.pixel_identical_collapsed,
            c.generator_max_share,
            classes_text(c),
            missing_text(c),
            c.held,
            c.status,
            opt3(s.map(|s| s.lsd_percentile_db)),
            opt3(s.map(|s| s.lsd_median_db)),
            opt3(s.map(|s| s.lsd_max_db)),
            s.map(|s| format!("{:.2} dB", s.lsd_ceiling_db))
                .unwrap_or_else(|| "n/a".to_string()),
            nt
        );
    }
}

fn classes_text(c: &CellRow) -> String {
    if c.classes.is_empty() {
        return "none".to_string();
    }
    c.classes
        .iter()
        .map(|(k, n)| format!("{k} {n}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn missing_text(c: &CellRow) -> String {
    if c.missing_classes.is_empty() {
        "none".to_string()
    } else {
        c.missing_classes.join(", ")
    }
}

fn is_audio_row(c: &CellRow) -> bool {
    c.format == "wav" || c.format == "flac"
}

fn is_image_row(c: &CellRow) -> bool {
    !is_audio_row(c)
}

fn derived_image(name: &str, d: &DerivedImage) -> String {
    let ov = if d.overrides.is_empty() {
        "none".to_string()
    } else {
        d.overrides
            .iter()
            .map(|o| {
                format!(
                    "{}: {:.1} dB, {:.3}",
                    o.format,
                    o.psnr_floor_db.unwrap_or(d.psnr_floor_db),
                    o.ssim_floor.unwrap_or(d.ssim_floor)
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    };
    format!(
        "| {name} | PSNR {:.1} dB, SSIM {:.3} | PSNR {:.1} dB, SSIM {:.3} | {} | {ov} |",
        d.provisional_psnr_floor_db,
        d.provisional_ssim_floor,
        d.psnr_floor_db,
        d.ssim_floor,
        d.source
    )
}

fn derived_audio(name: &str, d: &DerivedAudio) -> String {
    let ov = if d.overrides.is_empty() {
        "none".to_string()
    } else {
        d.overrides
            .iter()
            .map(|o| {
                format!(
                    "{}: {:.2} dB",
                    o.format,
                    o.lsd_ceiling_db.unwrap_or(d.lsd_ceiling_db)
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    };
    format!(
        "| {name} | LSD {:.2} dB | LSD {:.2} dB | {} | {ov} |",
        d.provisional_lsd_ceiling_db, d.lsd_ceiling_db, d.source
    )
}

pub fn render_markdown(r: &Record) -> String {
    let mut w = String::new();
    let _ = writeln!(w, "# unmark calibration record");
    let _ = writeln!(w);
    let _ = writeln!(
        w,
        "Generated from `calibration.json` by the calibration harness. Edits here are overwritten. Cell rows only: per-asset scores live in the record, keyed by doc_id and sha256."
    );
    let _ = writeln!(w);
    let _ = writeln!(w, "- date: {}", r.date);
    let _ = writeln!(w, "- policy version: {}", r.policy_version);
    let _ = writeln!(
        w,
        "- corpus manifest: {} (sha256 {})",
        r.corpus.manifest, r.corpus.manifest_sha256
    );
    if let Some(src) = &r.corpus.source_inventory {
        let _ = writeln!(
            w,
            "- source inventory: {} (sha256 {})",
            src.name, src.sha256
        );
    }
    let _ = writeln!(
        w,
        "- assets: {} listed, {} scored, {} excluded",
        r.corpus.assets_listed, r.corpus.assets_scored, r.corpus.assets_excluded
    );
    let elig: Vec<String> = r
        .corpus
        .by_eligibility
        .iter()
        .map(|(k, n)| format!("{k} {n}"))
        .collect();
    let _ = writeln!(w, "- scored by eligibility: {}", elig.join(", "));
    let _ = writeln!(w, "- encoder fingerprint: {}", r.encoder_fingerprint);
    let _ = writeln!(w, "- base seed: {}", r.base_seed);
    let _ = writeln!(
        w,
        "- derivation: image floors at the {}th percentile rounded down to {} dB and {}, audio ceiling at the {}th percentile rounded up to {} dB, n_min {}, class_min {}, no generator over half a cell",
        r.percentile_image, r.psnr_step_db, r.ssim_step, r.percentile_audio, r.lsd_step_db, r.n_min, r.class_min
    );
    let _ = writeln!(w, "- separation: {}", r.separation_rule);
    if let Some(p) = &r.corpus.prepare {
        let _ = writeln!(
            w,
            "- prepare: exact decoded-pixel dedupe always on; perceptual-hash collapse {}; content-class defaults {}; ladder rules {}; report-only rules {}",
            if p.near_duplicate_bits == 0 {
                "off".to_string()
            } else {
                format!("on at {} bits", p.near_duplicate_bits)
            },
            if p.content_class_rules.is_empty() { "none".to_string() } else { p.content_class_rules.join(", ") },
            if p.ladder_rules.is_empty() { "none".to_string() } else { p.ladder_rules.join(", ") },
            if p.report_only_rules.is_empty() { "none".to_string() } else { p.report_only_rules.join(", ") }
        );
    }
    let pixel_lost: usize = r
        .cells
        .iter()
        .filter(|c| c.plan.ends_with("-safe"))
        .map(|c| c.pixel_identical_collapsed)
        .sum();
    let _ = writeln!(
        w,
        "- pixel-identical rows lost across the safe cells: {pixel_lost}"
    );
    let _ = writeln!(
        w,
        "- accepted: {}",
        if r.accepted {
            "yes"
        } else {
            "no, a property failed or nothing qualified"
        }
    );
    let _ = writeln!(w);

    let _ = writeln!(w, "## Derived numbers");
    let _ = writeln!(w);
    let _ = writeln!(
        w,
        "| Budget | Provisional | Calibrated | Source | Overrides |"
    );
    let _ = writeln!(w, "|---|---|---|---|---|");
    let _ = writeln!(w, "{}", derived_image("image-safe", &r.derived.image_safe));
    let _ = writeln!(
        w,
        "{}",
        derived_image("image-aggressive", &r.derived.image_aggressive)
    );
    let _ = writeln!(w, "{}", derived_audio("audio-safe", &r.derived.audio_safe));
    let _ = writeln!(
        w,
        "{}",
        derived_audio("audio-aggressive", &r.derived.audio_aggressive)
    );
    let _ = writeln!(w);

    let _ = writeln!(w, "## Image cells");
    let _ = writeln!(w);
    let rows: Vec<CellRow> = r
        .cells
        .iter()
        .filter(|c| is_image_row(c))
        .cloned()
        .collect();
    if rows.is_empty() {
        let _ = writeln!(w, "No image asset scored.");
    } else {
        image_rows(&mut w, r, &rows);
    }
    let _ = writeln!(w);
    let _ = writeln!(w, "## Audio cells");
    let _ = writeln!(w);
    let rows: Vec<CellRow> = r
        .cells
        .iter()
        .filter(|c| is_audio_row(c))
        .cloned()
        .collect();
    if rows.is_empty() {
        let _ = writeln!(w, "No audio asset scored.");
    } else {
        audio_rows(&mut w, r, &rows);
    }
    let _ = writeln!(w);

    let _ = writeln!(w, "## Report-only rows");
    let _ = writeln!(w);
    let _ = writeln!(
        w,
        "Controls, MCU-padded images, short clips, report-only groups by reason, and the documented-marks splits. Reported beside the cells and never used to set a number."
    );
    let _ = writeln!(w);
    let rows: Vec<CellRow> = r
        .strata
        .iter()
        .filter(|c| is_image_row(c))
        .cloned()
        .collect();
    if !rows.is_empty() {
        image_rows(&mut w, r, &rows);
        let _ = writeln!(w);
    }
    let rows: Vec<CellRow> = r
        .strata
        .iter()
        .filter(|c| is_audio_row(c))
        .cloned()
        .collect();
    if !rows.is_empty() {
        audio_rows(&mut w, r, &rows);
        let _ = writeln!(w);
    }
    if r.strata.is_empty() {
        let _ = writeln!(w, "None.");
        let _ = writeln!(w);
    }

    let _ = writeln!(w, "## Post-processed ladder");
    let _ = writeln!(w);
    let _ = writeln!(
        w,
        "Pass and refuse counts of the safe and aggressive plans over post-processed assets, per degradation step, against the budget in force. Report only; nothing here feeds a percentile."
    );
    let _ = writeln!(w);
    if r.ladder.is_empty() {
        let _ = writeln!(w, "None.");
    } else {
        let _ = writeln!(
            w,
            "| Plan | Format | Step | Scored | Pass | Refuse | Held | Errors | Median PSNR | Median SSIM | Median LSD |"
        );
        let _ = writeln!(w, "|---|---|---|---|---|---|---|---|---|---|---|");
        for l in &r.ladder {
            let _ = writeln!(
                w,
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                l.plan,
                l.format,
                l.step,
                l.scored,
                l.pass,
                l.refuse,
                l.held,
                l.errors,
                opt3(l.psnr_median_db),
                opt3(l.ssim_median),
                opt3(l.lsd_median_db)
            );
        }
    }
    let _ = writeln!(w);

    let _ = writeln!(w, "## Metadata identity");
    let _ = writeln!(w);
    let _ = writeln!(w, "| Plan | Format | n | Identical | Failures | Declined |");
    let _ = writeln!(w, "|---|---|---|---|---|---|");
    for i in &r.identity {
        let _ = writeln!(
            w,
            "| {} | {} | {} | {} | {} | {} |",
            i.plan,
            i.format,
            i.count,
            if i.identical { "yes" } else { "no" },
            i.failures.len(),
            i.declined
        );
    }
    let _ = writeln!(w);

    let _ = writeln!(w, "## Properties");
    let _ = writeln!(w);
    let _ = writeln!(w, "| Property | Cell | Pass | Detail |");
    let _ = writeln!(w, "|---|---|---|---|");
    for p in &r.properties {
        let _ = writeln!(
            w,
            "| {} | {} | {} | {} |",
            p.name,
            p.cell.as_deref().unwrap_or("all"),
            if p.pass { "yes" } else { "no" },
            p.detail
        );
    }
    let _ = writeln!(w);

    let _ = writeln!(w, "## Exclusions by reason");
    let _ = writeln!(w);
    if r.corpus.exclusion_counts.is_empty() {
        let _ = writeln!(w, "None.");
    } else {
        let _ = writeln!(w, "| Reason | Count |");
        let _ = writeln!(w, "|---|---|");
        for (reason, n) in &r.corpus.exclusion_counts {
            let _ = writeln!(w, "| {reason} | {n} |");
        }
    }
    let _ = writeln!(w);

    let _ = writeln!(w, "## Fixture pins");
    let _ = writeln!(w);
    let _ = writeln!(
        w,
        "The fixture subset is synthetic broadband, worst case: procedural patterns and tones with a noise bed, so its scores sit below what generated content scores and set no number."
    );
    let _ = writeln!(w);
    let scored = r
        .fixture
        .iter()
        .filter(|f| matches!(f.score, Score::Image { .. } | Score::Audio { .. }))
        .count();
    let held = r
        .fixture
        .iter()
        .filter(|f| matches!(f.score, Score::Held { .. }))
        .count();
    let _ = writeln!(
        w,
        "{} fixture rows pinned by `tests/calibration.rs` at three decimals: {scored} scored, {held} held, {} identity.",
        r.fixture.len(),
        r.fixture
            .iter()
            .filter(|f| matches!(f.score, Score::Identity { .. }))
            .count()
    );
    w
}
