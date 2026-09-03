//! The corpus manifest: the row schema, the inventory CSV reader, annotation
//! merges, selection rules, hash verification, and the eligibility class
//! that decides whether a row can set a number or only be reported.

use super::MANIFEST_SCHEMA_VERSION;
use crate::asset::Format;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

fn derived_none() -> String {
    "none".to_string()
}

/// One corpus asset as the manifest describes it. Fields the harness filters
/// on are typed; every other inventory column rides along in `fields`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManifestAsset {
    /// Relative to the run's root, or absolute for a derived asset written
    /// outside the corpus.
    pub path: String,
    pub sha256: String,
    /// The physical container: `png`, `jpeg`, `webp`, `wav`, or `flac`.
    /// Anything else is carried and reported as not calibrated.
    #[serde(rename = "container", alias = "format")]
    pub container: String,
    #[serde(default)]
    pub doc_id: String,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub duration_s: Option<f64>,
    #[serde(default)]
    pub rate: Option<u32>,
    #[serde(default)]
    pub channels: Option<u32>,
    #[serde(default, alias = "generator_model")]
    pub generator: String,
    #[serde(default, rename = "content_class", alias = "class")]
    pub content_class: String,
    /// True when the asset was re-encoded, resized, or compressed after
    /// generation. Such a row only ever feeds the ladder table.
    #[serde(default)]
    pub post_processed: bool,
    /// The degradation step a post-processed row went through.
    #[serde(default)]
    pub ladder_step: String,
    /// A human-made control asset: scored in its own rows, never sets a number.
    #[serde(default)]
    pub control: bool,
    /// Part of the committed fixture subset: its per-plan scores land in the
    /// record's `fixture` section.
    #[serde(default)]
    pub fixture: bool,
    #[serde(default)]
    pub round: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// `none` for a corpus asset as generated, `container` for a lossless
    /// container derivation of one, `synth` for a builder-generated clip,
    /// `downscale` for a last-resort size derivation.
    #[serde(default = "derived_none")]
    pub derived: String,
    /// The doc_id of the source asset for a derived row.
    #[serde(default)]
    pub derived_from: String,
    /// Vendor-stated or observed marks, for example `synthid:vendor-stated`,
    /// `transformed`, `c2pa:observed`.
    #[serde(default)]
    pub documented_marks: Vec<String>,
    /// The doc_id of the base asset when this row is an overlay variant of it.
    #[serde(default)]
    pub near_duplicate_of: Option<String>,
    /// Audio only: true when the file is at the generator's native rate.
    #[serde(default)]
    pub native_rate: Option<bool>,
    /// Lossy images only: true when the generator emitted the container
    /// itself rather than a pipeline transcode.
    #[serde(default)]
    pub native_origin: Option<bool>,
    /// Lossy images only: the transcode quality when the pipeline wrote it.
    #[serde(default)]
    pub source_quality: Option<u32>,
    /// False when the asset is near-blank or near-silent. The prepare pass
    /// fills it from the measured variance when the manifest leaves it unset.
    #[serde(default)]
    pub variance_ok: Option<bool>,
    /// The size or duration band, filled by the prepare pass.
    #[serde(default)]
    pub band: Option<String>,
    #[serde(default)]
    pub fields: BTreeMap<String, String>,
}

impl ManifestAsset {
    /// The stable key for a row: its doc_id, or its path when the manifest
    /// has none.
    pub fn key(&self) -> String {
        if self.doc_id.is_empty() {
            self.path.clone()
        } else {
            self.doc_id.clone()
        }
    }

    /// A selectable field by name: the typed fields first, then the extra
    /// inventory columns.
    pub fn field(&self, name: &str) -> Option<String> {
        match name {
            "path" => Some(self.path.clone()),
            "sha256" => Some(self.sha256.clone()),
            "container" => Some(self.container.clone()),
            // The inventory's own format column when it has one, since it
            // is the logical family and not the container.
            "format" => self
                .fields
                .get("format")
                .cloned()
                .or_else(|| Some(self.container.clone())),
            "generator" | "generator_model" => Some(self.generator.clone()),
            "content_class" | "class" => Some(self.content_class.clone()),
            "round" => Some(self.round.clone()),
            "doc_id" => Some(self.doc_id.clone()),
            "derived" => Some(self.derived.clone()),
            "derived_from" => Some(self.derived_from.clone()),
            "ladder_step" => Some(self.ladder_step.clone()),
            "band" => self.band.clone(),
            "control" => Some(self.control.to_string()),
            "post_processed" => Some(self.post_processed.to_string()),
            "near_duplicate_of" => self.near_duplicate_of.clone(),
            "native_rate" => self.native_rate.map(|b| b.to_string()),
            "native_origin" => self.native_origin.map(|b| b.to_string()),
            "source_quality" => self.source_quality.map(|q| q.to_string()),
            "variance_ok" => self.variance_ok.map(|b| b.to_string()),
            "documented_marks" => Some(self.documented_marks.join(";")),
            other => self.fields.get(other).cloned(),
        }
    }

