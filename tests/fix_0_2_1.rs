//! The 0.2.1 fixes, each pinned by the probe that found it: input identity
//! and atomic output on the command line, refusals for inputs the metrics
//! cannot score, failed walkers failing inspect and plan, the no-op report,
//! unlisted chunks and segments stripped by default, the WebP VP8X flags,
//! element-level generator stripping, opt-outs that still write, verify
//! refusing unreadable replacements, and batch paths.

mod common;
use common::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use unmark::asset::Format;
use unmark::report::{EXIT_INSTRUMENTATION, EXIT_OK, EXIT_UNSUPPORTED, EXIT_USAGE};
use unmark::scan::ScanState;
use unmark::{clean, inspect, plan, policy, Options, UnmarkError, VerifyOutcome};

fn pkg() -> policy::PolicyPackage {
    policy::load().unwrap()
}

fn fixture(name: &str) -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "fixtures", name]
        .iter()
        .collect()
}

fn keep(items: &[&str]) -> Options {
    Options {
        keep: items.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("unmark-021-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_unmark"))
        .args(args)
        .output()
        .expect("the unmark binary runs");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

// --- S3: inputs the metrics cannot score are refused ------------------------

#[test]
fn empty_input_is_unsupported_for_every_verb() {
    let p = pkg();
    for r in [
        inspect(&[], &Options::default(), &p),
        plan(&[], &Options::default(), &p),
    ] {
        assert!(matches!(r, Err(UnmarkError::Unsupported(_))));
    }
    assert!(matches!(
        clean(&[], &Options::default(), &p),
        Err(UnmarkError::Unsupported(_))
    ));
}

#[test]
fn an_image_under_the_minimum_edge_is_refused_and_a_scorable_one_is_measured() {
    let p = pkg();
    let tiny = unmark::codec::Image {
        width: 4,
        height: 4,
        channels: 3,
        data: vec![100; 4 * 4 * 3],
    };
    let png = unmark::codec::png::encode(&tiny).unwrap();
    let err = clean(&png, &Options::default(), &p).unwrap_err();
    assert!(matches!(err, UnmarkError::Unsupported(_)), "{err}");
    let wide = unmark::codec::Image {
        width: 200,
        height: 1,
        channels: 3,
        data: (0..200 * 3).map(|i| (i % 251) as u8).collect(),
    };
    let png = unmark::codec::png::encode(&wide).unwrap();
    assert!(matches!(
        clean(&png, &Options::default(), &p),
        Err(UnmarkError::Unsupported(_))
    ));
    let ok = unmark::codec::Image {
        width: unmark::MIN_IMAGE_EDGE,
        height: unmark::MIN_IMAGE_EDGE,
        channels: 3,
        data: (0..unmark::MIN_IMAGE_EDGE * unmark::MIN_IMAGE_EDGE * 3)
            .map(|i| (i * 7 % 251) as u8)
            .collect(),
    };
    let png = unmark::codec::png::encode(&ok).unwrap();
    let out = clean(&png, &Options::default(), &p).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
    let sanity = out.report.sanity.unwrap();
    assert!(sanity.psnr_db.is_some() && sanity.ssim.is_some() && sanity.passed);
}

#[cfg(feature = "audio")]
#[test]
fn audio_without_samples_is_refused() {
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&36u32.to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&44100u32.to_le_bytes());
    wav.extend_from_slice(&88200u32.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&0u32.to_le_bytes());
    let err = clean(&wav, &Options::default(), &pkg()).unwrap_err();
    assert!(matches!(err, UnmarkError::Unsupported(_)), "{err}");
}

// --- S4: a walker that did not complete fails inspect and plan --------------

#[test]
fn malformed_containers_fail_inspect_and_plan_with_exit_30() {
    let p = pkg();
    for name in [
        "malformed.png",
        "malformed.jpg",
        "malformed.wav",
        "malformed.flac",
        "malformed.mp4",
    ] {
        let bytes = std::fs::read(fixture(name)).unwrap();
        let r = inspect(&bytes, &Options::default(), &p).unwrap();
        assert_eq!(r.exit_code, EXIT_INSTRUMENTATION, "{name} inspect");
        let r = plan(&bytes, &Options::default(), &p).unwrap();
        assert_eq!(r.exit_code, EXIT_INSTRUMENTATION, "{name} plan");
    }
    let good = std::fs::read(fixture("generated.png")).unwrap();
    assert_eq!(
        inspect(&good, &Options::default(), &p).unwrap().exit_code,
        EXIT_OK
    );
}

