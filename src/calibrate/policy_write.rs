//! Rewriting the policy package from a record: the derived numbers into the
//! `[budget.*]` tables, the per-format override tables, and the record's
//! identity into `[calibration]`. Comments and every other line are kept.

use super::record::Record;

const OVERRIDE_COMMENT: &str =
    "# per-format override, emitted because separation failed at the profile-wide number";

fn fmt_f(v: f64) -> String {
    let s = format!("{v}");
    if s.contains('.') {
        s
    } else {
        format!("{s}.0")
    }
}

fn set_key(lines: &mut [String], key: &str, value: &str) -> Result<(), String> {
    let mut found = false;
    for l in lines.iter_mut() {
        let t = l.trim_start();
        if t.starts_with(&format!("{key} =")) || t.starts_with(&format!("{key}=")) {
            let indent = &l[..l.len() - t.len()];
            *l = format!("{indent}{key} = {value}");
            found = true;
        }
    }
    if found {
        Ok(())
    } else {
        Err(format!("policy has no key {key} in the section"))
    }
}

pub fn rewrite_policy(
    policy: &str,
    record: &Record,
    record_sha256: &str,
) -> Result<String, String> {
    let mut sections: Vec<(String, Vec<String>)> = vec![(String::new(), Vec::new())];
    for line in policy.lines() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            sections.push((t.to_string(), vec![line.to_string()]));
        } else {
            sections.last_mut().unwrap().1.push(line.to_string());
        }
    }
    // Existing override sections are regenerated.
    sections.retain(|(h, _)| !(h.starts_with("[budget.") && h.contains(".override.")));

    let mut out: Vec<String> = Vec::new();
    for (header, lines) in sections.iter_mut() {
        match header.as_str() {
            "[budget.image-safe]" | "[budget.image-aggressive]" => {
                let d = if header == "[budget.image-safe]" {
                    &record.derived.image_safe
                } else {
                    &record.derived.image_aggressive
                };
                set_key(lines, "psnr_floor_db", &fmt_f(d.psnr_floor_db))?;
                set_key(lines, "ssim_floor", &fmt_f(d.ssim_floor))?;
                while lines.last().is_some_and(|l| l.trim().is_empty()) {
                    lines.pop();
                }
                out.extend(lines.iter().cloned());
                for o in &d.overrides {
                    out.push(String::new());
                    out.push(OVERRIDE_COMMENT.to_string());
                    out.push(format!(
                        "{}.override.{}]",
                        header.trim_end_matches(']'),
                        o.format
                    ));
                    out.push(format!(
                        "psnr_floor_db = {}",
                        fmt_f(o.psnr_floor_db.unwrap_or(d.psnr_floor_db))
                    ));
                    out.push(format!(
                        "ssim_floor = {}",
                        fmt_f(o.ssim_floor.unwrap_or(d.ssim_floor))
                    ));
                }
                out.push(String::new());
            }
            "[budget.audio-safe]" | "[budget.audio-aggressive]" => {
                let d = if header == "[budget.audio-safe]" {
                    &record.derived.audio_safe
                } else {
                    &record.derived.audio_aggressive
                };
                set_key(lines, "lsd_ceiling_db", &fmt_f(d.lsd_ceiling_db))?;
                while lines.last().is_some_and(|l| l.trim().is_empty()) {
                    lines.pop();
                }
                out.extend(lines.iter().cloned());
                for o in &d.overrides {
                    out.push(String::new());
                    out.push(OVERRIDE_COMMENT.to_string());
                    out.push(format!(
                        "{}.override.{}]",
                        header.trim_end_matches(']'),
                        o.format
                    ));
                    out.push(format!(
                        "lsd_ceiling_db = {}",
                        fmt_f(o.lsd_ceiling_db.unwrap_or(d.lsd_ceiling_db))
                    ));
                }
                out.push(String::new());
            }
            "[calibration]" => {
                let status = if record.accepted {
                    "calibrated"
                } else {
                    "provisional"
                };
                set_key(lines, "status", &format!("\"{status}\""))?;
                set_key(lines, "record_sha256", &format!("\"{record_sha256}\""))?;
                set_key(
                    lines,
                    "corpus_manifest_sha256",
                    &format!("\"{}\"", record.corpus.manifest_sha256),
                )?;
                set_key(lines, "date", &format!("\"{}\"", record.date))?;
                out.extend(lines.iter().cloned());
            }
            _ => out.extend(lines.iter().cloned()),
        }
    }
    let mut text = out.join("\n");
    if !text.ends_with('\n') {
        text.push('\n');
    }
    while text.contains("\n\n\n") {
        text = text.replace("\n\n\n", "\n\n");
    }
    Ok(text)
}
