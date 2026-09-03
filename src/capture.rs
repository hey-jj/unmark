//! The capture reading. An asset is certified capture when a well-formed
//! C2PA manifest store carries a manifest whose claim references an actions
//! assertion with a `c2pa.created` action of digital source type
//! `digitalCapture`, or whose signer certificate names a camera vendor, and
//! no later action names a generative source type or tool. Well-formed means
//! the JUMBF boxes, the claim CBOR, and the assertion references all parse;
//! keyword text inside an unparseable payload is never a claim. A claim is
//! read, never signature-verified. Camera-style EXIF without such a claim is uncertain:
//! stripped by default, with the hint named. A publisher manifest without a
//! capture action is not a capture at all.
//!
//! The rest of the old guardrail set is a constraint on what this build
//! contains rather than a runtime check: no safety-hash targeting, no keyed
//! mark scorer, no fabricated provenance. No code path implements them, which
//! is the enforcement.

use crate::asset::Format;
use crate::cbor;
use crate::detect::exif::{parse_tiff, ExifFacts};
use crate::jumbf;
use crate::scan::Detections;

/// What the asset's provenance says about capture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaptureStatus {
    /// A well-formed capture claim with no later generative action.
    Certified,
    /// A capture claim followed by an action naming a generative tool.
    Generative,
    /// Camera-style EXIF without a qualifying claim.
    Uncertain,
    /// No capture signal.
    None,
}

impl CaptureStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            CaptureStatus::Certified => "certified",
            CaptureStatus::Generative => "generative",
            CaptureStatus::Uncertain => "uncertain",
            CaptureStatus::None => "none",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureReading {
    pub status: CaptureStatus,
    /// The quoted claim markers for a certified or generative reading.
    pub claim: Option<String>,
    /// The named hint for an uncertain reading.
    pub hint: Option<String>,
}

pub const SIGNATURE_STATUS: &str = "not signature-verified";

/// The IPTC digital source type that names a capture, matched on its
/// terminal segment.
const DIGITAL_CAPTURE: &str = "digitalcapture";

/// Generative digital source types, matched on the terminal segment.
const GENERATIVE_SOURCE_TYPES: &[&str] = &[
    "trainedalgorithmicmedia",
    "compositewithtrainedalgorithmicmedia",
    "algorithmicmedia",
];

/// Camera vendors used by both the capture-signer read and the EXIF hint.
fn camera_makes() -> Vec<String> {
    include_str!("../policy/signatures/camera-makes.txt")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

fn generator_strings() -> Vec<String> {
    include_str!("../policy/signatures/generator-strings.txt")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_ascii_lowercase)
        .collect()
}

/// One action from a manifest's actions assertion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionRecord {
    pub action: String,
    pub digital_source_type: Option<String>,
    pub software_agent: Option<String>,
}

/// One well-formed manifest: its label, the actions its claim references,
/// and the names read from the signer's certificate chain.
#[derive(Clone, Debug)]
pub struct Manifest {
    pub label: String,
    pub actions: Vec<ActionRecord>,
    pub signer_names: Vec<String>,
}

/// Read the capture status from the asset's manifest and EXIF.
pub fn read_capture(bytes: &[u8], det: &Detections, format: Format) -> CaptureReading {
    for candidate in manifest_candidates(bytes, det, format) {
        if let Some(store) = parse_store(&candidate) {
            if let Some(reading) = read_store(&store) {
                return reading;
            }
        }
    }
    if let Some(hint) = exif_hint(bytes, format) {
        return CaptureReading {
            status: CaptureStatus::Uncertain,
            claim: None,
            hint: Some(hint),
        };
    }
    CaptureReading {
        status: CaptureStatus::None,
        claim: None,
        hint: None,
    }
}