// --- S5: a run that changes nothing says so --------------------------------

#[test]
fn an_unchanged_output_is_reported_as_a_no_op() {
    let out = clean(b"plain text\n", &Options::default(), &pkg()).unwrap();
    assert!(out.report.no_op);
    assert_eq!(out.output.as_deref(), Some(&b"plain text\n"[..]));
    assert!(out
        .report
        .actions
        .iter()
        .all(|a| a.outcome == "no-op" || a.outcome == "proposed" || a.outcome.contains("kept")));
    assert!(out
        .report
        .actions
        .iter()
        .any(|a| a.result.contains("nothing changed")));
    let changed = clean("a\u{200B}b\n".as_bytes(), &Options::default(), &pkg()).unwrap();
    assert!(!changed.report.no_op);
    let text = unmark::report::render_text(&out.report);
    assert!(text.contains("no-op"));
}

// --- S6 and G1: unlisted chunks and segments strip by default ---------------

#[cfg(feature = "audio")]
fn wav_with_chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut wav = build_wav(&WavOpts::default());
    wav.extend_from_slice(id);
    wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
    wav.extend_from_slice(data);
    if data.len() % 2 == 1 {
        wav.push(0);
    }
    let riff_len = (wav.len() - 8) as u32;
    wav[4..8].copy_from_slice(&riff_len.to_le_bytes());
    wav
}

#[cfg(feature = "audio")]
#[test]
fn a_bext_description_strips_by_default_and_stays_under_keep() {
    let mark = b"generated by an llm";
    let mut bext = mark.to_vec();
    bext.resize(602, 0);
    let wav = wav_with_chunk(b"bext", &bext);
    let p = pkg();
    for opts in [
        Options::default(),
        Options {
            no_degrade: true,
            ..Default::default()
        },
    ] {
        let out = clean(&wav, &opts, &p).unwrap();
        assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
        let bytes = out.output.unwrap();
        assert!(
            !contains(&bytes, mark),
            "the bext description survived the default run"
        );
        assert!(!contains(&bytes, b"bext"));
    }
    let out = clean(&wav, &keep(&["MC06"]), &p).unwrap();
    let bytes = out.output.unwrap();
    assert!(contains(&bytes, mark), "--keep MC06 dropped the chunk");
    assert!(out.report.kept.iter().any(|k| k.item.starts_with("MC06")));
}

fn jpeg_with_segments(extra: &[(u8, &[u8])]) -> Vec<u8> {
    let base = build_jpeg(&JpegOpts::default());
    let mut out = base[..2].to_vec();
    for (marker, data) in extra {
        out.push(0xFF);
        out.push(*marker);
        out.extend_from_slice(&((data.len() + 2) as u16).to_be_bytes());
        out.extend_from_slice(data);
    }
    out.extend_from_slice(&base[2..]);
    out
}