    /// Set a typed field from an annotation column; unknown names land in
    /// `fields`.
    pub fn set_field(&mut self, name: &str, value: &str) {
        let v = value.trim();
        let flag = |v: &str| matches!(v.to_ascii_lowercase().as_str(), "true" | "1" | "yes");
        let opt_flag = |v: &str| {
            if v.is_empty() {
                None
            } else {
                Some(flag(v))
            }
        };
        match name {
            "container" | "format" => self.container = v.to_string(),
            "generator" | "generator_model" => self.generator = v.to_string(),
            "content_class" | "class" => self.content_class = v.to_string(),
            "round" => self.round = v.to_string(),
            "derived" => {
                self.derived = if v.is_empty() {
                    derived_none()
                } else {
                    v.to_string()
                }
            }
            "derived_from" => self.derived_from = v.to_string(),
            "ladder_step" => self.ladder_step = v.to_string(),
            "control" => self.control = flag(v),
            "post_processed" => self.post_processed = flag(v),
            "near_duplicate_of" => {
                self.near_duplicate_of = if v.is_empty() {
                    None
                } else {
                    Some(v.to_string())
                }
            }
            "native_rate" => self.native_rate = opt_flag(v),
            "native_origin" => self.native_origin = opt_flag(v),
            "variance_ok" => self.variance_ok = opt_flag(v),
            "source_quality" => self.source_quality = v.parse().ok(),
            "documented_marks" => {
                for m in v.split([';', '|']) {
                    let m = m.trim();
                    if !m.is_empty() && !self.documented_marks.iter().any(|x| x == m) {
                        self.documented_marks.push(m.to_string());
                    }
                }
            }
            "tags" => {
                for t in v.split([';', '|']) {
                    let t = t.trim();
                    if !t.is_empty() && !self.tags.iter().any(|x| x == t) {
                        self.tags.push(t.to_string());
                    }
                }
            }
            other => {
                self.fields.insert(other.to_string(), v.to_string());
            }
        }
    }

    /// The documented-marks split key: the sorted marks joined, or
    /// `unmarked`.
    pub fn marks_key(&self) -> String {
        if self.documented_marks.is_empty() {
            return "unmarked".to_string();
        }
        let mut m = self.documented_marks.clone();
        m.sort();
        m.dedup();
        m.join("+")
    }
}

/// The source inventory a prepared manifest was built from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SourceInventory {
    pub name: String,
    pub sha256: String,
}

/// How the prepare pass shaped the manifest, carried into the record.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PrepareInfo {
    /// The perceptual-hash collapse distance; zero means off.
    pub near_duplicate_bits: u32,
    /// `media=class` defaults applied to unclassified rows.
    pub content_class_rules: Vec<String>,
    pub ladder_rules: Vec<String>,
    pub report_only_rules: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_inventory: Option<SourceInventory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prepare: Option<PrepareInfo>,
    pub assets: Vec<ManifestAsset>,
}

/// The content classes a medium needs before a cell may set a number.
pub fn required_classes(media: crate::asset::Media) -> &'static [&'static str] {
    match media {
        crate::asset::Media::Image => &[
            "photographic",
            "flat-illustration",
            "dense-texture",
            "text-ui",
        ],
        _ => &["music", "speech", "ambient", "tonal-synthetic"],
    }
}

/// Manifest-driven selection. An empty include list admits every round or
/// tag; the exclude lists always apply. `include` and `exclude` hold generic
/// `field=value` rules over any manifest field or inventory column; for a
/// field named in `include`, an asset must match one of that field's values.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Filters {
    pub include_rounds: Vec<String>,
    pub exclude_rounds: Vec<String>,
    pub include_tags: Vec<String>,
    pub exclude_tags: Vec<String>,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

