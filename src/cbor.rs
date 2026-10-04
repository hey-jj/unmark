//! A minimal CBOR reader (RFC 8949) for the C2PA claim, assertion, and
//! signature payloads. It reads structure and values. Nothing here signs,
//! verifies, or writes.

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Uint(u64),
    /// A negative integer, stored as `n` for the value `-1 - n`.
    Negative(u64),
    Bytes(Vec<u8>),
    Text(String),
    Array(Vec<Value>),
    Map(Vec<(Value, Value)>),
    Tag(u64, Box<Value>),
    Bool(bool),
    Null,
    Undefined,
    Float(f64),
    Simple(u8),
}

/// Refuse nesting deeper than this limit.
const MAX_DEPTH: usize = 64;

/// Decode one item that fills the buffer exactly. None on any malformed
/// encoding, trailing bytes, or excessive nesting.
pub fn decode(bytes: &[u8]) -> Option<Value> {
    let mut p = 0;
    let v = item(bytes, &mut p, 0)?;
    if p != bytes.len() {
        return None;
    }
    Some(v)
}

/// The initial byte and its argument. `None` as the argument marks an
/// indefinite length.
fn head(bytes: &[u8], p: &mut usize) -> Option<(u8, u8, Option<u64>)> {
    let b = *bytes.get(*p)?;
    *p += 1;
    let major = b >> 5;
    let info = b & 0x1F;
    let arg = match info {
        0..=23 => Some(info as u64),
        24 => {
            let v = *bytes.get(*p)?;
            *p += 1;
            Some(v as u64)
        }
        25 => {
            let s = bytes.get(*p..*p + 2)?;
            *p += 2;
            Some(u16::from_be_bytes([s[0], s[1]]) as u64)
        }
        26 => {
            let s = bytes.get(*p..*p + 4)?;
            *p += 4;
            Some(u32::from_be_bytes([s[0], s[1], s[2], s[3]]) as u64)
        }
        27 => {
            let s = bytes.get(*p..*p + 8)?;
            *p += 8;
            let mut a = [0u8; 8];
            a.copy_from_slice(s);
            Some(u64::from_be_bytes(a))
        }
        31 if matches!(major, 2..=5 | 7) => None,
        _ => return None,
    };
    Some((major, info, arg))
}

fn take<'a>(bytes: &'a [u8], p: &mut usize, n: u64) -> Option<&'a [u8]> {
    let n = usize::try_from(n).ok()?;
    let s = bytes.get(*p..p.checked_add(n)?)?;
    *p += n;
    Some(s)
}

fn at_break(bytes: &[u8], p: &mut usize) -> bool {
    if bytes.get(*p) == Some(&0xFF) {
        *p += 1;
        true
    } else {
        false
    }
}

fn item(bytes: &[u8], p: &mut usize, depth: usize) -> Option<Value> {
    if depth > MAX_DEPTH {
        return None;
    }
    let (major, info, arg) = head(bytes, p)?;
    Some(match major {
        0 => Value::Uint(arg?),
        1 => Value::Negative(arg?),
        2 | 3 => {
            let mut buf = Vec::new();
            match arg {
                Some(n) => buf.extend_from_slice(take(bytes, p, n)?),
                None => loop {
                    if at_break(bytes, p) {
                        break;
                    }
                    let (m, _, a) = head(bytes, p)?;
                    if m != major {
                        return None;
                    }
                    buf.extend_from_slice(take(bytes, p, a?)?);
                },
            }
            if major == 2 {
                Value::Bytes(buf)
            } else {
                Value::Text(String::from_utf8(buf).ok()?)
            }
        }
        4 => {
            let mut items = Vec::new();
            match arg {
                Some(n) => {
                    for _ in 0..n {
                        items.push(item(bytes, p, depth + 1)?);
                    }
                }
                None => loop {
                    if at_break(bytes, p) {
                        break;
                    }
                    items.push(item(bytes, p, depth + 1)?);
                },
            }
            Value::Array(items)
        }
        5 => {
            let mut pairs = Vec::new();
            match arg {
                Some(n) => {
                    for _ in 0..n {
                        let k = item(bytes, p, depth + 1)?;
                        let v = item(bytes, p, depth + 1)?;
                        pairs.push((k, v));
                    }
                }
                None => loop {
                    if at_break(bytes, p) {
                        break;
                    }
                    let k = item(bytes, p, depth + 1)?;
                    let v = item(bytes, p, depth + 1)?;
                    pairs.push((k, v));
                },
            }
            Value::Map(pairs)
        }
        6 => Value::Tag(arg?, Box::new(item(bytes, p, depth + 1)?)),
        _ => match arg {
            Some(20) => Value::Bool(false),
            Some(21) => Value::Bool(true),
            Some(22) => Value::Null,
            Some(23) => Value::Undefined,
            Some(v) => match info {
                25 => Value::Float(half_to_f64(v as u16)),
                26 => Value::Float(f32::from_bits(v as u32) as f64),
                27 => Value::Float(f64::from_bits(v)),
                _ => Value::Simple(v as u8),
            },
            None => return None,
        },
    })
}

