//! Container builders for the test suite. Each builds a minimal but structurally
//! valid asset carrying the metadata a test needs. Image and sample data are
//! placeholder bytes, since the metadata path never decodes them.

#![allow(dead_code)]

/// CRC-32/ISO-HDLC, the PNG chunk CRC.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

pub fn png_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_input = Vec::new();
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
    out
}

pub struct PngOpts {
    pub text: bool,
    pub exif: bool,
    pub xmp: bool,
    pub c2pa: bool,
    pub c2pa_payload: Option<Vec<u8>>,
    pub idat: Vec<u8>,
}

impl Default for PngOpts {
    fn default() -> Self {
        PngOpts {
            text: false,
            exif: false,
            xmp: false,
            c2pa: false,
            c2pa_payload: None,
            idat: stored_idat(PNG_SIDE),
        }
    }
}

/// The built PNG is a decodable 16 by 16 RGB gradient, so the default run's
/// pixel path can decode it.
pub const PNG_SIDE: usize = 16;

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &d in data {
        a = (a + d as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// A zlib stream with one stored block carrying the filtered scanlines of a
/// side-by-side RGB gradient.
pub fn stored_idat(side: usize) -> Vec<u8> {
    let mut raw = Vec::with_capacity(side * (1 + side * 3));
    for y in 0..side {
        raw.push(0);
        for x in 0..side {
            raw.push((x * 255 / (side - 1)) as u8);
            raw.push((y * 255 / (side - 1)) as u8);
            raw.push(((x + y) * 255 / (2 * side - 2)) as u8);
        }
    }
    let mut out = vec![0x78, 0x01, 0x01];
    let len = raw.len() as u16;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&(!len).to_le_bytes());
    out.extend_from_slice(&raw);
    out.extend_from_slice(&adler32(&raw).to_be_bytes());
    out
}

pub fn build_png(o: &PngOpts) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']);
    // IHDR: PNG_SIDE by PNG_SIDE RGB.
    let side = PNG_SIDE as u32;
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&side.to_be_bytes());
    ihdr.extend_from_slice(&side.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    out.extend_from_slice(&png_chunk(b"IHDR", &ihdr));
    if o.text {
        let mut d = Vec::new();
        d.extend_from_slice(b"parameters");
        d.push(0);
        d.extend_from_slice(b"a prompt, steps: 30, seed: 12345, model: sdxl");
        out.extend_from_slice(&png_chunk(b"tEXt", &d));
    }
    if o.exif {
        let mut d = Vec::new();
        d.extend_from_slice(b"II*\0\x08\0\0\0\0\0\0\0");
        out.extend_from_slice(&png_chunk(b"eXIf", &d));
    }
    if o.xmp {
        let mut d = Vec::new();
        d.extend_from_slice(b"XML:com.adobe.xmp");
        d.push(0);
        d.extend_from_slice(&[0, 0]); // compression flag and method
        d.push(0); // language null
        d.push(0); // translated keyword null
        d.extend_from_slice(b"<x:xmpmeta><xmp:CreatorTool>ComfyUI</xmp:CreatorTool></x:xmpmeta>");
        out.extend_from_slice(&png_chunk(b"iTXt", &d));
    }
    if o.c2pa {
        let default = b"jumb\0\0\0\0c2pa manifest store".to_vec();
        let payload = o.c2pa_payload.as_ref().unwrap_or(&default);
        out.extend_from_slice(&png_chunk(b"caBX", payload));
    }
    out.extend_from_slice(&png_chunk(b"IDAT", &o.idat));
    out.extend_from_slice(&png_chunk(b"IEND", &[]));
    out
}

fn jpeg_app(marker: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = vec![0xFF, marker];
    let len = (payload.len() + 2) as u16;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(payload);
    out
}

#[derive(Default)]
pub struct JpegOpts {
    pub exif: bool,
    pub exif_payload: Option<Vec<u8>>,
    pub xmp: bool,
    pub c2pa: bool,
    pub c2pa_payload: Option<Vec<u8>>,
}