pub fn split_rule(rule: &str) -> Result<(&str, &str), String> {
    rule.split_once('=')
        .map(|(k, v)| (k.trim(), v.trim()))
        .filter(|(k, _)| !k.is_empty())
        .ok_or_else(|| format!("selection rule {rule:?} is not field=value"))
}

impl Filters {
    /// Reject a malformed rule before any file is read.
    pub fn validate(&self) -> Result<(), String> {
        for r in self.include.iter().chain(&self.exclude) {
            split_rule(r)?;
        }
        Ok(())
    }

    /// The exclusion reason, or None when the asset is admitted. A
    /// post-processed row is admitted here; eligibility routes it to the
    /// ladder table.
    pub fn exclusion(&self, a: &ManifestAsset) -> Option<String> {
        let mut included_fields: Vec<&str> = Vec::new();
        for r in &self.include {
            let Ok((k, _)) = split_rule(r) else { continue };
            if !included_fields.contains(&k) {
                included_fields.push(k);
            }
        }
        for k in included_fields {
            let value = a.field(k).unwrap_or_default();
            let matched = self.include.iter().any(|r| {
                split_rule(r)
                    .map(|(rk, rv)| rk == k && rv == value)
                    .unwrap_or(false)
            });
            if !matched {
                return Some(format!("{k} {value:?} not included"));
            }
        }
        for r in &self.exclude {
            let Ok((k, v)) = split_rule(r) else { continue };
            if a.field(k).as_deref() == Some(v) {
                return Some(format!("{k} {v:?} excluded"));
            }
        }
        if !self.include_rounds.is_empty() && !self.include_rounds.contains(&a.round) {
            return Some(format!("round {} not included", a.round));
        }
        if self.exclude_rounds.contains(&a.round) {
            return Some(format!("round {} excluded", a.round));
        }
        if !self.include_tags.is_empty() && !a.tags.iter().any(|t| self.include_tags.contains(t)) {
            return Some("no included tag".to_string());
        }
        if let Some(t) = a.tags.iter().find(|t| self.exclude_tags.contains(t)) {
            return Some(format!("tag {t} excluded"));
        }
        None
    }
}

// --- eligibility -------------------------------------------------------------------

/// What a scored row may do in the record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Eligibility {
    /// A native first-generation asset: sets numbers in its native cell.
    Floor,
    /// A lossless container derivation: sets numbers in a derived cell that
    /// shares its source's percentiles by construction.
    Container,
    /// Scored and reported, never counted toward n_min.
    ReportOnly,
    /// A post-processed asset: pass and refuse rates in the ladder table.
    Ladder,
    /// A human-made control asset.
    Control,
}

impl Eligibility {
    pub fn as_str(&self) -> &'static str {
        match self {
            Eligibility::Floor => "floor",
            Eligibility::Container => "container",
            Eligibility::ReportOnly => "report-only",
            Eligibility::Ladder => "ladder",
            Eligibility::Control => "control",
        }
    }

    /// Rows that may set a number.
    pub fn sets_numbers(&self) -> bool {
        matches!(self, Eligibility::Floor | Eligibility::Container)
    }
}

/// The eligibility of a row and the reason when it only reports. The rules
/// are the corpus ruling's filter fields: `derived` in {none, container},
/// `near_duplicate_of` null, `native_rate` true for audio, `source_quality`
/// at or above 90 or a native origin for a lossy image, `variance_ok` true.
pub fn eligibility(a: &ManifestAsset) -> (Eligibility, Option<String>) {
    if a.control {
        return (Eligibility::Control, None);
    }
    if a.fields.get("identity_only").map(String::as_str) == Some("true") {
        return (
            Eligibility::ReportOnly,
            Some("identity-only container".to_string()),
        );
    }
    if let Some(rule) = a.fields.get("report_only") {
        return (Eligibility::ReportOnly, Some(format!("rule {rule}")));
    }
    if a.post_processed {
        return (Eligibility::Ladder, None);
    }
    let derived = match a.derived.as_str() {
        "none" | "" => Eligibility::Floor,
        "container" => Eligibility::Container,
        other => return (Eligibility::ReportOnly, Some(format!("derived={other}"))),
    };
    if let Some(base) = &a.near_duplicate_of {
        return (
            Eligibility::ReportOnly,
            Some(format!("near-duplicate of {base}")),
        );
    }
    if a.variance_ok == Some(false) {
        return (Eligibility::ReportOnly, Some("variance".to_string()));
    }
    let is_audio = matches!(a.container.as_str(), "wav" | "flac");
    if is_audio && a.native_rate != Some(true) {
        return (
            Eligibility::ReportOnly,
            Some("native_rate not true".to_string()),
        );
    }
    let lossy = a.container == "jpeg"
        || (a.container == "webp"
            && a.fields.get("webp_lossy").map(String::as_str) == Some("true"));
    if lossy && a.native_origin != Some(true) && a.source_quality.is_none_or(|q| q < 90) {
        return (
            Eligibility::ReportOnly,
            Some("source quality unknown or under 90".to_string()),
        );
    }
    (derived, None)
}