fn half_to_f64(h: u16) -> f64 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = ((h >> 10) & 0x1F) as i32;
    let frac = (h & 0x3FF) as f64;
    let v = match exp {
        0 => frac * 2f64.powi(-24),
        31 => {
            if frac == 0.0 {
                f64::INFINITY
            } else {
                f64::NAN
            }
        }
        _ => (1.0 + frac / 1024.0) * 2f64.powi(exp - 15),
    };
    sign * v
}

impl Value {
    /// The value under a text key of a map.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Map(pairs) => pairs
                .iter()
                .find(|(k, _)| matches!(k, Value::Text(t) if t == key))
                .map(|(_, v)| v),
            _ => None,
        }
    }

    /// The value under an integer key of a map, the COSE header form.
    pub fn get_int(&self, key: i64) -> Option<&Value> {
        match self {
            Value::Map(pairs) => pairs
                .iter()
                .find(|(k, _)| match k {
                    Value::Uint(u) => key >= 0 && *u == key as u64,
                    Value::Negative(n) => key < 0 && *n == (-1 - key) as u64,
                    _ => false,
                })
                .map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Value::Text(t) => Some(t),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Value::Bytes(b) => Some(b),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn is_map(&self) -> bool {
        matches!(self, Value::Map(_))
    }

    /// The value with any tags peeled off.
    pub fn untagged(&self) -> &Value {
        let mut v = self;
        while let Value::Tag(_, inner) = v {
            v = inner;
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_common_shapes_and_refuses_trailing_bytes() {
        // {"a": 1, "b": [true, null, "x"], 33: h'0102'}
        let bytes = [
            0xA3, 0x61, b'a', 0x01, 0x61, b'b', 0x83, 0xF5, 0xF6, 0x61, b'x', 0x18, 0x21, 0x42,
            0x01, 0x02,
        ];
        let v = decode(&bytes).unwrap();
        assert_eq!(v.get("a"), Some(&Value::Uint(1)));
        assert_eq!(v.get("b").unwrap().as_array().unwrap().len(), 3);
        assert_eq!(v.get_int(33).unwrap().as_bytes(), Some(&[1u8, 2][..]));
        let mut trailing = bytes.to_vec();
        trailing.push(0);
        assert!(decode(&trailing).is_none());
        assert!(decode(&bytes[..5]).is_none());
        // Tag 18 around an array, and a negative key.
        let cose = [0xD2, 0x82, 0x40, 0xA1, 0x20, 0x01];
        let v = decode(&cose).unwrap();
        let inner = v.untagged().as_array().unwrap();
        assert_eq!(inner[1].get_int(-1), Some(&Value::Uint(1)));
        // Indefinite text and a half float.
        let indef = [0x7F, 0x61, b'a', 0x61, b'b', 0xFF];
        assert_eq!(decode(&indef).unwrap().as_text(), Some("ab"));
        assert_eq!(decode(&[0xF9, 0x3C, 0x00]).unwrap(), Value::Float(1.0));
    }

    #[test]
    fn keyword_text_is_not_cbor() {
        assert!(decode(b"jumb c2pa c2pa.created digitalCapture").is_none());
    }
}
