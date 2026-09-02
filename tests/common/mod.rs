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
    pub idat: Vec<u8>,
}

impl Default for PngOpts {
    fn default() -> Self {
        PngOpts {
            text: false,
            exif: false,
            xmp: false,
            c2pa: false,
            idat: vec![1, 2, 3, 4, 5, 6, 7, 8],
        }
    }
}

pub fn build_png(o: &PngOpts) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']);
    // IHDR: 1x1 RGB.
    let ihdr = [0, 0, 0, 1, 0, 0, 0, 1, 8, 2, 0, 0, 0];
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
        out.extend_from_slice(&png_chunk(b"caBX", b"jumb\0\0\0\0c2pa manifest store"));
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
        out.extend_from_slice(&jpeg_app(0xEB, payload));
    }
    // A tiny entropy-coded scan and end of image.
    out.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00]);
    out.extend_from_slice(&[0x12, 0x34, 0x56, 0x78]);
    out.extend_from_slice(&[0xFF, 0xD9]);
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