/// A decodable JPEG body from the crate's own encoder: everything after the
/// SOI marker.
fn jpeg_body() -> Vec<u8> {
    let img = unmark::codec::Image {
        width: 16,
        height: 16,
        channels: 3,
        data: (0..16 * 16 * 3).map(|i| (i * 7 % 256) as u8).collect(),
    };
    let bytes = unmark::codec::jpeg::encode(&img, 92, "4:4:4").unwrap();
    bytes[2..].to_vec()
}

pub fn build_jpeg(o: &JpegOpts) -> Vec<u8> {
    let mut out = vec![0xFF, 0xD8];
    if o.exif {
        let mut p = b"Exif\0\0".to_vec();
        match &o.exif_payload {
            Some(tiff) => p.extend_from_slice(tiff),
            None => p.extend_from_slice(b"II*\0\x08\0\0\0\0\0\0\0"),
        }
        out.extend_from_slice(&jpeg_app(0xE1, &p));
    }
    if o.xmp {
        let mut p = b"http://ns.adobe.com/xap/1.0/\0".to_vec();
        p.extend_from_slice(b"<x:xmpmeta><xmp:CreatorTool>Firefly</xmp:CreatorTool></x:xmpmeta>");
        out.extend_from_slice(&jpeg_app(0xE1, &p));
    }
    if o.c2pa {
        let default = b"JP\0\0jumb c2pa manifest".to_vec();
        let payload = o.c2pa_payload.as_ref().unwrap_or(&default);
        // The JPEG box carriage: CI "JP", a box instance, a packet sequence,
        // then the box.
        let mut packet = Vec::new();
        if payload.starts_with(b"JP") {
            packet.extend_from_slice(payload);
        } else {
            packet.extend_from_slice(b"JP\0\x01\0\0\0\x01");
            packet.extend_from_slice(payload);
        }
        out.extend_from_slice(&jpeg_app(0xEB, &packet));
    }
    // The real frame, scan, and end of image.
    out.extend_from_slice(&jpeg_body());
    out
}

fn riff_chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(id);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
    if data.len() % 2 == 1 {
        out.push(0); // even-boundary padding
    }
    out
}

fn riff_container(form: &[u8; 4], chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut body = form.to_vec();
    for c in chunks {
        body.extend_from_slice(c);
    }
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    out
}

#[derive(Default)]
pub struct WavOpts {
    pub list_info: bool,
    pub id3: bool,
    pub c2pa: bool,
    pub bext: bool,
}

pub fn build_wav(o: &WavOpts) -> Vec<u8> {
    let mut chunks = Vec::new();
    // fmt: PCM, mono, 8000 Hz, 8-bit.
    let mut fmt = Vec::new();
    fmt.extend_from_slice(&1u16.to_le_bytes());
    fmt.extend_from_slice(&1u16.to_le_bytes());
    fmt.extend_from_slice(&8000u32.to_le_bytes());
    fmt.extend_from_slice(&8000u32.to_le_bytes());
    fmt.extend_from_slice(&1u16.to_le_bytes());
    fmt.extend_from_slice(&8u16.to_le_bytes());
    chunks.push(riff_chunk(b"fmt ", &fmt));
    if o.list_info {
        let mut d = b"INFO".to_vec();
        d.extend_from_slice(&riff_chunk(b"ISFT", b"ComfyUI\0"));
        chunks.push(riff_chunk(b"LIST", &d));
    }
    if o.bext {
        chunks.push(riff_chunk(b"bext", &[0u8; 32]));
    }
    if o.id3 {
        // A minimal embedded id3 chunk with an ID3v2 header inside.
        let mut d = b"ID3\x03\x00\x00".to_vec();
        d.extend_from_slice(&[0, 0, 0, 4]); // synchsafe size 4
        d.extend_from_slice(b"TEST");
        chunks.push(riff_chunk(b"id3 ", &d));
    }
    if o.c2pa {
        chunks.push(riff_chunk(b"C2PA", b"jumb c2pa store"));
    }
    // The audio payload.
    chunks.push(riff_chunk(b"data", &[10, 20, 30, 40, 50, 60]));
    riff_container(b"WAVE", &chunks)
}

