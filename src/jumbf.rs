//! JUMBF box reading (ISO/IEC 19566-5) for the C2PA manifest store. The
//! reader walks box headers, superboxes, and description boxes. It writes
//! nothing.

/// One box: its four-byte type and the payload after the header.
#[derive(Clone, Debug)]
pub struct JBox<'a> {
    pub kind: [u8; 4],
    pub payload: &'a [u8],
}

/// The C2PA UUID suffix; the first four bytes name the box role.
pub const C2PA_SUFFIX: [u8; 12] = [
    0x00, 0x11, 0x00, 0x10, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
];

pub fn c2pa_uuid(role: &[u8; 4]) -> [u8; 16] {
    let mut u = [0u8; 16];
    u[..4].copy_from_slice(role);
    u[4..].copy_from_slice(&C2PA_SUFFIX);
    u
}

fn be32(b: &[u8]) -> Option<u32> {
    Some(u32::from_be_bytes([
        *b.first()?,
        *b.get(1)?,
        *b.get(2)?,
        *b.get(3)?,
    ]))
}

/// Parse a sequence of boxes that fills `bytes` exactly. None when a header
/// is short, a length overruns, or bytes are left over.
pub fn boxes(bytes: &[u8]) -> Option<Vec<JBox<'_>>> {
    let mut out = Vec::new();
    let mut p = 0usize;
    while p < bytes.len() {
        let lbox = be32(bytes.get(p..)?)? as usize;
        let kind_bytes = bytes.get(p + 4..p + 8)?;
        let mut kind = [0u8; 4];
        kind.copy_from_slice(kind_bytes);
        let (header, total) = match lbox {
            0 => (8usize, bytes.len() - p),
            1 => {
                let hi = be32(bytes.get(p + 8..)?)? as u64;
                let lo = be32(bytes.get(p + 12..)?)? as u64;
                let xl = usize::try_from((hi << 32) | lo).ok()?;
                (16usize, xl)
            }
            n => (8usize, n),
        };
        if total < header || p.checked_add(total)? > bytes.len() {
            return None;
        }
        out.push(JBox {
            kind,
            payload: &bytes[p + header..p + total],
        });
        p += total;
    }
    Some(out)
}

/// A superbox: the description box's UUID and label, and the content boxes
/// after it.
#[derive(Clone, Debug)]
pub struct SuperBox<'a> {
    pub uuid: [u8; 16],
    pub label: Option<String>,
    pub content: Vec<JBox<'a>>,
}

/// Read a `jumb` box whose first child is a well-formed `jumd` description.
pub fn superbox<'a>(b: &JBox<'a>) -> Option<SuperBox<'a>> {
    if &b.kind != b"jumb" {
        return None;
    }
    let children = boxes(b.payload)?;
    let (desc, rest) = children.split_first()?;
    if &desc.kind != b"jumd" {
        return None;
    }
    let d = desc.payload;
    let mut uuid = [0u8; 16];
    uuid.copy_from_slice(d.get(..16)?);
    let toggles = *d.get(16)?;
    let mut p = 17usize;
    let label = if toggles & 0x02 != 0 {
        let end = d[p..].iter().position(|&c| c == 0)? + p;
        let s = String::from_utf8(d[p..end].to_vec()).ok()?;
        p = end + 1;
        Some(s)
    } else {
        None
    };
    if toggles & 0x04 != 0 {
        p += 4;
    }
    if toggles & 0x08 != 0 {
        p += 32;
    }
    if p > d.len() {
        return None;
    }
    // Anything after the fixed fields must itself be a box list (a private
    // box) or empty.
    if p < d.len() && boxes(&d[p..]).is_none() {
        return None;
    }
    Some(SuperBox {
        uuid,
        label,
        content: rest.to_vec(),
    })
}

impl<'a> SuperBox<'a> {
    pub fn is(&self, role: &[u8; 4]) -> bool {
        self.uuid == c2pa_uuid(role)
    }

    /// The nested superboxes among the content boxes. A `jumb` child that
    /// does not parse makes the whole box malformed.
    pub fn children(&self) -> Option<Vec<SuperBox<'a>>> {
        let mut out = Vec::new();
        for c in &self.content {
            if &c.kind == b"jumb" {
                out.push(superbox(c)?);
            }
        }
        Some(out)
    }

    /// The payload of the first content box of a kind.
    pub fn content_of(&self, kind: &[u8; 4]) -> Option<&'a [u8]> {
        self.content
            .iter()
            .find(|c| &c.kind == kind)
            .map(|c| c.payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jbox(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut out = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(payload);
        out
    }

    #[test]
    fn a_labelled_superbox_parses_and_keyword_text_does_not() {
        let mut jumd = c2pa_uuid(b"c2pa").to_vec();
        jumd.push(0x03);
        jumd.extend_from_slice(b"c2pa\0");
        let mut inner = jbox(b"jumd", &jumd);
        inner.extend_from_slice(&jbox(b"cbor", &[0xA0]));
        let outer = jbox(b"jumb", &inner);
        let bs = boxes(&outer).unwrap();
        assert_eq!(bs.len(), 1);
        let sb = superbox(&bs[0]).unwrap();
        assert!(sb.is(b"c2pa"));
        assert_eq!(sb.label.as_deref(), Some("c2pa"));
        assert_eq!(sb.content_of(b"cbor"), Some(&[0xA0u8][..]));
        assert!(boxes(b"jumb c2pa c2pa.created digitalCapture").is_none());
        // A truncated length overruns and is refused.
        let mut bad = outer.clone();
        bad[3] += 1;
        assert!(boxes(&bad).is_none());
    }
}