// --- parsing --------------------------------------------------------------------------

pub fn parse_manifest(text: &str) -> Result<Manifest, String> {
    let m: Manifest = serde_json::from_str(text).map_err(|e| format!("manifest: {e}"))?;
    if m.schema_version != MANIFEST_SCHEMA_VERSION {
        return Err(format!(
            "manifest schema {} is not {MANIFEST_SCHEMA_VERSION}",
            m.schema_version
        ));
    }
    Ok(m)
}

/// The container a cell name denotes.
pub fn manifest_format(name: &str) -> Option<Format> {
    match name {
        "png" => Some(Format::Png),
        "jpeg" | "jpg" => Some(Format::Jpeg),
        "webp" => Some(Format::WebP),
        "wav" => Some(Format::RiffWav),
        "flac" => Some(Format::Flac),
        _ => None,
    }
}

/// Containers the harness cannot decode but can rewrite: they serve the
/// metadata byte-identity cells only, compared on the container's signal
/// stream.
pub fn identity_only_format(name: &str) -> Option<Format> {
    match name {
        "mp4" | "m4a" | "mov" | "isobmff" => Some(Format::Isobmff),
        "mp3" => Some(Format::Mp3),
        _ => None,
    }
}

/// The cell format name for a container.
pub fn format_name(f: Format) -> &'static str {
    match f {
        Format::RiffWav => "wav",
        other => other.as_str(),
    }
}

/// Map an inventory's container name onto the cell format names. Unknown
/// names pass through lowercased and are excluded later as not calibrated.
pub fn normalize_format(name: &str) -> String {
    let n = name.trim().to_ascii_lowercase();
    match n.as_str() {
        "png" | "screenshot-png" => "png".to_string(),
        "jpg" | "jpeg" => "jpeg".to_string(),
        "webp" => "webp".to_string(),
        "wav" | "riff-wav" => "wav".to_string(),
        "flac" => "flac".to_string(),
        _ => n,
    }
}

/// The container for an inventory row: the first of `physical_variant` and
/// the path's extension that names a calibrated container. When neither does, the inventory's own word stands (the variant when it has one,
/// else the extension, else the format column), so the row is reported as
/// not calibrated under that name. The magic-byte sniff at read time still
/// has the last word.
pub fn derive_format(variant: &str, format: &str, path: &str) -> String {
    let ext = std::path::Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_default();
    // The format column is the logical doctype family, never the container,
    // so it is not a candidate for a calibrated name.
    for candidate in [variant, &ext] {
        let n = normalize_format(candidate);
        if manifest_format(&n).is_some() {
            return n;
        }
    }
    if !variant.trim().is_empty() {
        normalize_format(variant)
    } else if !ext.is_empty() {
        normalize_format(&ext)
    } else {
        normalize_format(format)
    }
}

/// A small RFC 4180 reader: quoted fields, doubled quotes, CRLF or LF.
pub fn parse_csv(text: &str) -> Result<Vec<Vec<String>>, String> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' => {
                    if chars.peek() == Some(&'"') {
                        field.push('"');
                        chars.next();
                    } else {
                        quoted = false;
                    }
                }
                _ => field.push(c),
            }
            continue;
        }
        match c {
            '"' if field.is_empty() => quoted = true,
            ',' => row.push(std::mem::take(&mut field)),
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            _ => field.push(c),
        }
    }
    if quoted {
        return Err("csv: unterminated quoted field".to_string());
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    Ok(rows)
}

