//! Policy package loading, validation, and digest. The canonical package lives
//! in `policy/` and is embedded at build time. The transform catalog, the mark
//! classes, the profile stances, the guardrail prose, and the fidelity budgets
//! all live here rather than in Rust source.

use crate::budget::Budget;
use sha2::{Digest, Sha256};

pub const POLICY_TOML: &str = include_str!("../policy/policy.toml");

/// Embedded signature files, keyed by package-relative path. These are data the
/// detectors match against and are part of the digested package.
pub const SIGNATURES: &[(&str, &str)] = &[
    (
        "signatures/generator-strings.txt",
        include_str!("../policy/signatures/generator-strings.txt"),
    ),
    (
        "signatures/png-keywords.txt",
        include_str!("../policy/signatures/png-keywords.txt"),
    ),
    (
        "signatures/exif-software.txt",
        include_str!("../policy/signatures/exif-software.txt"),
    ),
    (
        "signatures/xmp-fields.txt",
        include_str!("../policy/signatures/xmp-fields.txt"),
    ),
    (
        "signatures/camera-makes.txt",
        include_str!("../policy/signatures/camera-makes.txt"),
    ),
];

#[derive(Clone, Debug)]
pub struct MarkClass {
    pub id: String,
    pub honesty: String,
    pub label: String,
    pub guard: String,
    pub judge: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Transform {
    pub id: String,
    pub name: String,
    pub order: i64,
    pub tier: String,
    pub fidelity: String,
    pub default: bool,
    /// The milestone the transform ships in. Metadata transforms are 1, the
    /// pixel and audio degrade transforms are 2.
    pub milestone: i64,
    /// Pinned parameters, in key order. A degrade transform's cell key in the
    /// calibration record includes these, so a change invalidates the record.
    pub params: Vec<(String, toml::Value)>,
    pub target: String,
    pub guard: String,
}

impl Transform {
    /// A parameter as a float, accepting an integer literal.
    pub fn param_f64(&self, key: &str) -> Option<f64> {
        self.params
            .iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, v)| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
    }

    pub fn param_i64(&self, key: &str) -> Option<i64> {
        self.params
            .iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, v)| v.as_integer())
    }

    pub fn param_str(&self, key: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, v)| v.as_str())
    }

    /// The parameters rendered `key=value` in key order, for the cell key and
    /// the snapshot.
    pub fn params_rendered(&self) -> String {
        self.params
            .iter()
            .map(|(k, v)| match v {
                toml::Value::String(s) => format!("{k}={s}"),
                other => format!("{k}={other}"),
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// The calibration table: where the record lives, what it was derived from,
/// and the pinned derivation rules and metric parameters.
#[derive(Clone, Debug)]
pub struct Calibration {
    pub record: String,
    pub status: String,
    pub record_sha256: String,
    pub corpus_manifest_sha256: String,
    pub date: String,
    pub base_seed: u64,
    pub percentile_image: u32,
    pub percentile_audio: u32,
    pub n_min: usize,
    pub psnr_step_db: f64,
    pub ssim_step: f64,
    pub lsd_step_db: f64,
    /// The `[calibration.metrics]` table in key order, echoed into the record.
    pub metrics: Vec<(String, toml::Value)>,
}

impl Calibration {
    pub fn metric_f64(&self, key: &str) -> Option<f64> {
        self.metrics
            .iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, v)| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
    }

    pub fn metric_usize(&self, key: &str) -> Option<usize> {
        self.metrics
            .iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, v)| v.as_integer())
            .and_then(|i| usize::try_from(i).ok())
    }
}

#[derive(Clone, Debug)]
pub struct Profile {
    pub name: String,
    pub media: String,
    pub tier: String,
    pub milestone: i64,
    pub feature: Option<String>,
    pub transforms: Vec<String>,
    pub optional_transforms: Vec<String>,
    pub budget: String,
    pub notes: String,
}

#[derive(Clone, Debug)]
pub struct Guard {
    pub id: String,
    pub name: String,
    pub guard: String,
}