/// The byte ranges that may hold a manifest store, one per carriage. JPEG
/// APP11 packets that share a box instance are reassembled in order; a text
/// wrapper is decoded from its variation selectors; the other containers
/// hand over the chunk or box payload.
fn manifest_candidates(bytes: &[u8], det: &Detections, format: Format) -> Vec<Vec<u8>> {
    let mut out: Vec<Vec<u8>> = Vec::new();
    let Some(c2pa) = det.get("c2pa") else {
        return out;
    };
    let mut packets: Vec<(u16, Vec<u8>)> = Vec::new();
    for loc in &c2pa.locations {
        let Some(region) = bytes.get(loc.offset..loc.offset + loc.length) else {
            continue;
        };
        match format {
            Format::Jpeg => {
                let Some(data) = region.get(4..) else {
                    continue;
                };
                if data.len() >= 8 && &data[..2] == b"JP" {
                    let en = u16::from_be_bytes([data[2], data[3]]);
                    let rest = &data[8..];
                    if let Some(g) = packets.iter_mut().find(|g| g.0 == en) {
                        // A continuation repeats the box header; the
                        // payload after it continues the first packet.
                        g.1.extend_from_slice(rest.get(8..).unwrap_or(&[]));
                    } else {
                        packets.push((en, rest.to_vec()));
                    }
                } else {
                    out.push(data.to_vec());
                }
            }
            Format::Png => {
                if region.len() >= 12 {
                    out.push(region[8..region.len() - 4].to_vec());
                }
            }
            Format::WebP | Format::RiffWav => {
                if region.len() >= 8 {
                    let size =
                        u32::from_le_bytes([region[4], region[5], region[6], region[7]]) as usize;
                    let end = (8 + size).min(region.len());
                    out.push(region[8..end].to_vec());
                }
            }
            Format::Isobmff => {
                // uuid box: size, type, 16-byte extended type, then the C2PA
                // box body: version and flags, a purpose string, and for
                // the manifest purpose the store itself.
                if let Some(body) = region.get(24..) {
                    let after_flags = body.get(4..).unwrap_or(&[]);
                    if let Some(nul) = after_flags.iter().position(|&c| c == 0) {
                        if &after_flags[..nul] == b"manifest" {
                            out.push(after_flags[nul + 1..].to_vec());
                            continue;
                        }
                    }
                    out.push(body.to_vec());
                }
            }
            Format::Text | Format::Svg | Format::Html => {
                if let Ok(text) = std::str::from_utf8(region) {
                    let run: Vec<char> = text.chars().collect();
                    if let Some(decoded) = crate::detect::c2pa::decode_variation_run(&run) {
                        out.push(decoded);
                    }
                }
            }
            _ => out.push(region.to_vec()),
        }
    }
    out.extend(packets.into_iter().map(|g| g.1));
    // A candidate must begin at a superbox header. When the carriage put
    // bytes before it, start at the first box header instead.
    out.into_iter()
        .filter_map(|c| {
            if c.len() >= 8 && &c[4..8] == b"jumb" {
                return Some(c);
            }
            let at = c.windows(4).position(|w| w == b"jumb")?;
            if at < 4 {
                return None;
            }
            Some(c[at - 4..].to_vec())
        })
        .collect()
}

/// Parse a manifest store. None unless the bytes are one well-formed C2PA
/// store: a `c2pa` superbox holding manifests that each carry an assertion
/// store, a claim decoding to a CBOR map with its assertion references, and
/// a claim signature. An actions assertion must be one the claim references.
pub fn parse_store(bytes: &[u8]) -> Option<Vec<Manifest>> {
    let top = jumbf::boxes(bytes)?;
    if top.len() != 1 {
        return None;
    }
    let store = jumbf::superbox(&top[0])?;
    if !store.is(b"c2pa") || store.label.as_deref() != Some("c2pa") {
        return None;
    }
    let mut manifests = Vec::new();
    for m in store.children()? {
        if !(m.is(b"c2ma") || m.is(b"c2um")) {
            return None;
        }
        manifests.push(parse_manifest(&m)?);
    }
    if manifests.is_empty() {
        return None;
    }
    Some(manifests)
}

fn parse_manifest(m: &jumbf::SuperBox) -> Option<Manifest> {
    let label = m.label.clone()?;
    let children = m.children()?;
    let assertions = children
        .iter()
        .find(|c| c.is(b"c2as") && c.label.as_deref() == Some("c2pa.assertions"))?;
    let claim_box = children
        .iter()
        .find(|c| c.is(b"c2cl") && c.label.as_deref() == Some("c2pa.claim"))?;
    let signature_box = children
        .iter()
        .find(|c| c.is(b"c2cs") && c.label.as_deref() == Some("c2pa.signature"))?;
    let claim = cbor::decode(claim_box.content_of(b"cbor")?)?;
    if !claim.is_map() {
        return None;
    }
    claim.get("signature")?.as_text()?;
    let mut referenced: Vec<String> = Vec::new();
    let mut any_list = false;
    for key in ["assertions", "created_assertions", "gathered_assertions"] {
        if let Some(list) = claim.get(key) {
            any_list = true;
            for entry in list.as_array()? {
                let url = entry.get("url")?.as_text()?;
                referenced.push(url.to_string());
            }
        }
    }
    if !any_list {
        return None;
    }
    let mut actions = Vec::new();
    for a in assertions.children()? {
        let alabel = a.label.clone()?;
        if !alabel.starts_with("c2pa.actions") {
            continue;
        }
        let is_referenced = referenced
            .iter()
            .any(|u| u.ends_with(&format!("c2pa.assertions/{alabel}")));
        if !is_referenced {
            return None;
        }
        let body = cbor::decode(a.content_of(b"cbor")?)?;
        for act in body.get("actions")?.as_array()? {
            let action = act.get("action")?.as_text()?.to_string();
            let digital_source_type = act
                .get("digitalSourceType")
                .and_then(|v| v.as_text())
                .map(str::to_string);
            let software_agent = act.get("softwareAgent").and_then(|v| match v {
                cbor::Value::Text(t) => Some(t.clone()),
                cbor::Value::Map(_) => v.get("name").and_then(|n| n.as_text()).map(str::to_string),
                _ => None,
            });
            actions.push(ActionRecord {
                action,
                digital_source_type,
                software_agent,
            });
        }
    }
    let signer_names = signature_box
        .content_of(b"cbor")
        .and_then(cbor::decode)
        .map(|v| cose_names(&v))
        .unwrap_or_default();
    Some(Manifest {
        label,
        actions,
        signer_names,
    })
}