#[test]
fn jpeg_comment_and_app15_generator_marks_strip_by_default() {
    let jpg = jpeg_with_segments(&[(0xFE, b"AI-MARK-JPEG-COM"), (0xEF, b"AI-MARK-JPEG-APP15")]);
    let p = pkg();
    let out = clean(&jpg, &Options::default(), &p).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
    let bytes = out.output.unwrap();
    assert!(!contains(&bytes, b"AI-MARK-JPEG-COM"));
    assert!(!contains(&bytes, b"AI-MARK-JPEG-APP15"));
    // The strip is a container rewrite: with --no-degrade the scan is
    // byte-identical and the segments are still gone.
    let out = clean(
        &jpg,
        &Options {
            no_degrade: true,
            ..Default::default()
        },
        &p,
    )
    .unwrap();
    let bytes = out.output.unwrap();
    assert!(!contains(&bytes, b"AI-MARK-JPEG-COM"));
    assert_eq!(
        unmark::container::signal_stream(&bytes, Format::Jpeg),
        unmark::container::signal_stream(&jpg, Format::Jpeg)
    );
    let out = clean(&jpg, &keep(&["MC06", "PX02", "PX01"]), &p).unwrap();
    assert!(contains(&out.output.unwrap(), b"AI-MARK-JPEG-COM"));
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

// --- G2: a WebP whose VP8X advertised the dropped chunks still decodes ------

#[test]
fn a_webp_with_vp8x_metadata_flags_cleans_by_default_and_decodes() {
    let img = unmark::codec::Image {
        width: 32,
        height: 24,
        channels: 3,
        data: (0..32 * 24 * 3).map(|i| (i * 13 % 251) as u8).collect(),
    };
    let webp = unmark::codec::webp::encode_lossless_with(
        &img,
        Some(camera_tiff()),
        Some(b"<x:xmpmeta><xmp:CreatorTool>ComfyUI</xmp:CreatorTool></x:xmpmeta>".to_vec()),
        None,
    )
    .unwrap();
    let before = unmark::detect::inspect(&webp);
    assert_eq!(
        before.get("xmp").map(|d| d.state),
        Some(ScanState::ConfirmedPresent)
    );
    let p = pkg();
    let out = clean(&webp, &Options::default(), &p).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
    let bytes = out.output.expect("the default run writes the WebP");
    assert!(unmark::codec::decode_image(&bytes, Format::WebP).is_ok());
    let after = unmark::detect::inspect(&bytes);
    for class in ["exif", "xmp"] {
        assert_eq!(
            after.get(class).map(|d| d.state),
            Some(ScanState::ConfirmedAbsent)
        );
    }
    // The metadata-only rewrite clears the VP8X flags it orphaned.
    let spec = unmark::container::DropSpec {
        exif: true,
        xmp: true,
        ..Default::default()
    };
    let stripped = unmark::container::rewrite(&webp, Format::WebP, &spec).unwrap();
    let (chunks, _) = unmark::detect::riff::chunks(&stripped);
    let vp8x = chunks.iter().find(|c| &c.id == b"VP8X").expect("VP8X kept");
    assert_eq!(vp8x.data[0] & 0x0C, 0, "EXIF or XMP flag left set");
    assert!(unmark::codec::decode_image(&stripped, Format::WebP).is_ok());
}

// --- G3: generator elements go, content stays ------------------------------

#[test]
fn html_generator_meta_is_cut_in_either_quoting_and_the_body_stays() {
    let p = pkg();
    let single = "<!doctype html>\n<html><head><meta name='generator' content='ComfyUI-PROBE'></head><body>KEEP VISIBLE</body></html>\n";
    let double = "<!doctype html>\n<html><head><meta name=\"generator\" content=\"ComfyUI-PROBE\"></head><body>KEEP VISIBLE</body></html>\n";
    for text in [single, double] {
        let out = clean(text.as_bytes(), &Options::default(), &p).unwrap();
        assert_eq!(out.report.exit_code, EXIT_OK);
        let got = String::from_utf8(out.output.unwrap()).unwrap();
        assert_eq!(
            got,
            "<!doctype html>\n<html><head></head><body>KEEP VISIBLE</body></html>\n"
        );
    }
    // A generator comment inside a line leaves the rest of the line; one
    // on its own line takes the line.
    let inline = "<p>x</p><!-- Generator: ComfyUI --><p>y</p>\n";
    let out = clean(inline.as_bytes(), &Options::default(), &p).unwrap();
    assert_eq!(
        String::from_utf8(out.output.unwrap()).unwrap(),
        "<p>x</p><p>y</p>\n"
    );
    let banner = "# Generator: ComfyUI-PROBE\nHello world.\n";
    let out = clean(banner.as_bytes(), &Options::default(), &p).unwrap();
    assert_eq!(
        String::from_utf8(out.output.unwrap()).unwrap(),
        "Hello world.\n"
    );
    // A prose line that mentions a generator is content and stays.
    let prose = "The generator: a small script.\n";
    let out = clean(prose.as_bytes(), &Options::default(), &p).unwrap();
    assert_eq!(String::from_utf8(out.output.unwrap()).unwrap(), prose);
}

// --- P1: an opt-out still writes -------------------------------------------

#[cfg(feature = "image")]
#[test]
fn opting_out_of_the_dwtdct_removal_still_writes_the_other_strips() {
    let marked = std::fs::read(fixture("efficacy/small-sdxl.png")).unwrap();
    let p = pkg();
    for opts in [
        keep(&["dwtdct"]),
        Options {
            no_degrade: true,
            ..Default::default()
        },
    ] {
        let out = clean(&marked, &opts, &p).unwrap();
        assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
        assert!(out.output.is_some(), "the opt-out wrote nothing");
        assert!(out.report.kept.iter().any(|k| k.item.starts_with("PX02")));
        assert!(!out
            .report
            .stripped_and_proven_gone
            .iter()
            .any(|l| l == "dwtDct pixel mark"));
    }
}

// --- P2: verify refuses a replacement it cannot read ------------------------

#[test]
fn verify_refuses_unsupported_malformed_and_wrong_format_outputs() {
    let png = std::fs::read(fixture("generated.png")).unwrap();
    let p = pkg();
    let out = clean(&png, &Options::default(), &p).unwrap();
    let report = serde_json::to_string(&out.report).unwrap();
    assert_eq!(
        unmark::verify(&out.output.unwrap(), &report, &p),
        VerifyOutcome::Verified
    );
    for (name, bytes) in [
        ("blob", vec![0xC0u8; 32]),
        (
            "malformed",
            std::fs::read(fixture("malformed.png")).unwrap(),
        ),
        ("text", b"plain text\n".to_vec()),
    ] {
        match unmark::verify(&bytes, &report, &p) {
            VerifyOutcome::Mismatch(problems) => assert!(!problems.is_empty(), "{name}"),
            VerifyOutcome::Verified => panic!("{name} verified against a PNG report"),
        }
    }
}

// --- S1, S2, S5, P3, P4: the command line ----------------------------------

#[test]
fn an_output_that_names_the_input_is_refused_and_the_input_is_untouched() {
    let dir = scratch("identity");
    let src = dir.join("same.txt");
    std::fs::write(&src, "a\u{200B}b\n").unwrap();
    let before = std::fs::read(&src).unwrap();
    let (code, _, err) = run(&["clean", "--overwrite", "--out", s(&src), s(&src)]);
    assert_eq!(code, EXIT_USAGE, "{err}");
    assert!(err.contains("names the input"));
    assert_eq!(std::fs::read(&src).unwrap(), before);
    #[cfg(unix)]
    {
        let link = dir.join("link.txt");
        std::os::unix::fs::symlink("same.txt", &link).unwrap();
        let (code, _, _) = run(&["clean", "--overwrite", "--out", s(&link), s(&src)]);
        assert_eq!(code, EXIT_USAGE);
        assert_eq!(std::fs::read(&src).unwrap(), before);
        let hard = dir.join("hard.txt");
        std::fs::hard_link(&src, &hard).unwrap();
        let (code, _, _) = run(&["clean", "--overwrite", "--out", s(&hard), s(&src)]);
        assert_eq!(code, EXIT_USAGE);
        assert_eq!(std::fs::read(&src).unwrap(), before);
    }
}

#[cfg(unix)]
#[test]
fn a_failed_write_leaves_no_partial_output() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("partial");
    let src = dir.join("in.txt");
    std::fs::write(&src, "a\u{200B}b\n").unwrap();
    let ro = dir.join("ro");
    std::fs::create_dir_all(&ro).unwrap();
    std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o555)).unwrap();
    let dest = ro.join("out.txt");
    let (code, _, err) = run(&["clean", "--out", s(&dest), s(&src)]);
    let writable = std::fs::write(ro.join("probe"), b"x").is_ok();
    std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o755)).unwrap();
    if writable {
        // Running with permissions that ignore the mode; the refusal path
        // cannot be exercised here.
        return;
    }
    assert_eq!(code, EXIT_INSTRUMENTATION, "{err}");
    let left: Vec<_> = std::fs::read_dir(&ro).unwrap().collect();
    assert!(left.is_empty(), "partial or temporary output left behind");
}

