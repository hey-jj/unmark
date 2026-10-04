//! C2PA recognition. The parser reads structure and content. Signatures
//! remain unchecked. The build is offline and carries no certificate stack,
//! so a refusal keys on manifest content.

/// True when a byte range carries a JUMBF box or a C2PA marker. The JUMBF box
/// type is the four bytes `jumb`, and a C2PA store names itself `c2pa`.
pub fn looks_like_c2pa(bytes: &[u8]) -> bool {
    contains(bytes, b"jumb") || contains_ci(bytes, b"c2pa")
}

/// Decode a run of Unicode variation selectors to the bytes it carries. The
/// encoding maps a byte to `U+FE00 + b` for the low sixteen values and to
/// `U+E0100 + (b - 16)` for the rest, the scheme a C2PA text wrapper uses to
/// ride inside otherwise-plain text. Returns None when a code point in the run
/// is not a variation selector.
pub fn decode_variation_run(run: &[char]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(run.len());
    for &c in run {
        let u = c as u32;
        if (0xFE00..=0xFE0F).contains(&u) {
            out.push((u - 0xFE00) as u8);
        } else if (0xE0100..=0xE01EF).contains(&u) {
            out.push((u - 0xE0100 + 16) as u8);
        } else {
            return None;
        }
    }
    Some(out)
}

/// Classify a variation-selector run for the MC07 collision. A run that decodes
/// to bytes carrying a C2PA marker is a manifest wrapper and belongs on the
/// C2PA path. A run long enough to be a wrapper that does not decode to one is
/// reported malformed and left alone. A short run is ordinary invisible text.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RunClass {
    /// A C2PA text wrapper. Route to the manifest path.
    C2paWrapper,
    /// Wrapper-shaped and unparseable. Leave it alone.
    Malformed,
    /// Ordinary invisible characters.
    Hygiene,
}

/// The length at or above which an unparseable variation-selector run is
/// treated as a wrapper that failed to parse. A shorter unparseable run is
/// treated as stray hygiene.
pub const WRAPPER_SUSPECT_LEN: usize = 8;

pub fn classify_run(run: &[char]) -> RunClass {
    match decode_variation_run(run) {
        Some(bytes) if looks_like_c2pa(&bytes) => RunClass::C2paWrapper,
        _ if run.len() >= WRAPPER_SUSPECT_LEN => RunClass::Malformed,
        _ => RunClass::Hygiene,
    }
}

pub fn contains(hay: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || hay.len() < needle.len() {
        return false;
    }
    hay.windows(needle.len()).any(|w| w == needle)
}

pub fn contains_ci(hay: &[u8], needle_lower: &[u8]) -> bool {
    if needle_lower.is_empty() || hay.len() < needle_lower.len() {
        return false;
    }
    hay.windows(needle_lower.len())
        .any(|w| w.eq_ignore_ascii_case(needle_lower))
}