/// The organization and common names in a COSE_Sign1 certificate chain
/// (header label 33 in the protected or unprotected header).
fn cose_names(sig: &cbor::Value) -> Vec<String> {
    let mut names = Vec::new();
    let Some(parts) = sig.untagged().as_array() else {
        return names;
    };
    let mut chains: Vec<&cbor::Value> = Vec::new();
    if let Some(unprotected) = parts.get(1) {
        if let Some(x5) = unprotected.get_int(33) {
            chains.push(x5);
        }
    }
    let protected_decoded = parts
        .first()
        .and_then(|p| p.as_bytes())
        .and_then(cbor::decode);
    if let Some(p) = &protected_decoded {
        if let Some(x5) = p.get_int(33) {
            chains.push(x5);
        }
    }
    for chain in chains {
        let certs: Vec<&[u8]> = match chain {
            cbor::Value::Bytes(b) => vec![b.as_slice()],
            cbor::Value::Array(items) => items.iter().filter_map(|c| c.as_bytes()).collect(),
            _ => Vec::new(),
        };
        for cert in certs {
            der_names(cert, 0, &mut names);
        }
    }
    names
}

/// Walk DER and collect the values of organization (2.5.4.10) and common
/// name (2.5.4.3) attributes.
fn der_names(bytes: &[u8], depth: usize, out: &mut Vec<String>) {
    if depth > 32 {
        return;
    }
    let Some(nodes) = der_list(bytes) else {
        return;
    };
    if nodes.len() == 2 && nodes[0].0 == 0x06 {
        let oid = nodes[0].1;
        if (oid == [0x55, 0x04, 0x0A] || oid == [0x55, 0x04, 0x03])
            && matches!(nodes[1].0, 0x0C | 0x13 | 0x14 | 0x16)
        {
            out.push(String::from_utf8_lossy(nodes[1].1).into_owned());
        }
    }
    for (tag, content) in nodes {
        if tag & 0x20 != 0 {
            der_names(content, depth + 1, out);
        }
    }
}

/// A DER tag-length-value list filling `bytes`. Multi-byte tags are refused.
fn der_list(bytes: &[u8]) -> Option<Vec<(u8, &[u8])>> {
    let mut out = Vec::new();
    let mut p = 0usize;
    while p < bytes.len() {
        let tag = *bytes.get(p)?;
        if tag & 0x1F == 0x1F {
            return None;
        }
        let first = *bytes.get(p + 1)?;
        let (len, header) = if first < 0x80 {
            (first as usize, 2usize)
        } else {
            let n = (first & 0x7F) as usize;
            if n == 0 || n > 4 {
                return None;
            }
            let mut len = 0usize;
            for i in 0..n {
                len = (len << 8) | *bytes.get(p + 2 + i)? as usize;
            }
            (len, 2 + n)
        };
        let start = p + header;
        let end = start.checked_add(len)?;
        let content = bytes.get(start..end)?;
        out.push((tag, content));
        p = end;
    }
    Some(out)
}

fn ends_with_segment(value: &str, segment: &str) -> bool {
    let v = value.to_ascii_lowercase();
    v == segment || v.ends_with(&format!("/{segment}"))
}

fn is_generative(a: &ActionRecord, generators: &[String]) -> Option<String> {
    if let Some(st) = &a.digital_source_type {
        for g in GENERATIVE_SOURCE_TYPES {
            if ends_with_segment(st, g) {
                return Some(format!(
                    "a later action {} with digitalSourceType {g}",
                    a.action
                ));
            }
        }
    }
    if let Some(agent) = &a.software_agent {
        let al = agent.to_ascii_lowercase();
        for g in generators {
            if al.contains(g.as_str()) {
                return Some(format!("a later action {} naming {g}", a.action));
            }
        }
    }
    None
}