#[derive(Default)]
pub struct WebpOpts {
    pub exif: bool,
    pub xmp: bool,
    pub c2pa: bool,
}

pub fn build_webp(o: &WebpOpts) -> Vec<u8> {
    let mut chunks = Vec::new();
    chunks.push(riff_chunk(
        b"VP8L",
        &[0x2F, 0x00, 0x00, 0x00, 0x00, 0x88, 0x88, 0x08],
    ));
    if o.exif {
        let mut d = b"Exif\0\0".to_vec();
        d.extend_from_slice(b"II*\0\x08\0\0\0\0\0\0\0");
        chunks.push(riff_chunk(b"EXIF", &d));
    }
    if o.xmp {
        chunks.push(riff_chunk(
            b"XMP ",
            b"<x:xmpmeta><xmp:CreatorTool>Imagen</xmp:CreatorTool></x:xmpmeta>",
        ));
    }
    if o.c2pa {
        chunks.push(riff_chunk(b"C2PA", b"jumb c2pa store"));
    }
    riff_container(b"WEBP", &chunks)
}

/// A TIFF block for a camera photograph: Make, plus exposure, aperture, ISO,
/// and focal length tags. Feeds guardrail G3.
pub fn camera_tiff() -> Vec<u8> {
    // Little-endian TIFF. IFD0 with entries, Make string stored after the IFD.
    let mut out = Vec::new();
    out.extend_from_slice(b"II"); // byte order
    out.extend_from_slice(&42u16.to_le_bytes()); // magic
    out.extend_from_slice(&8u32.to_le_bytes()); // IFD0 offset

    // Entries: Make(0x010F ascii), ExposureTime(0x829A), FNumber(0x829D),
    // ISO(0x8827), FocalLength(0x920A).
    let make = b"Canon\0";
    let entries: Vec<(u16, u16, u32, u32)> = vec![
        // tag, type, count, value-or-offset
        (0x010F, 2, make.len() as u32, 0), // Make offset patched below
        (0x829A, 5, 1, 100),               // ExposureTime (rational, offset unused by detector)
        (0x829D, 5, 1, 100),               // FNumber
        (0x8827, 3, 1, 400),               // ISO (short inline)
        (0x920A, 5, 1, 100),               // FocalLength
    ];
    let count = entries.len() as u16;
    let ifd_start = 8;
    let ifd_len = 2 + entries.len() * 12 + 4;
    let make_offset = (ifd_start + ifd_len) as u32;

    out.extend_from_slice(&count.to_le_bytes());
    for (i, (tag, typ, cnt, val)) in entries.iter().enumerate() {
        out.extend_from_slice(&tag.to_le_bytes());
        out.extend_from_slice(&typ.to_le_bytes());
        out.extend_from_slice(&cnt.to_le_bytes());
        if i == 0 {
            out.extend_from_slice(&make_offset.to_le_bytes());
        } else {
            out.extend_from_slice(&val.to_le_bytes());
        }
    }
    out.extend_from_slice(&0u32.to_le_bytes()); // next IFD
    out.extend_from_slice(make);
    out
}

// --- FLAC builder -----------------------------------------------------------

fn flac_block(kind: u8, last: bool, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let header = kind | if last { 0x80 } else { 0 };
    out.push(header);
    let len = data.len();
    out.push((len >> 16) as u8);
    out.push((len >> 8) as u8);
    out.push(len as u8);
    out.extend_from_slice(data);
    out
}