#[derive(Clone, Debug)]
pub struct PolicyPackage {
    pub version: String,
    pub digest: String,
    pub supported_containers: Vec<String>,
    pub mark_classes: Vec<MarkClass>,
    pub transforms: Vec<Transform>,
    pub profiles: Vec<Profile>,
    pub guards: Vec<Guard>,
    pub allowlist: Vec<String>,
    /// Named budgets keyed by the `budget` string a profile references.
    pub budgets: Vec<(String, Budget)>,
    /// Per-format overrides, `(budget, format, budget values)`, from
    /// `[budget.<name>.override.<format>]`. Emitted by the calibration
    /// harness only where separation fails at the profile-wide number.
    pub overrides: Vec<(String, String, Budget)>,
    pub calibration: Calibration,
}

impl PolicyPackage {
    pub fn profile(&self, name: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.name == name)
    }

    pub fn mark_class(&self, id: &str) -> Option<&MarkClass> {
        self.mark_classes.iter().find(|m| m.id == id)
    }

    pub fn transform(&self, id: &str) -> Option<&Transform> {
        self.transforms.iter().find(|t| t.id == id)
    }

    pub fn budget_for(&self, name: &str) -> Budget {
        if name == "exact" {
            return Budget::Exact;
        }
        self.budgets
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, b)| b.clone())
            .unwrap_or(Budget::Exact)
    }

    /// The budget in force for a format: the per-format override when one
    /// exists, otherwise the profile-wide numbers.
    pub fn budget_for_format(&self, name: &str, format: &str) -> Budget {
        self.overrides
            .iter()
            .find(|(n, f, _)| n == name && f == format)
            .map(|(_, _, b)| b.clone())
            .unwrap_or_else(|| self.budget_for(name))
    }
}

/// Canonical serialization for the digest: this file with the digest value
/// emptied and CRLF folded to LF, then each signature file path-sorted, each
/// preceded by a NUL-delimited path header.
pub fn canonical_bytes() -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"policy.toml\0");
    let lf = POLICY_TOML.replace("\r\n", "\n");
    for line in lf.split_inclusive('\n') {
        if line.trim_start().starts_with("digest = ") {
            out.extend_from_slice(b"digest = \"\"\n");
        } else {
            out.extend_from_slice(line.as_bytes());
        }
    }
    let mut files: Vec<(&str, &str)> = SIGNATURES.to_vec();
    files.sort_by_key(|(p, _)| *p);
    for (path, content) in files {
        out.push(0);
        out.extend_from_slice(path.as_bytes());
        out.push(0);
        out.extend_from_slice(content.replace("\r\n", "\n").as_bytes());
    }
    out
}

pub fn compute_digest() -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonical_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn as_str(v: &toml::Value, what: &str) -> Result<String, String> {
    v.as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("{what} must be a string"))
}

fn str_array(v: &toml::Value, what: &str) -> Result<Vec<String>, String> {
    let arr = v
        .as_array()
        .ok_or_else(|| format!("{what} must be an array"))?;
    arr.iter()
        .map(|x| as_str(x, what))
        .collect::<Result<Vec<_>, _>>()
}