#[test]
fn a_batch_no_op_writes_nothing_and_a_single_out_writes_a_copy() {
    let dir = scratch("noop");
    let src = dir.join("plain.txt");
    std::fs::write(&src, "plain text\n").unwrap();
    let copy = dir.join("copy.txt");
    let (code, out, _) = run(&["clean", "--out", s(&copy), s(&src)]);
    assert_eq!(code, EXIT_OK);
    assert_eq!(std::fs::read(&copy).unwrap(), b"plain text\n");
    let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(v["no_op"], true);
    let inputs = dir.join("in");
    std::fs::create_dir_all(&inputs).unwrap();
    std::fs::copy(&src, inputs.join("plain.txt")).unwrap();
    let outputs = dir.join("out");
    let (code, out, err) = run(&["clean", "--out", s(&outputs), s(&inputs)]);
    assert_eq!(code, EXIT_OK, "{err}");
    assert!(!outputs.join("plain.txt").exists(), "a no-op wrote a file");
    assert!(err.contains("no-op"));
    assert_eq!(out.lines().count(), 1);
}

#[test]
fn directories_expand_recursively_and_every_input_gets_a_report() {
    let dir = scratch("batch");
    let inputs = dir.join("in");
    std::fs::create_dir_all(inputs.join("nested")).unwrap();
    std::fs::copy(fixture("generated.png"), inputs.join("a.png")).unwrap();
    std::fs::copy(fixture("malformed.png"), inputs.join("bad.png")).unwrap();
    std::fs::write(inputs.join("blob.bin"), [0xC0u8; 16]).unwrap();
    std::fs::copy(fixture("generated.png"), inputs.join("nested/deep.png")).unwrap();
    let outputs = dir.join("out");
    let (code, out, err) = run(&["clean", "--out", s(&outputs), s(&inputs)]);
    assert_eq!(code, EXIT_UNSUPPORTED, "{err}");
    let reports: Vec<serde_json::Value> = out
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(reports.len(), 4, "one report per input: {out}");
    let by_input = |suffix: &str| {
        reports
            .iter()
            .find(|r| r["input"].as_str().unwrap().ends_with(suffix))
            .unwrap_or_else(|| panic!("no report for {suffix}"))
    };
    assert_eq!(by_input("a.png")["exit_code"], 0);
    assert_eq!(by_input("bad.png")["exit_code"], EXIT_INSTRUMENTATION);
    assert!(by_input("bad.png")["error"].is_string());
    assert_eq!(by_input("blob.bin")["exit_code"], EXIT_UNSUPPORTED);
    assert_eq!(by_input("nested/deep.png")["exit_code"], 0);
    assert!(outputs.join("a.png").exists());
    assert!(
        outputs.join("nested/deep.png").exists(),
        "the nested tree is mirrored"
    );
    assert!(!outputs.join("bad.png").exists());
    assert!(!outputs.join("blob.bin").exists());
    // A directory holding one file is still a batch.
    let one = dir.join("one");
    std::fs::create_dir_all(&one).unwrap();
    std::fs::copy(fixture("generated.png"), one.join("only.png")).unwrap();
    let one_out = dir.join("one-out");
    let (code, _, err) = run(&["clean", "--out", s(&one_out), s(&one)]);
    assert_eq!(code, EXIT_OK, "{err}");
    assert!(one_out.join("only.png").exists());
}