/// Read an inventory CSV as a manifest. Required columns: `path`, `sha256`,
/// and `format`. The container comes from `derive_format`; `round`,
/// `generator_model`, and `archetype` map to the typed fields; every column
/// is also kept verbatim in `fields`.
pub fn parse_manifest_csv(text: &str) -> Result<Manifest, String> {
    let rows = parse_csv(text)?;
    let mut it = rows.into_iter();
    let header = it.next().ok_or("csv: empty inventory")?;
    let col = |name: &str| header.iter().position(|h| h.trim() == name);
    let path_i = col("path").ok_or("csv: no path column")?;
    let sha_i = col("sha256").ok_or("csv: no sha256 column")?;
    let format_i = col("format").ok_or("csv: no format column")?;
    let mut assets = Vec::new();
    for (n, row) in it.enumerate() {
        if row.iter().all(|f| f.trim().is_empty()) {
            continue;
        }
        if row.len() != header.len() {
            return Err(format!(
                "csv: row {} has {} fields, header has {}",
                n + 2,
                row.len(),
                header.len()
            ));
        }
        let get = |name: &str| {
            col(name)
                .map(|i| row[i].trim().to_string())
                .unwrap_or_default()
        };
        let mut fields = BTreeMap::new();
        for (h, v) in header.iter().zip(&row) {
            fields.insert(h.trim().to_string(), v.trim().to_string());
        }
        let path = row[path_i].trim().to_string();
        let container = derive_format(&get("physical_variant"), row[format_i].trim(), &path);
        assets.push(ManifestAsset {
            path,
            sha256: row[sha_i].trim().to_ascii_lowercase(),
            container,
            doc_id: get("doc_id"),
            width: None,
            height: None,
            duration_s: None,
            rate: None,
            channels: None,
            generator: get("generator_model"),
            // The archetype is an inventory code, kept in `fields`; the
            // content class is assigned by the prepare rules or an
            // annotation, never read from pixels.
            content_class: String::new(),
            post_processed: matches!(get("post_processed").as_str(), "true" | "1" | "yes"),
            ladder_step: get("ladder_step"),
            control: matches!(get("control").as_str(), "true" | "1" | "yes"),
            fixture: false,
            round: get("round"),
            tags: Vec::new(),
            derived: derived_none(),
            derived_from: String::new(),
            documented_marks: Vec::new(),
            near_duplicate_of: None,
            native_rate: None,
            native_origin: None,
            source_quality: None,
            variance_ok: None,
            band: None,
            fields,
        });
    }
    Ok(Manifest {
        schema_version: MANIFEST_SCHEMA_VERSION.to_string(),
        source_inventory: None,
        prepare: None,
        assets,
    })
}

/// Merge an annotation CSV into the manifest by `doc_id`: every column other
/// than doc_id sets the named field on the matching row. Rows the manifest
/// does not carry are ignored and counted.
pub fn annotate(manifest: &mut Manifest, text: &str) -> Result<usize, String> {
    let rows = parse_csv(text)?;
    let mut it = rows.into_iter();
    let header = it.next().ok_or("annotation csv: empty")?;
    let id_i = header
        .iter()
        .position(|h| h.trim() == "doc_id")
        .ok_or("annotation csv: no doc_id column")?;
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    for (i, a) in manifest.assets.iter().enumerate() {
        if !a.doc_id.is_empty() {
            index.insert(a.doc_id.clone(), i);
        }
    }
    let mut unmatched = 0;
    for row in it {
        if row.iter().all(|f| f.trim().is_empty()) {
            continue;
        }
        let Some(&i) = index.get(row[id_i].trim()) else {
            unmatched += 1;
            continue;
        };
        for (h, v) in header.iter().zip(&row) {
            let name = h.trim();
            if name == "doc_id" || name.is_empty() {
                continue;
            }
            manifest.assets[i].set_field(name, v);
        }
    }
    Ok(unmatched)
}

/// A loaded manifest with its identity: the file's sha256 and basename.
pub struct LoadedManifest {
    pub manifest: Manifest,
    pub sha256: String,
    pub name: String,
}

