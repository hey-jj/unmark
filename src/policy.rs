//! Policy package loading, validation, and digest. The canonical package lives
//! in `policy/` and is embedded at build time. The mark classes, the transform
//! catalog, the held list, the guardrail prose, and the sanity floor all live
//! in the policy package.

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
    /// For a blind class: what the default run does to it and the cited
    /// survival figure.
    pub survives: Option<String>,
    pub citation: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Transform {
    pub id: String,
    pub name: String,
    pub order: i64,
    /// `metadata`, `pixel`, `audio`, or `text`.
    pub kind: String,
    /// The container names the transform runs on by default.
    pub applies: Vec<String>,
    pub target: String,
    pub guard: String,
    /// Pinned parameters, in key order.
    pub params: Vec<(String, toml::Value)>,
    pub strength: Option<String>,
    pub cited_effect: Option<String>,
    pub citation: Option<String>,
}

impl Transform {
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

    /// The parameters rendered `key=value` in key order.
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

    /// True for a transform that changes pixels or samples.
    pub fn is_degrade(&self) -> bool {
        self.kind == "pixel" || self.kind == "audio"
    }

    pub fn applies_to(&self, container: &str) -> bool {
        self.applies.iter().any(|c| c == container)
    }
}

/// A transform that does not run in this release and has no flag.
#[derive(Clone, Debug)]
pub struct Held {
    pub id: String,
    pub name: String,
    pub reason: String,
}

#[derive(Clone, Debug)]
pub struct Guard {
    pub id: String,
    pub name: String,
    pub guard: String,
}

/// The sanity floor: the one refusal in a run.
#[derive(Clone, Debug)]
pub struct Sanity {
    pub status: String,
    pub psnr_floor_db: f64,
    pub ssim_floor: f64,
}

#[derive(Clone, Debug)]
pub struct PolicyPackage {
    pub version: String,
    pub digest: String,
    pub supported_containers: Vec<String>,
    pub mark_classes: Vec<MarkClass>,
    pub transforms: Vec<Transform>,
    pub held: Vec<Held>,
    pub guards: Vec<Guard>,
    pub allowlist: Vec<String>,
    pub sanity: Sanity,
}

impl PolicyPackage {
    pub fn mark_class(&self, id: &str) -> Option<&MarkClass> {
        self.mark_classes.iter().find(|m| m.id == id)
    }

    pub fn transform(&self, id: &str) -> Option<&Transform> {
        self.transforms.iter().find(|t| t.id == id)
    }

    /// The default run for a container: every transform that applies, in
    /// canonical order.
    pub fn default_run(&self, container: &str) -> Vec<String> {
        self.transforms
            .iter()
            .filter(|t| t.applies_to(container))
            .map(|t| t.id.clone())
            .collect()
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

fn opt_str(t: &toml::value::Table, key: &str) -> Option<String> {
    t.get(key).and_then(|v| v.as_str()).map(str::to_string)
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
                survives: opt_str(t, "survives"),
                citation: opt_str(t, "citation"),
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
                kind: as_str(t.get("kind").ok_or("transform.kind")?, "kind")?,
                applies: str_array(t.get("applies").ok_or("transform.applies")?, "applies")?,
                target: as_str(t.get("target").ok_or("transform.target")?, "target")?,
                guard: as_str(t.get("guard").ok_or("transform.guard")?, "guard")?,
                params: t
                    .get("params")
                    .and_then(|v| v.as_table())
                    .map(|p| p.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                    .unwrap_or_default(),
                strength: opt_str(t, "strength"),
                cited_effect: opt_str(t, "cited_effect"),
                citation: opt_str(t, "citation"),
            });
        }
    }
    transforms.sort_by_key(|t| t.order);
    for t in &transforms {
        if t.is_degrade() && t.cited_effect.is_none() {
            return Err(format!(
                "degrade transform {} carries no cited effect; a transform without one is held, not run",
                t.id
            ));
        }
    }

    let mut held = Vec::new();
    if let Some(arr) = root.get("held").and_then(|v| v.as_array()) {
        for h in arr {
            let t = h.as_table().ok_or("held must be a table")?;
            held.push(Held {
                id: as_str(t.get("id").ok_or("held.id")?, "held.id")?,
                name: as_str(t.get("name").ok_or("held.name")?, "held.name")?,
                reason: as_str(t.get("reason").ok_or("held.reason")?, "held.reason")?,
            });
        }
    }

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

    let s = root
        .get("sanity")
        .and_then(|v| v.as_table())
        .ok_or("missing [sanity]")?;
    let f = |k: &str| -> Result<f64, String> {
        s.get(k)
            .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
            .ok_or(format!("sanity.{k} must be a number"))
    };
    let sanity = Sanity {
        status: as_str(s.get("status").ok_or("sanity.status")?, "sanity.status")?,
        psnr_floor_db: f("psnr_floor_db")?,
        ssim_floor: f("ssim_floor")?,
    };

    Ok(PolicyPackage {
        version,
        digest: compute_digest(),
        supported_containers,
        mark_classes,
        transforms,
        held,
        guards,
        allowlist,
        sanity,
    })
}