fn opt_str_array(t: &toml::value::Table, key: &str) -> Vec<String> {
    t.get(key)
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Parse the embedded package into typed rules. Fails closed on a malformed
/// package.
pub fn load() -> Result<PolicyPackage, String> {
    let value: toml::Value = toml::from_str(POLICY_TOML).map_err(|e| e.to_string())?;
    let root = value.as_table().ok_or("policy root must be a table")?;

    let policy = root
        .get("policy")
        .and_then(|v| v.as_table())
        .ok_or("missing [policy]")?;
    let version = as_str(policy.get("version").ok_or("missing version")?, "version")?;

    let semantics = root
        .get("semantics")
        .and_then(|v| v.as_table())
        .ok_or("missing [semantics]")?;
    let supported_containers = str_array(
        semantics
            .get("supported_containers")
            .ok_or("missing supported_containers")?,
        "supported_containers",
    )?;

    let mut mark_classes = Vec::new();
    if let Some(arr) = root.get("mark_class").and_then(|v| v.as_array()) {
        for m in arr {
            let t = m.as_table().ok_or("mark_class must be a table")?;
            mark_classes.push(MarkClass {
                id: as_str(t.get("id").ok_or("mark_class.id")?, "mark_class.id")?,
                honesty: as_str(t.get("honesty").ok_or("mark_class.honesty")?, "honesty")?,
                label: as_str(t.get("label").ok_or("mark_class.label")?, "label")?,
                guard: as_str(t.get("guard").ok_or("mark_class.guard")?, "guard")?,
                judge: t.get("judge").and_then(|v| v.as_str()).map(str::to_string),
            });
        }
    }

    let mut transforms = Vec::new();
    if let Some(arr) = root.get("transform").and_then(|v| v.as_array()) {
        for tr in arr {
            let t = tr.as_table().ok_or("transform must be a table")?;
            transforms.push(Transform {
                id: as_str(t.get("id").ok_or("transform.id")?, "transform.id")?,
                name: as_str(t.get("name").ok_or("transform.name")?, "name")?,
                order: t.get("order").and_then(|v| v.as_integer()).unwrap_or(0),
                tier: as_str(t.get("tier").ok_or("transform.tier")?, "tier")?,
                fidelity: as_str(t.get("fidelity").ok_or("transform.fidelity")?, "fidelity")?,
                default: t.get("default").and_then(|v| v.as_bool()).unwrap_or(false),
                milestone: t.get("milestone").and_then(|v| v.as_integer()).unwrap_or(1),
                params: t
                    .get("params")
                    .and_then(|v| v.as_table())
                    .map(|p| p.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                    .unwrap_or_default(),
                target: as_str(t.get("target").ok_or("transform.target")?, "target")?,
                guard: as_str(t.get("guard").ok_or("transform.guard")?, "guard")?,
            });
        }
    }
    transforms.sort_by_key(|t| t.order);

    let mut profiles = Vec::new();
    if let Some(tbl) = root.get("profile").and_then(|v| v.as_table()) {
        for (name, pv) in tbl {
            let p = pv.as_table().ok_or("profile must be a table")?;
            profiles.push(Profile {
                name: name.clone(),
                media: as_str(p.get("media").ok_or("profile.media")?, "media")?,
                tier: as_str(p.get("tier").ok_or("profile.tier")?, "tier")?,
                milestone: p.get("milestone").and_then(|v| v.as_integer()).unwrap_or(1),
                feature: p
                    .get("feature")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                transforms: opt_str_array(p, "transforms"),
                optional_transforms: opt_str_array(p, "optional_transforms"),
                budget: as_str(p.get("budget").ok_or("profile.budget")?, "budget")?,
                notes: p
                    .get("notes")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            });
        }
    }
    profiles.sort_by(|a, b| a.name.cmp(&b.name));

    let mut guards = Vec::new();
    if let Some(arr) = root.get("guard").and_then(|v| v.as_array()) {
        for g in arr {
            let t = g.as_table().ok_or("guard must be a table")?;
            guards.push(Guard {
                id: as_str(t.get("id").ok_or("guard.id")?, "guard.id")?,
                name: as_str(t.get("name").ok_or("guard.name")?, "name")?,
                guard: as_str(t.get("guard").ok_or("guard.guard")?, "guard")?,
            });
        }
    }

    let allowlist = root
        .get("allowlist")
        .and_then(|v| v.as_table())
        .and_then(|t| t.get("keep"))
        .map(|v| str_array(v, "allowlist.keep"))
        .transpose()?
        .unwrap_or_default();

    let mut budgets = Vec::new();
    let mut overrides = Vec::new();
    if let Some(tbl) = root.get("budget").and_then(|v| v.as_table()) {
        for (name, bv) in tbl {
            let b = bv.as_table().ok_or("budget must be a table")?;
            let kind = as_str(b.get("kind").ok_or("budget.kind")?, "kind")?;
            let budget = parse_budget(&kind, b, None)?;
            budgets.push((name.clone(), budget.clone()));
            if let Some(ov) = b.get("override").and_then(|v| v.as_table()) {
                for (format, fv) in ov {
                    let o = fv.as_table().ok_or("budget override must be a table")?;
                    // An override carries the signal numbers only; the
                    // geometry floors stay profile-wide.
                    overrides.push((
                        name.clone(),
                        format.clone(),
                        parse_budget(&kind, o, Some(&budget))?,
                    ));
                }
            }
        }
    }

    let calibration = load_calibration(root)?;

    Ok(PolicyPackage {
        version,
        digest: compute_digest(),
        supported_containers,
        mark_classes,
        transforms,
        profiles,
        guards,
        allowlist,
        budgets,
        overrides,
        calibration,
    })
}

/// Parse a budget table. With `base`, a missing key falls back to the base
/// budget's value, which is how an override carries only the signal numbers.
fn parse_budget(
    kind: &str,
    b: &toml::value::Table,
    base: Option<&Budget>,
) -> Result<Budget, String> {
    let f = |k: &str, fallback: f64| {
        b.get(k)
            .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
            .unwrap_or(fallback)
    };
    match kind {
        "image" => {
            let (p, s, r, c) = match base {
                Some(Budget::Image {
                    psnr_floor_db,
                    ssim_floor,
                    resample_ratio_min,
                    crop_area_min,
                }) => (
                    *psnr_floor_db,
                    *ssim_floor,
                    *resample_ratio_min,
                    *crop_area_min,
                ),
                _ => (0.0, 0.0, 0.0, 0.0),
            };
            Ok(Budget::Image {
                psnr_floor_db: f("psnr_floor_db", p),
                ssim_floor: f("ssim_floor", s),
                resample_ratio_min: f("resample_ratio_min", r),
                crop_area_min: f("crop_area_min", c),
            })
        }
        "audio" => {
            let (l, r, lo, hi) = match base {
                Some(Budget::Audio {
                    lsd_ceiling_db,
                    resample_ratio_min,
                    time_stretch_min,
                    time_stretch_max,
                }) => (
                    *lsd_ceiling_db,
                    *resample_ratio_min,
                    *time_stretch_min,
                    *time_stretch_max,
                ),
                _ => (0.0, 0.0, 0.0, 0.0),
            };
            Ok(Budget::Audio {
                lsd_ceiling_db: f("lsd_ceiling_db", l),
                resample_ratio_min: f("resample_ratio_min", r),
                time_stretch_min: f("time_stretch_min", lo),
                time_stretch_max: f("time_stretch_max", hi),
            })
        }
        other => Err(format!("unknown budget kind {other}")),
    }
}

fn load_calibration(root: &toml::value::Table) -> Result<Calibration, String> {
    let c = root
        .get("calibration")
        .and_then(|v| v.as_table())
        .ok_or("missing [calibration]")?;
    let s = |k: &str| -> Result<String, String> {
        as_str(c.get(k).ok_or(format!("missing calibration.{k}"))?, k)
    };
    let f = |k: &str| -> Result<f64, String> {
        c.get(k)
            .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
            .ok_or(format!("calibration.{k} must be a number"))
    };
    let i = |k: &str| -> Result<i64, String> {
        c.get(k)
            .and_then(|v| v.as_integer())
            .ok_or(format!("calibration.{k} must be an integer"))
    };
    let metrics = c
        .get("metrics")
        .and_then(|v| v.as_table())
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();
    Ok(Calibration {
        record: s("record")?,
        status: s("status")?,
        record_sha256: s("record_sha256")?,
        corpus_manifest_sha256: s("corpus_manifest_sha256")?,
        date: s("date")?,
        // TOML integers are signed 64-bit, so the seed is stored as the
        // two's-complement reading of a u64.
        base_seed: i("base_seed")? as u64,
        percentile_image: u32::try_from(i("percentile_image")?).map_err(|e| e.to_string())?,
        percentile_audio: u32::try_from(i("percentile_audio")?).map_err(|e| e.to_string())?,
        n_min: usize::try_from(i("n_min")?).map_err(|e| e.to_string())?,
        psnr_step_db: f("psnr_step_db")?,
        ssim_step: f("ssim_step")?,
        lsd_step_db: f("lsd_step_db")?,
        metrics,
    })
}