/// The capture reading of a parsed store. Manifests sit in store order, so
/// a generative action in any manifest after the claim, or after the
/// capture action within it, is a later action.
fn read_store(manifests: &[Manifest]) -> Option<CaptureReading> {
    let makes = camera_makes();
    let generators = generator_strings();
    for (mi, m) in manifests.iter().enumerate() {
        let capture_at = m.actions.iter().position(|a| {
            a.action == "c2pa.created"
                && a.digital_source_type
                    .as_deref()
                    .is_some_and(|st| ends_with_segment(st, DIGITAL_CAPTURE))
        });
        let vendor = m.signer_names.iter().find_map(|n| {
            let nl = n.to_ascii_lowercase();
            makes
                .iter()
                .find(|mk| nl.contains(&mk.to_ascii_lowercase()))
                .cloned()
        });
        let claim = match (capture_at, &vendor) {
            (Some(_), _) => format!(
                "manifest {} c2pa.created with digitalSourceType digitalCapture",
                m.label
            ),
            (None, Some(v)) => format!("manifest {} signed by the camera vendor {v}", m.label),
            (None, None) => continue,
        };
        let from = capture_at.map(|i| i + 1).unwrap_or(0);
        let mut later: Option<String> = None;
        for a in m.actions.iter().skip(from) {
            if let Some(l) = is_generative(a, &generators) {
                later = Some(l);
                break;
            }
        }
        if later.is_none() {
            'outer: for m2 in manifests.iter().skip(mi + 1) {
                for a in &m2.actions {
                    if let Some(l) = is_generative(a, &generators) {
                        later = Some(format!("{l} in manifest {}", m2.label));
                        break 'outer;
                    }
                }
            }
        }
        return Some(match later {
            Some(l) => CaptureReading {
                status: CaptureStatus::Generative,
                claim: Some(format!("{claim}; {l}")),
                hint: None,
            },
            None => CaptureReading {
                status: CaptureStatus::Certified,
                claim: Some(claim),
                hint: None,
            },
        });
    }
    None
}

/// The EXIF hint: a known camera body with lens data and a shutter,
/// aperture, and ISO triple describes a photograph, without proving one.
pub fn exif_hint(bytes: &[u8], format: Format) -> Option<String> {
    let facts = exif_facts(bytes, format)?;
    let make = facts.make.as_deref()?;
    let known = camera_makes().iter().any(|m| {
        make.eq_ignore_ascii_case(m) || make.to_ascii_lowercase().contains(&m.to_ascii_lowercase())
    });
    if known && facts.has_exposure_triple() && facts.has_lens {
        Some(format!(
            "EXIF names a camera body ({make}) with lens data and a shutter, aperture, and ISO triple"
        ))
    } else {
        None
    }
}

/// Pull EXIF facts from a supported image container without decoding pixels.
pub fn exif_facts(bytes: &[u8], format: Format) -> Option<ExifFacts> {
    match format {
        Format::Jpeg => {
            let (segments, _) = crate::detect::jpeg::segments(bytes);
            for s in segments {
                if s.marker == 0xE1 && s.data.starts_with(crate::detect::jpeg::EXIF_ID) {
                    return parse_tiff(&s.data[crate::detect::jpeg::EXIF_ID.len()..]);
                }
            }
            None
        }
        Format::Png => {
            let (chunks, _) = crate::detect::png_text::chunks(bytes);
            for c in chunks {
                if c.kind == *b"eXIf" {
                    return parse_tiff(c.data);
                }
            }
            None
        }
        Format::WebP => {
            let (chunks, _) = crate::detect::riff::chunks(bytes);
            for c in chunks {
                if &c.id == b"EXIF" {
                    let payload = c
                        .data
                        .strip_prefix(crate::detect::jpeg::EXIF_ID)
                        .unwrap_or(c.data);
                    return parse_tiff(payload);
                }
            }
            None
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_bytes_are_not_a_manifest_store() {
        assert!(parse_store(
            b"jumb c2pa c2pa.created digitalSourceType digitalCapture signer Leica Camera AG"
        )
        .is_none());
        assert!(parse_store(b"not-a-manifest c2pa.created digitalCapture").is_none());
        assert!(parse_store(b"").is_none());
    }

    #[test]
    fn der_names_finds_the_organization() {
        // SEQUENCE { SET { SEQUENCE { OID 2.5.4.10, UTF8String "Leica Camera AG" } } }
        let inner = [
            &[0x06, 0x03, 0x55, 0x04, 0x0A, 0x0C, 0x0F][..],
            b"Leica Camera AG",
        ]
        .concat();
        let seq = [&[0x30, inner.len() as u8][..], &inner].concat();
        let set = [&[0x31, seq.len() as u8][..], &seq].concat();
        let cert = [&[0x30, set.len() as u8][..], &set].concat();
        let mut names = Vec::new();
        der_names(&cert, 0, &mut names);
        assert_eq!(names, vec!["Leica Camera AG".to_string()]);
    }
}