/// Load a JSON or CSV manifest by extension. The digest is over the file's
/// bytes as read, so it is the corpus identity whichever form was given.
pub fn load_manifest(path: &std::path::Path) -> Result<LoadedManifest, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let text = std::str::from_utf8(&bytes).map_err(|e| format!("manifest utf-8: {e}"))?;
    let is_csv = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("csv"));
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let sha256 = sha256_hex(&bytes);
    let mut manifest = if is_csv {
        parse_manifest_csv(text)?
    } else {
        parse_manifest(text)?
    };
    if is_csv {
        manifest.source_inventory = Some(SourceInventory {
            name: name.clone(),
            sha256: sha256.clone(),
        });
    }
    Ok(LoadedManifest {
        manifest,
        sha256,
        name,
    })
}

/// Resolve a row's path against the root: absolute paths stand alone.
pub fn resolve_path(root: &std::path::Path, a: &ManifestAsset) -> std::path::PathBuf {
    let p = std::path::Path::new(&a.path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        root.join(p)
    }
}

/// Read an admitted asset and check its hash against the manifest. A
/// mismatch or an unreadable file is a run error, never a silent exclusion.
pub fn verify_sha256(root: &std::path::Path, a: &ManifestAsset) -> Result<Vec<u8>, String> {
    let full = resolve_path(root, a);
    let bytes = std::fs::read(&full).map_err(|e| format!("{}: {e}", full.display()))?;
    let hash = sha256_hex(&bytes);
    if hash != a.sha256.to_ascii_lowercase() {
        return Err(format!(
            "sha256 mismatch for {}: manifest says {}, file is {hash}",
            a.key(),
            a.sha256
        ));
    }
    Ok(bytes)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&Sha256::digest(bytes));
    out
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Exclusion {
    pub doc_id: String,
    pub reason: String,
}

/// The selection a filter set makes over a manifest, before any measurement.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Selection {
    pub manifest: String,
    pub manifest_sha256: String,
    pub filters: Filters,
    pub listed: usize,
    pub admitted: usize,
    pub excluded: usize,
    /// Admitted counts keyed `container/round`.
    pub admitted_by_format_round: Vec<(String, usize)>,
    pub admitted_by_format: Vec<(String, usize)>,
    /// Admitted assets whose container is not one the harness calibrates.
    pub admitted_uncalibrated: Vec<(String, usize)>,
    pub exclusions: Vec<Exclusion>,
}

