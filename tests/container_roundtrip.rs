//! Container rewrite golden tests. The signal stream must be byte-identical
//! across a metadata strip, and a file without the targeted metadata must come
//! out byte-identical. The RIFF path is tested in both directions because a
//! size or padding bug there destroys the asset.

mod common;
use common::*;
use unmark::asset::{sniff, Format};
use unmark::container::{self, DropSpec};
use unmark::detect;
use unmark::scan::ScanState;

fn all_off_but(f: impl Fn(&mut DropSpec)) -> DropSpec {
    let mut s = DropSpec::default();
    f(&mut s);
    s
}

#[test]
fn png_strip_preserves_idat_and_removes_text() {
    let png = build_png(&PngOpts {
        text: true,
        c2pa: true,
        idat: vec![9, 8, 7, 6, 5, 4, 3, 2, 1],
        ..Default::default()
    });
    let before_sig = container::signal_stream(&png, Format::Png);
    let spec = all_off_but(|s| {
        s.png_text = true;
        s.c2pa = true;
    });
    let out = container::rewrite(&png, Format::Png, &spec).unwrap();
    let after_sig = container::signal_stream(&out, Format::Png);
    assert_eq!(before_sig, after_sig, "IDAT stream changed");
    let det = detect::inspect(&out);
    assert_eq!(
        det.get("png_text").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
    assert_eq!(
        det.get("c2pa").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
}

#[test]
fn png_without_text_is_unchanged() {
    let png = build_png(&PngOpts::default());
    let spec = all_off_but(|s| {
        s.png_text = true;
        s.c2pa = true;
        s.exif = true;
        s.xmp = true;
    });
    let out = container::rewrite(&png, Format::Png, &spec).unwrap();
    assert_eq!(png, out, "a PNG with no targeted metadata was rewritten");
}

#[test]
fn jpeg_strip_preserves_scan() {
    let jpg = build_jpeg(&JpegOpts {
        exif: true,
        xmp: true,
        c2pa: true,
        ..Default::default()
    });
    let before = container::signal_stream(&jpg, Format::Jpeg);
    let spec = all_off_but(|s| {
        s.exif = true;
        s.xmp = true;
        s.c2pa = true;
    });
    let out = container::rewrite(&jpg, Format::Jpeg, &spec).unwrap();
    let after = container::signal_stream(&out, Format::Jpeg);
    assert!(!before.is_empty());
    assert_eq!(before, after, "entropy scan changed");
    let det = detect::inspect(&out);
    assert_eq!(
        det.get("exif").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
}

/// RIFF direction one: stripping the ancillary chunks yields a file whose RIFF
/// size and padding are rebuilt correctly, byte-identical to the same WAV built
/// without those chunks.
#[test]
fn riff_strip_matches_clean_build() {
    let dirty = build_wav(&WavOpts {
        list_info: true,
        id3: true,
        c2pa: true,
        ..Default::default()
    });
    let clean = build_wav(&WavOpts::default());
    let spec = all_off_but(|s| {
        s.riff_ancillary = true;
        s.id3 = true;
        s.c2pa = true;
    });
    let out = container::rewrite(&dirty, Format::RiffWav, &spec).unwrap();
    assert_eq!(out, clean, "stripped WAV does not match a clean build");
    // The RIFF size field equals the body length.
    let declared = u32::from_le_bytes([out[4], out[5], out[6], out[7]]) as usize;
    assert_eq!(declared, out.len() - 8, "RIFF size field not rebuilt");
    assert_eq!(sniff(&out), Format::RiffWav);
}

/// RIFF direction two: a WAV with no ancillary chunks is left byte-identical,
/// so a strip never touches a file that has nothing to strip.
#[test]
fn riff_clean_input_is_unchanged() {
    let clean = build_wav(&WavOpts::default());
    let spec = all_off_but(|s| {
        s.riff_ancillary = true;
        s.id3 = true;
        s.c2pa = true;
    });
    let out = container::rewrite(&clean, Format::RiffWav, &spec).unwrap();
    assert_eq!(clean, out);
}

#[test]
fn riff_odd_length_chunk_padding_is_honored() {
    // An odd-length LIST INFO forces a pad byte. After the strip the data chunk
    // still parses, proving sizes were rebuilt around the padding.
    let wav = build_wav(&WavOpts {
        list_info: true,
        ..Default::default()
    });
    let spec = all_off_but(|s| s.riff_ancillary = true);
    let out = container::rewrite(&wav, Format::RiffWav, &spec).unwrap();
    let sig = container::signal_stream(&out, Format::RiffWav);
    assert_eq!(sig, container::signal_stream(&wav, Format::RiffWav));
}

#[test]
fn flac_vorbis_strip_is_opt_in_and_preserves_audio() {
    let flac = build_flac(true);
    let before = container::signal_stream(&flac, Format::Flac);

    // Default metadata pass preserves Vorbis comments (production data).
    let preserved = container::rewrite(&flac, Format::Flac, &DropSpec::default()).unwrap();
    assert_eq!(
        flac, preserved,
        "a default pass must not remove Vorbis comments"
    );

    // The unlisted-block strip alone leaves the comment, which answers to its
    // own transform, so a kept comment survives it.
    let spec = all_off_but(|s| s.unlisted = true);
    let unlisted_only = container::rewrite(&flac, Format::Flac, &spec).unwrap();
    assert_eq!(
        detect::inspect(&unlisted_only)
            .get("vorbis")
            .map(|d| d.state),
        Some(ScanState::ConfirmedPresent)
    );

    // The Vorbis strip removes the comment and leaves the audio frames identical.
    let spec = all_off_but(|s| s.vorbis = true);
    let out = container::rewrite(&flac, Format::Flac, &spec).unwrap();
    assert_eq!(
        before,
        container::signal_stream(&out, Format::Flac),
        "audio frames changed"
    );
    let det = detect::inspect(&out);
    assert_eq!(
        det.get("vorbis").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
    assert_eq!(sniff(&out), Format::Flac);
}

#[test]
fn webp_strip_preserves_bitstream() {
    let webp = build_webp(&WebpOpts {
        exif: true,
        xmp: true,
        c2pa: true,
    });
    let before = container::signal_stream(&webp, Format::WebP);
    let spec = all_off_but(|s| {
        s.exif = true;
        s.xmp = true;
        s.c2pa = true;
    });
    let out = container::rewrite(&webp, Format::WebP, &spec).unwrap();
    assert_eq!(before, container::signal_stream(&out, Format::WebP));
    let det = detect::inspect(&out);
    assert_eq!(
        det.get("c2pa").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
}