#[test]
fn colliding_outputs_are_refused_and_the_first_survives() {
    let dir = scratch("collide");
    let a = dir.join("a");
    let b = dir.join("b");
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir_all(&b).unwrap();
    std::fs::write(a.join("same.txt"), "a\u{200B}b\n").unwrap();
    std::fs::write(b.join("same.txt"), "c\u{200B}d\n").unwrap();
    let outputs = dir.join("out");
    let (code, out, err) = run(&[
        "clean",
        "--out",
        s(&outputs),
        s(&a.join("same.txt")),
        s(&b.join("same.txt")),
    ]);
    assert_eq!(code, EXIT_USAGE, "{err}");
    assert!(err.contains("already written"));
    assert_eq!(std::fs::read(outputs.join("same.txt")).unwrap(), b"ab\n");
    let reports: Vec<serde_json::Value> = out
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[1]["exit_code"], EXIT_USAGE);
    // Two directory arguments mirror their trees, so the same name in each
    // lands in its own place.
    let outputs = dir.join("out2");
    let (code, _, err) = run(&["clean", "--out", s(&outputs), s(&a), s(&b)]);
    assert_eq!(code, EXIT_OK, "{err}");
    assert_eq!(std::fs::read(outputs.join("a/same.txt")).unwrap(), b"ab\n");
    assert_eq!(std::fs::read(outputs.join("b/same.txt")).unwrap(), b"cd\n");
}