/// Apply the filters and, when `verify` is set, check every admitted file's
/// sha256 against the manifest, failing on the first mismatch or unreadable
/// file. Nothing is measured.
pub fn select(
    loaded: &LoadedManifest,
    root: &std::path::Path,
    filters: &Filters,
    verify: bool,
) -> Result<Selection, String> {
    filters.validate()?;
    let mut by_fr: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_f: BTreeMap<String, usize> = BTreeMap::new();
    let mut uncal: BTreeMap<String, usize> = BTreeMap::new();
    let mut exclusions = Vec::new();
    for a in &loaded.manifest.assets {
        if let Some(reason) = filters.exclusion(a) {
            exclusions.push(Exclusion {
                doc_id: a.key(),
                reason,
            });
            continue;
        }
        if verify {
            verify_sha256(root, a)?;
        }
        *by_fr
            .entry(format!("{}/{}", a.container, a.round))
            .or_default() += 1;
        *by_f.entry(a.container.clone()).or_default() += 1;
        if manifest_format(&a.container).is_none() {
            *uncal.entry(a.container.clone()).or_default() += 1;
        }
    }
    let listed = loaded.manifest.assets.len();
    Ok(Selection {
        manifest: loaded.name.clone(),
        manifest_sha256: loaded.sha256.clone(),
        filters: filters.clone(),
        listed,
        admitted: listed - exclusions.len(),
        excluded: exclusions.len(),
        admitted_by_format_round: by_fr.into_iter().collect(),
        admitted_by_format: by_f.into_iter().collect(),
        admitted_uncalibrated: uncal.into_iter().collect(),
        exclusions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_csv_reader_handles_quotes_and_maps_the_inventory_columns() {
        let text = "doc_id,path,format,round,wave,generator_model,generator_lane,physical_variant,archetype,sha256\r\n\
                    d1,documents/a.png,screenshot,2,3,\"vendor/model, v2\",\"\",png,A3,ABCDEF\n\
                    d2,documents/b.mp3,audio,4,1,local,\"\",mp3,\"R4 \"\"x\"\"\",0011\n";
        let m = parse_manifest_csv(text).unwrap();
        assert_eq!(m.assets.len(), 2);
        let a = &m.assets[0];
        assert_eq!(a.container, "png");
        assert_eq!(a.round, "2");
        assert_eq!(a.generator, "vendor/model, v2");
        assert_eq!(a.content_class, "");
        assert_eq!(a.field("archetype").as_deref(), Some("A3"));
        assert_eq!(a.sha256, "abcdef");
        assert_eq!(a.field("wave").as_deref(), Some("3"));
        assert_eq!(a.field("container").as_deref(), Some("png"));
        assert_eq!(a.field("format").as_deref(), Some("screenshot"));
        let b = &m.assets[1];
        assert_eq!(b.container, "mp3");
        assert_eq!(b.field("archetype").as_deref(), Some("R4 \"x\""));
        assert!(
            manifest_format(&b.container).is_none(),
            "mp3 is not calibrated"
        );
        assert_eq!(derive_format("screenshot", "screenshot", "d/x.png"), "png");
        assert_eq!(
            derive_format("credentialed", "c2pa-asset", "d/x.webp"),
            "webp"
        );
        assert_eq!(derive_format("", "audio", "d/x.wav"), "wav");
        assert_eq!(derive_format("heic", "png", "d/x.heic"), "heic");
        assert_eq!(derive_format("", "pdf-image", "d/x.pdf"), "pdf");
    }

    #[test]
    fn generic_rules_select_by_any_field_and_annotations_merge() {
        let text = "doc_id,path,format,round,wave,physical_variant,sha256\n\
                    d1,a.png,png,1,0,,00\nd2,b.png,png,4,1,,00\nd3,c.jpg,jpeg,2,1,,00\n";
        let mut m = parse_manifest_csv(text).unwrap();
        let f = Filters {
            exclude: vec!["round=4".to_string()],
            include: vec![
                "container=png".to_string(),
                "container=jpeg".to_string(),
                "wave=1".to_string(),
            ],
            ..Filters::default()
        };
        let reasons: Vec<Option<String>> = m.assets.iter().map(|a| f.exclusion(a)).collect();
        assert!(reasons[0].as_deref().unwrap().contains("wave"));
        assert!(reasons[1].as_deref().unwrap().contains("round"));
        assert!(reasons[2].is_none());
        assert!(Filters {
            include: vec!["novalue".to_string()],
            ..Filters::default()
        }
        .validate()
        .is_err());
        let unmatched = annotate(
            &mut m,
            "doc_id,documented_marks,native_origin,source_quality,ladder_step\n\
             d3,synthid:vendor-stated;transformed,true,,\nd9,x,,,\n",
        )
        .unwrap();
        assert_eq!(unmatched, 1);
        assert_eq!(
            m.assets[2].documented_marks,
            vec!["synthid:vendor-stated", "transformed"]
        );
        assert_eq!(m.assets[2].native_origin, Some(true));
        assert_eq!(m.assets[2].marks_key(), "synthid:vendor-stated+transformed");
    }

    #[test]
    fn eligibility_follows_the_ruling() {
        let base = parse_manifest_csv("doc_id,path,format,sha256\nd1,a.jpg,jpeg,00\n")
            .unwrap()
            .assets
            .remove(0);
        let (e, r) = eligibility(&base);
        assert_eq!(e, Eligibility::ReportOnly);
        assert!(r.unwrap().contains("source quality"));
        let mut native = base.clone();
        native.native_origin = Some(true);
        assert_eq!(eligibility(&native).0, Eligibility::Floor);
        let mut q = base.clone();
        q.source_quality = Some(92);
        assert_eq!(eligibility(&q).0, Eligibility::Floor);
        let mut dup = native.clone();
        dup.near_duplicate_of = Some("d0".to_string());
        assert_eq!(eligibility(&dup).0, Eligibility::ReportOnly);
        let mut wav = base.clone();
        wav.container = "wav".to_string();
        assert!(eligibility(&wav).1.unwrap().contains("native_rate"));
        wav.native_rate = Some(true);
        wav.derived = "container".to_string();
        assert_eq!(eligibility(&wav).0, Eligibility::Container);
        let mut synth = wav.clone();
        synth.derived = "synth".to_string();
        assert_eq!(eligibility(&synth).0, Eligibility::ReportOnly);
        let mut ladder = base.clone();
        ladder.post_processed = true;
        assert_eq!(eligibility(&ladder).0, Eligibility::Ladder);
    }
}
