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
    pub target: String,
    pub guard: String,
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
    if let Some(tbl) = root.get("budget").and_then(|v| v.as_table()) {
        for (name, bv) in tbl {
            let b = bv.as_table().ok_or("budget must be a table")?;
            let kind = as_str(b.get("kind").ok_or("budget.kind")?, "kind")?;
            let f = |k: &str| b.get(k).and_then(|v| v.as_float()).unwrap_or(0.0);
            let budget = match kind.as_str() {
                "image" => Budget::Image {
                    psnr_floor_db: f("psnr_floor_db"),
                    ssim_floor: f("ssim_floor"),
                    resample_ratio_min: f("resample_ratio_min"),
                    crop_area_min: f("crop_area_min"),
                },
                "audio" => Budget::Audio {
                    lsd_ceiling_db: f("lsd_ceiling_db"),
                    resample_ratio_min: f("resample_ratio_min"),
                    time_stretch_min: f("time_stretch_min"),
                    time_stretch_max: f("time_stretch_max"),
                },
                other => return Err(format!("unknown budget kind {other}")),
            };
            budgets.push((name.clone(), budget));
        }
    }

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
    })
}
