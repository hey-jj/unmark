//! XMP recognition. XMP is an XML packet, so detection reads well-known field
//! names as evidence without a full XML parse.

/// The standard XMP packet marker in a JPEG APP1 segment.
pub const XMP_APP1_ID: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";

/// The iTXt keyword PNG uses to carry an XMP packet.
pub const XMP_PNG_KEYWORD: &str = "XML:com.adobe.xmp";

/// Pull a short evidence string from an XMP packet: the CreatorTool value when
/// present, else a marker that a packet was found.
pub fn creator_tool(packet: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(packet);
    for key in ["xmp:CreatorTool", "CreatorTool"] {
        if let Some(v) = attr_or_element(&text, key) {
            return Some(v);
        }
    }
    None
}

/// True when the packet names a C2PA manifest reference, which XMP can carry.
pub fn references_c2pa(packet: &[u8]) -> bool {
    let lower = packet.to_ascii_lowercase();
    super::c2pa::contains_ci(&lower, b"c2pa")
}

fn attr_or_element(text: &str, key: &str) -> Option<String> {
    // Attribute form: key="value"
    if let Some(i) = text.find(&format!("{key}=\"")) {
        let start = i + key.len() + 2;
        if let Some(rest) = text.get(start..) {
            if let Some(end) = rest.find('"') {
                let v = rest[..end].trim().to_string();
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
    }
    // Element form: <key>value</key>
    let open = format!("<{key}>");
    if let Some(i) = text.find(&open) {
        let start = i + open.len();
        if let Some(rest) = text.get(start..) {
            if let Some(end) = rest.find('<') {
                let v = rest[..end].trim().to_string();
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
    }
    None
}