/// A FLAC file with a STREAMINFO block, an optional Vorbis comment, and audio
/// frames. Vorbis comments carry a vendor string and a comment count.
pub fn build_flac(with_vorbis: bool) -> Vec<u8> {
    let mut out = b"fLaC".to_vec();
    // STREAMINFO is mandatory and 34 bytes. It is the last block when no comment
    // follows.
    out.extend_from_slice(&flac_block(0, !with_vorbis, &[0u8; 34]));
    if with_vorbis {
        // vendor length (LE) + vendor + user comment count (LE, zero).
        let vendor = b"reference libFLAC comfyui";
        let mut vc = Vec::new();
        vc.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
        vc.extend_from_slice(vendor);
        vc.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&flac_block(4, true, &vc));
    }
    // Audio frames: a frame sync and placeholder bytes.
    out.extend_from_slice(&[0xFF, 0xF8, 0x69, 0x18, 0x11, 0x22, 0x33, 0x44]);
    out
}

// --- ISOBMFF builders -------------------------------------------------------

fn mp4_box(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&((payload.len() + 8) as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(payload);
    out
}

fn ftyp() -> Vec<u8> {
    let mut p = Vec::new();
    p.extend_from_slice(b"isom");
    p.extend_from_slice(&0u32.to_be_bytes());
    p.extend_from_slice(b"isommp42");
    mp4_box(b"ftyp", &p)
}

fn c2pa_uuid() -> Vec<u8> {
    let mut p = Vec::new();
    p.extend_from_slice(&[0u8; 16]); // usertype UUID
    p.extend_from_slice(b"jumb c2pa manifest store");
    mp4_box(b"uuid", &p)
}

/// An MP4 carrying an ilst atom, for the declined-strip test.
pub fn build_mp4_with_ilst() -> Vec<u8> {
    let ilst = mp4_box(b"ilst", b"\x00\x00\x00\x18\xa9toocomfyui data");
    let mut meta_payload = vec![0u8, 0, 0, 0]; // FullBox version and flags
    meta_payload.extend_from_slice(&ilst);
    let meta = mp4_box(b"meta", &meta_payload);
    let udta = mp4_box(b"udta", &meta);
    let moov = mp4_box(b"moov", &udta);
    let mut out = ftyp();
    out.extend_from_slice(&moov);
    out.extend_from_slice(&mp4_box(b"mdat", &[1, 2, 3, 4, 5, 6, 7, 8]));
    out
}

/// An MP4 with a top-level C2PA uuid box before mdat and an stco table pointing
/// into mdat. Returns the file and the mdat sample payload for verification.
pub fn build_mp4_with_c2pa_uuid() -> (Vec<u8>, Vec<u8>) {
    let sample = vec![0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x11, 0x22];
    let mdat = mp4_box(b"mdat", &sample);
    let build_moov = |stco_offset: u32| -> Vec<u8> {
        let mut stco_payload = vec![0u8, 0, 0, 0];
        stco_payload.extend_from_slice(&1u32.to_be_bytes());
        stco_payload.extend_from_slice(&stco_offset.to_be_bytes());
        let stco = mp4_box(b"stco", &stco_payload);
        let stbl = mp4_box(b"stbl", &stco);
        let minf = mp4_box(b"minf", &stbl);
        let mdia = mp4_box(b"mdia", &minf);
        let trak = mp4_box(b"trak", &mdia);
        mp4_box(b"moov", &trak)
    };
    let uuid = c2pa_uuid();
    let moov_len = build_moov(0).len();
    let mdat_payload_offset = (ftyp().len() + uuid.len() + moov_len + 8) as u32;
    let moov = build_moov(mdat_payload_offset);
    let mut out = ftyp();
    out.extend_from_slice(&uuid);
    out.extend_from_slice(&moov);
    out.extend_from_slice(&mdat);
    (out, sample)
}

// --- A C2PA manifest store writer, test-only ---------------------------------
//
// The crate writes no manifests. These helpers build well-formed stores so the
// capture rule can be exercised against real structure: JUMBF boxes, a claim
// in CBOR that references its assertions, an actions assertion, and a COSE
// signature box carrying a certificate whose subject names an organization.

#[allow(dead_code)]
pub enum Cb {
    U(u64),
    N(u64),
    T(String),
    B(Vec<u8>),
    A(Vec<Cb>),
    M(Vec<(Cb, Cb)>),
    Tag(u64, Box<Cb>),
    Null,
}

#[allow(dead_code)]
fn cb_head(major: u8, n: u64) -> Vec<u8> {
    let mut out = Vec::new();
    if n < 24 {
        out.push((major << 5) | n as u8);
    } else if n <= 0xFF {
        out.push((major << 5) | 24);
        out.push(n as u8);
    } else if n <= 0xFFFF {
        out.push((major << 5) | 25);
        out.extend_from_slice(&(n as u16).to_be_bytes());
    } else {
        out.push((major << 5) | 26);
        out.extend_from_slice(&(n as u32).to_be_bytes());
    }
    out
}

#[allow(dead_code)]
pub fn cbor(v: &Cb) -> Vec<u8> {
    match v {
        Cb::U(n) => cb_head(0, *n),
        Cb::N(n) => cb_head(1, *n),
        Cb::T(t) => {
            let mut o = cb_head(3, t.len() as u64);
            o.extend_from_slice(t.as_bytes());
            o
        }
        Cb::B(b) => {
            let mut o = cb_head(2, b.len() as u64);
            o.extend_from_slice(b);
            o
        }
        Cb::A(items) => {
            let mut o = cb_head(4, items.len() as u64);
            for i in items {
                o.extend(cbor(i));
            }
            o
        }
        Cb::M(pairs) => {
            let mut o = cb_head(5, pairs.len() as u64);
            for (k, v) in pairs {
                o.extend(cbor(k));
                o.extend(cbor(v));
            }
            o
        }
        Cb::Tag(t, inner) => {
            let mut o = cb_head(6, *t);
            o.extend(cbor(inner));
            o
        }
        Cb::Null => vec![0xF6],
    }
}

#[allow(dead_code)]
fn t(s: &str) -> Cb {
    Cb::T(s.to_string())
}

#[allow(dead_code)]
pub fn jbox(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
    out.extend_from_slice(kind);
    out.extend_from_slice(payload);
    out
}

/// A labelled superbox with the C2PA UUID for `role`.
#[allow(dead_code)]
pub fn jumb(role: &[u8; 4], label: &str, content: &[Vec<u8>]) -> Vec<u8> {
    let mut jumd = role.to_vec();
    jumd.extend_from_slice(&[
        0x00, 0x11, 0x00, 0x10, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
    ]);
    jumd.push(0x03);
    jumd.extend_from_slice(label.as_bytes());
    jumd.push(0);
    let mut inner = jbox(b"jumd", &jumd);
    for c in content {
        inner.extend_from_slice(c);
    }
    jbox(b"jumb", &inner)
}

#[allow(dead_code)]
pub struct ActionSpec {
    pub action: &'static str,
    pub source_type: Option<&'static str>,
    pub agent: Option<&'static str>,
}

#[allow(dead_code)]
pub struct ManifestSpec {
    pub label: String,
    pub actions: Vec<ActionSpec>,
    pub signer_org: Option<String>,
    pub claim_generator: String,
    /// Whether the claim references the actions assertion. A store that
    /// carries actions the claim never references is malformed.
    pub reference_actions: bool,
}

#[allow(dead_code)]
impl ManifestSpec {
    pub fn new(label: &str) -> Self {
        ManifestSpec {
            label: label.to_string(),
            actions: Vec::new(),
            signer_org: None,
            claim_generator: "test-writer/0.0".to_string(),
            reference_actions: true,
        }
    }
    pub fn action(
        mut self,
        action: &'static str,
        source_type: Option<&'static str>,
        agent: Option<&'static str>,
    ) -> Self {
        self.actions.push(ActionSpec {
            action,
            source_type,
            agent,
        });
        self
    }
    pub fn signer(mut self, org: &str) -> Self {
        self.signer_org = Some(org.to_string());
        self
    }
    pub fn generator(mut self, g: &str) -> Self {
        self.claim_generator = g.to_string();
        self
    }
    pub fn unreferenced_actions(mut self) -> Self {
        self.reference_actions = false;
        self
    }
}

pub const DIGITAL_CAPTURE_URI: &str =
    "http://cv.iptc.org/newscodes/digitalsourcetype/digitalCapture";
pub const TRAINED_MEDIA_URI: &str =
    "http://cv.iptc.org/newscodes/digitalsourcetype/trainedAlgorithmicMedia";

/// A minimal DER blob: SEQUENCE { SET { SEQUENCE { OID 2.5.4.10, UTF8String org } } }.
#[allow(dead_code)]
fn der_cert(org: &str) -> Vec<u8> {
    let mut attr = vec![0x06, 0x03, 0x55, 0x04, 0x0A, 0x0C, org.len() as u8];
    attr.extend_from_slice(org.as_bytes());
    let seq = [&[0x30, attr.len() as u8][..], &attr].concat();
    let set = [&[0x31, seq.len() as u8][..], &seq].concat();
    [&[0x30, set.len() as u8][..], &set].concat()
}

#[allow(dead_code)]
pub fn manifest_store(specs: &[ManifestSpec]) -> Vec<u8> {
    let mut manifests = Vec::new();
    for spec in specs {
        let mut actions = Vec::new();
        for a in &spec.actions {
            let mut m = vec![(t("action"), t(a.action))];
            if let Some(st) = a.source_type {
                m.push((t("digitalSourceType"), t(st)));
            }
            if let Some(ag) = a.agent {
                m.push((t("softwareAgent"), t(ag)));
            }
            actions.push(Cb::M(m));
        }
        let mut assertion_boxes = Vec::new();
        let mut refs = Vec::new();
        if !spec.actions.is_empty() {
            let body = cbor(&Cb::M(vec![(t("actions"), Cb::A(actions))]));
            assertion_boxes.push(jumb(b"cbor", "c2pa.actions", &[jbox(b"cbor", &body)]));
            if spec.reference_actions {
                refs.push(Cb::M(vec![
                    (t("url"), t("self#jumbf=c2pa.assertions/c2pa.actions")),
                    (t("hash"), Cb::B(vec![0u8; 32])),
                ]));
            }
        }
        let assertion_store = jumb(b"c2as", "c2pa.assertions", &assertion_boxes);
        let claim = cbor(&Cb::M(vec![
            (t("dc:format"), t("image/png")),
            (t("instanceID"), t("xmp:iid:test")),
            (t("claim_generator"), t(&spec.claim_generator)),
            (t("assertions"), Cb::A(refs)),
            (t("signature"), t("self#jumbf=c2pa.signature")),
            (t("alg"), t("sha256")),
        ]));
        let claim_box = jumb(b"c2cl", "c2pa.claim", &[jbox(b"cbor", &claim)]);
        let protected = cbor(&Cb::M(vec![(Cb::U(1), Cb::N(6))]));
        let mut unprotected = Vec::new();
        if let Some(org) = &spec.signer_org {
            unprotected.push((Cb::U(33), Cb::B(der_cert(org))));
        }
        let cose = cbor(&Cb::Tag(
            18,
            Box::new(Cb::A(vec![
                Cb::B(protected),
                Cb::M(unprotected),
                Cb::Null,
                Cb::B(vec![0u8; 64]),
            ])),
        ));
        let signature_box = jumb(b"c2cs", "c2pa.signature", &[jbox(b"cbor", &cose)]);
        manifests.push(jumb(
            b"c2ma",
            &spec.label,
            &[assertion_store, claim_box, signature_box],
        ));
    }
    jumb(b"c2pa", "c2pa", &manifests)
}

/// One manifest carrying a capture claim, signed by `org` when given.
#[allow(dead_code)]
pub fn capture_store(org: Option<&str>) -> Vec<u8> {
    let mut spec = ManifestSpec::new("urn:uuid:capture-1").action(
        "c2pa.created",
        Some(DIGITAL_CAPTURE_URI),
        None,
    );
    if let Some(o) = org {
        spec = spec.signer(o);
    }
    manifest_store(&[spec])
}
