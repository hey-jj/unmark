//! MPEG audio: the tags strip, the information frame is rewritten in place
//! as its minimal form, the ancillary bytes are zeroed, the referenced
//! frame bytes are untouched, every class has its own keep, and under the
//! audio feature the highpass and re-encode carry the measured delay and
//! padding so the output decodes to the input's length.

use std::path::PathBuf;
use unmark::asset::Format;
use unmark::container;
use unmark::detect;
use unmark::report::{EXIT_INSTRUMENTATION, EXIT_OK};
use unmark::scan::ScanState;
use unmark::{clean, inspect, policy, Options, UnmarkError};

fn fixture(name: &str) -> Vec<u8> {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "fixtures", "mp3", name]
        .iter()
        .collect();
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn pkg() -> policy::PolicyPackage {
    policy::load().unwrap()
}

fn keep(items: &[&str]) -> Options {
    Options {
        keep: items.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    }
}

fn no_degrade() -> Options {
    Options {
        no_degrade: true,
        ..Default::default()
    }
}

fn state(bytes: &[u8], class: &str) -> Option<ScanState> {
    detect::inspect(bytes).get(class).map(|d| d.state)
}

const CLASSES: [&str; 4] = ["id3", "ape", "mp3_info", "mp3_ancillary"];

#[test]
fn mp3_is_a_supported_container_with_four_classes() {
    assert!(Format::Mp3.is_supported_container());
    let tagged = fixture("tagged.mp3");
    let det = detect::inspect(&tagged);
    assert_eq!(det.format, Format::Mp3);
    for class in CLASSES {
        assert_eq!(
            det.get(class).map(|d| d.state),
            Some(ScanState::ConfirmedPresent),
            "{class}"
        );
    }
    let id3 = det.get("id3").unwrap();
    assert!(id3.locations.iter().any(|l| l.container == "ID3v2"));
    assert!(id3.locations.iter().any(|l| l.container == "ID3v1"));
    let info = det.get("mp3_info").unwrap();
    assert!(info.locations[0]
        .detail
        .starts_with("Info tag, encoder LAME"));
    assert!(info.evidence.iter().any(|e| e.starts_with("delay ")));
    let plain = fixture("plain.mp3");
    for class in ["id3", "ape", "mp3_info"] {
        assert_eq!(
            state(&plain, class),
            Some(ScanState::ConfirmedAbsent),
            "{class}"
        );
    }
}

#[test]
fn the_default_run_strips_every_class_and_leaves_the_referenced_bytes() {
    let tagged = fixture("tagged.mp3");
    let before = container::signal_stream(&tagged, Format::Mp3);
    assert!(!before.is_empty());
    let out = clean(&tagged, &no_degrade(), &pkg()).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
    assert_eq!(out.report.output_format, "mp3");
    let bytes = out.output.expect("the run writes");
    assert_eq!(container::signal_stream(&bytes, Format::Mp3), before);
    for class in CLASSES {
        assert_eq!(
            state(&bytes, class),
            Some(ScanState::ConfirmedAbsent),
            "{class}"
        );
    }
    for label in [
        "ID3 tag",
        "APE tag",
        "information frame identity",
        "MPEG ancillary data",
    ] {
        assert!(
            out.report
                .stripped_and_proven_gone
                .iter()
                .any(|l| l == label),
            "{label} not listed: {:?}",
            out.report.stripped_and_proven_gone
        );
    }
    // The information frame is still there, the same size, minimal.
    let l = detect::mp3::layout(&bytes);
    let info = l.info.expect("the frame stays");
    assert_eq!(info.offset, 0);
    assert!(!info.identity);
    assert!(info.frames.is_some() && info.bytes.is_some());
    assert!(info.delay_padding.is_some());
    assert_eq!(info.encoder, None);
    let mc14 = out
        .report
        .actions
        .iter()
        .find(|a| a.transform == "MC14")
        .unwrap();
    assert_eq!(mc14.outcome, "rewritten");
    assert_eq!(mc14.removed, vec!["encoder_identity".to_string()]);
    for field in ["frame_count", "byte_count", "delay", "padding"] {
        assert!(mc14.kept.iter().any(|k| k == field), "{field}");
    }
    let au06 = out
        .report
        .actions
        .iter()
        .find(|a| a.transform == "AU06")
        .unwrap();
    assert!(au06.outcome.contains("kept"), "{}", au06.outcome);
    for class in ["audioseal", "synthid_audio"] {
        let s = out
            .report
            .survived
            .iter()
            .find(|s| s.class == class)
            .unwrap();
        assert!(s.citation.is_some());
    }
    #[cfg(not(feature = "audio"))]
    {
        let out = clean(&tagged, &Options::default(), &pkg()).unwrap();
        let au06 = out
            .report
            .actions
            .iter()
            .find(|a| a.transform == "AU06")
            .unwrap();
        assert_eq!(au06.outcome, "not_attempted");
        assert!(!au06.result.is_empty());
    }
    // The default run strips the same classes.
    let default = clean(&tagged, &Options::default(), &pkg()).unwrap();
    assert_eq!(default.report.exit_code, EXIT_OK);
    let bytes = default.output.unwrap();
    for class in CLASSES {
        assert_eq!(
            state(&bytes, class),
            Some(ScanState::ConfirmedAbsent),
            "{class}"
        );
    }
}

#[test]
fn each_class_has_its_own_keep() {
    let tagged = fixture("tagged.mp3");
    let p = pkg();
    for (name, class, id) in [
        ("id3", "id3", "MC05"),
        ("ape", "ape", "MC13"),
        ("mp3_info", "mp3_info", "MC14"),
        ("MC13", "ape", "MC13"),
        ("rewrite-info-frame", "mp3_info", "MC14"),
        ("MC15", "mp3_ancillary", "MC15"),
        ("mp3_ancillary", "mp3_ancillary", "MC15"),
    ] {
        let mut opts = keep(&[name]);
        opts.no_degrade = true;
        let out = clean(&tagged, &opts, &p).unwrap();
        assert_eq!(
            out.report.exit_code, EXIT_OK,
            "--keep {name}: {:?}",
            out.report.actions
        );
        let bytes = out.output.unwrap();
        assert_eq!(
            state(&bytes, class),
            Some(ScanState::ConfirmedPresent),
            "--keep {name} dropped {class}"
        );
        for other in CLASSES {
            if other != class {
                assert_eq!(
                    state(&bytes, other),
                    Some(ScanState::ConfirmedAbsent),
                    "{other} under --keep {name}"
                );
            }
        }
        assert!(out
            .report
            .kept
            .iter()
            .any(|k| k.item.starts_with(id) && k.reason.contains("kept by flag")));
    }
}

#[test]
fn a_reservoir_dependent_stream_keeps_its_frame_in_place() {
    // The information frame precedes frames that draw from the reservoir;
    // the rewrite never moves or drops it, so nothing breaks.
    let res = fixture("reservoir.mp3");
    let l = detect::mp3::layout(&res);
    let info = l.info.as_ref().expect("an Info frame");
    assert!(!info.identity, "the synthetic frame carries no identity");
    assert_eq!(state(&res, "mp3_info"), Some(ScanState::ConfirmedAbsent));
    let out = clean(&res, &no_degrade(), &pkg()).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
    let bytes = out.output.unwrap();
    assert_eq!(bytes.len(), res.len());
    assert_eq!(
        &bytes[..info.len],
        &res[..info.len],
        "the frame is untouched"
    );
    assert!(bytes.iter().zip(res.iter()).all(|(a, b)| a == b || *a == 0));
    // A LAME-written frame carries identity and is rewritten at its size.
    let tagged = fixture("tagged.mp3");
    let info = detect::mp3::layout(&tagged).info.unwrap();
    assert!(info.identity);
    let spec = container::DropSpec {
        info_identity: true,
        ..Default::default()
    };
    let rewritten = container::rewrite(&tagged, Format::Mp3, &spec).unwrap();
    assert_eq!(rewritten.len(), tagged.len());
    let after = detect::mp3::layout(&rewritten).info.unwrap();
    assert_eq!((after.offset, after.len), (info.offset, info.len));
    assert_eq!(after.frames, info.frames);
    assert_eq!(after.bytes, info.bytes);
    assert_eq!(after.has_toc, info.has_toc);
    assert_eq!(after.delay_padding, info.delay_padding);
    assert!(!after.identity);
    // Deterministic: a second rewrite is a no-op.
    assert_eq!(
        container::rewrite(&rewritten, Format::Mp3, &spec).unwrap(),
        rewritten
    );
}

#[test]
fn a_vbr_xing_frame_keeps_its_table_and_loses_its_identity() {
    let vbr = fixture("vbr.mp3");
    let before = detect::mp3::layout(&vbr).info.unwrap();
    assert!(before.has_toc && before.identity);
    let out = clean(&vbr, &no_degrade(), &pkg()).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK);
    let bytes = out.output.unwrap();
    assert_eq!(state(&bytes, "mp3_info"), Some(ScanState::ConfirmedAbsent));
    let after = detect::mp3::layout(&bytes).info.unwrap();
    assert!(after.has_toc && after.kind == "Xing");
    let mc14 = out
        .report
        .actions
        .iter()
        .find(|a| a.transform == "MC14")
        .unwrap();
    assert!(mc14.kept.iter().any(|k| k == "toc"));
}

#[test]
fn a_truncated_stream_is_malformed_and_fails_closed() {
    let tagged = fixture("tagged.mp3");
    let layout = detect::mp3::layout(&tagged);
    let mut cut = tagged[..layout.frames_end - 7].to_vec();
    cut.extend_from_slice(&tagged[layout.frames_end..]);
    let det = detect::inspect(&cut);
    assert_eq!(
        det.get("mp3_info").map(|d| d.state),
        Some(ScanState::Malformed)
    );
    let r = inspect(&cut, &Options::default(), &pkg()).unwrap();
    assert_eq!(r.exit_code, EXIT_INSTRUMENTATION);
    let err = clean(&cut, &Options::default(), &pkg()).unwrap_err();
    assert!(matches!(err, UnmarkError::Inspection(_)), "{err}");
}

// --- MC15: the ancillary bytes ----------------------------------------------

#[test]
fn the_ancillary_bytes_are_zeroed_by_default_and_kept_under_the_flag() {
    let tagged = fixture("tagged.mp3");
    let p = pkg();
    let count = |b: &[u8]| b.windows(4).filter(|w| w == b"LAME").count();
    assert!(count(&tagged) > 1);
    let anc = detect::inspect(&tagged)
        .get("mp3_ancillary")
        .cloned()
        .unwrap();
    assert_eq!(anc.state, ScanState::ConfirmedPresent);
    assert!(anc.evidence.iter().any(|e| e.starts_with("LAME in ")));
    for opts in [Options::default(), no_degrade()] {
        let out = clean(&tagged, &opts, &p).unwrap();
        assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
        let bytes = out.output.unwrap();
        assert_eq!(count(&bytes), 0, "an encoder name survived in the frames");
        assert_eq!(
            state(&bytes, "mp3_ancillary"),
            Some(ScanState::ConfirmedAbsent)
        );
        assert!(out
            .report
            .stripped_and_proven_gone
            .iter()
            .any(|l| l == "MPEG ancillary data"));
    }
    let scrubbed = container::mp3::scrub_ancillary(&tagged).unwrap();
    assert_eq!(scrubbed.len(), tagged.len());
    let l = detect::mp3::layout(&tagged);
    let mut p = l.frames_start;
    while p < l.frames_end {
        let h = detect::mp3::parse_header(&tagged[p..]).unwrap();
        let head = 4 + if h.crc { 2 } else { 0 } + h.side_info;
        assert_eq!(&scrubbed[p..p + head], &tagged[p..p + head]);
        p += h.len;
    }
    assert!(scrubbed
        .iter()
        .zip(tagged.iter())
        .all(|(a, b)| a == b || *a == 0));
}

#[cfg(feature = "audio")]
#[test]
fn zeroed_and_rewritten_frames_decode_to_the_same_samples() {
    // The acceptance property: a decode of the cleaned file matches a
    // decode of the input sample for sample under --no-degrade, with the
    // delay and padding the rewritten frame carries applied where the
    // input's own extension made the decoder apply them.
    let p = pkg();
    for name in [
        "tagged.mp3",
        "vbr.mp3",
        "plain.mp3",
        "reservoir.mp3",
        "broadband.mp3",
    ] {
        let original = fixture(name);
        let (a, _) = unmark::codec::mp3::decode_with_bitrate(&original).unwrap();
        let out = clean(&original, &no_degrade(), &p).unwrap();
        assert_eq!(
            out.report.exit_code, EXIT_OK,
            "{name}: {:?}",
            out.report.actions
        );
        let cleaned = out.output.unwrap();
        // The decode applies the rewritten frame's delay and padding where
        // the input's own extension made the decoder apply them.
        let (b, _) = unmark::codec::mp3::decode_with_bitrate(&cleaned).unwrap();
        assert_eq!(a.rate, b.rate, "{name}");
        assert_eq!(a.channels.len(), b.channels.len(), "{name}");
        for (ca, cb) in a.channels.iter().zip(b.channels.iter()) {
            assert_eq!(ca.len(), cb.len(), "{name}: sample count changed");
            assert!(
                ca.iter()
                    .zip(cb.iter())
                    .all(|(x, y)| x.to_bits() == y.to_bits()),
                "{name}: the samples differ"
            );
        }
    }
}

// --- Milestone B: the highpass and the re-encode ---------------------------

#[cfg(feature = "audio")]
#[test]
fn the_default_run_highpasses_and_re_encodes_at_the_input_bitrate() {
    let tagged = fixture("tagged.mp3");
    let p = pkg();
    let out = clean(&tagged, &Options::default(), &p).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
    let bytes = out.output.expect("the run writes");
    assert_eq!(out.report.output_format, "mp3");
    let au06 = out
        .report
        .actions
        .iter()
        .find(|a| a.transform == "AU06")
        .unwrap();
    assert_eq!(au06.outcome, "applied");
    assert!(
        au06.result.starts_with("applied at 1500 Hz, LSD "),
        "{}",
        au06.result
    );
    let au03 = out
        .report
        .actions
        .iter()
        .find(|a| a.transform == "AU03")
        .unwrap();
    assert_eq!(au03.outcome, "applied");
    assert!(
        au03.result.contains("128 kbps CBR") && au03.result.contains("second lossy stage"),
        "{}",
        au03.result
    );
    assert_eq!(au03.delay, Some(unmark::codec::mp3::ENCODER_DELAY as u32));
    assert!(au03.padding.is_some());
    let sanity = out.report.sanity.as_ref().expect("the sanity block");
    assert!(sanity.passed);
    assert!(sanity.lsd_db.is_some_and(|v| v > 0.0));
    for class in CLASSES {
        assert_eq!(
            state(&bytes, class),
            Some(ScanState::ConfirmedAbsent),
            "{class}"
        );
    }
    // The output's information frame carries the fields, and no identity.
    let info = detect::mp3::layout(&bytes)
        .info
        .expect("a minimal information frame");
    assert!(!info.identity);
    assert_eq!(
        info.delay_padding,
        Some((au03.delay.unwrap() as u16, au03.padding.unwrap() as u16))
    );
    let (back, kbps) = unmark::codec::mp3::decode_with_bitrate(&bytes).unwrap();
    assert_eq!(kbps, 128);
    assert_eq!(back.channels.len(), 1);
    assert!(!bytes.windows(4).any(|w| w == b"LAME"));
    // Duration: with the fields applied, the output matches the input
    // within one frame, and a second default run stays within one frame.
    let (input, _) = unmark::codec::mp3::decode_with_bitrate(&tagged).unwrap();
    let (raw, _) = unmark::codec::mp3::decode_raw_with_bitrate(&bytes).unwrap();
    let (d, pd) = info.delay_padding.unwrap();
    let trimmed = unmark::codec::mp3::apply_delay_padding(&raw, d, pd);
    assert_eq!(trimmed.frames(), back.frames());
    assert!(
        trimmed.frames().abs_diff(input.frames()) <= 1152,
        "{} vs {}",
        trimmed.frames(),
        input.frames()
    );
    let again = clean(&bytes, &Options::default(), &p).unwrap();
    assert_eq!(
        again.report.exit_code, EXIT_OK,
        "{:?}",
        again.report.actions
    );
    let second = again.output.unwrap();
    assert!(detect::mp3::layout(&second)
        .info
        .unwrap()
        .delay_padding
        .is_some());
    let (trimmed2, _) = unmark::codec::mp3::decode_with_bitrate(&second).unwrap();
    assert!(
        trimmed2.frames().abs_diff(input.frames()) <= 1152,
        "{} vs {}",
        trimmed2.frames(),
        input.frames()
    );
    // Under --no-degrade the frames are the input's referenced bytes.
    let nd = clean(&tagged, &no_degrade(), &p).unwrap();
    let input_info = detect::mp3::layout(&tagged).info.unwrap();
    assert_eq!(
        nd.output.unwrap().len(),
        container::signal_stream(&tagged, Format::Mp3).len() + input_info.len
    );
    assert!(nd.report.sanity.is_none());
    assert!(nd.report.kept.iter().any(|k| k.item.starts_with("AU06")));
}

#[cfg(feature = "audio")]
#[test]
fn a_variable_rate_input_is_re_encoded_at_its_average() {
    let p = pkg();
    let vbr = fixture("vbr.mp3");
    let out = clean(&vbr, &Options::default(), &p).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
    let au03 = out
        .report
        .actions
        .iter()
        .find(|a| a.transform == "AU03")
        .unwrap();
    let (_, input_kbps) = unmark::codec::mp3::decode_with_bitrate(&vbr).unwrap();
    let pinned: u32 = au03
        .result
        .split(", ")
        .nth(1)
        .and_then(|s| s.split(' ').next())
        .and_then(|n| n.parse().ok())
        .expect("a bitrate in the result");
    assert!(au03.result.contains("kbps CBR") && au03.result.contains("second lossy stage"));
    assert!(
        pinned.abs_diff(input_kbps) <= 8,
        "pinned {pinned} vs input average {input_kbps}"
    );
    let (_, out_kbps) =
        unmark::codec::mp3::decode_with_bitrate(out.output.as_ref().unwrap()).unwrap();
    assert_eq!(out_kbps, pinned);
}

// --- M1 and M2: aligned LSD on broadband content, and a kept frame -----------

#[cfg(feature = "audio")]
#[test]
fn a_lame_encoded_broadband_input_passes_the_default_run_with_the_lsd_aligned() {
    let wide = fixture("broadband.mp3");
    let p = pkg();
    let out = clean(&wide, &Options::default(), &p).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
    let sanity = out.report.sanity.as_ref().unwrap();
    let lsd = sanity.lsd_db.unwrap();
    assert!(sanity.passed, "LSD {lsd}");
    assert!(lsd > 1.0 && lsd < 4.0, "LSD {lsd}");
    // The lag a search finds is the delay the frame carries plus the
    // decoder's own, and an unaligned measure reads several dB.
    let (audio, kbps) = unmark::codec::mp3::decode_with_bitrate(&wide).unwrap();
    let processed = unmark::transform::audio::apply(
        &audio,
        &unmark::transform::audio::AudioParams {
            highpass_hz: Some(1500.0),
        },
    );
    let enc = unmark::codec::mp3::encode(&processed, kbps).unwrap();
    let (back, _) = unmark::codec::mp3::decode_raw_with_bitrate(&enc.bytes).unwrap();
    let lag = unmark::budget::align_lag(&processed.channels[0], &back.channels[0], 2304, 16384);
    assert_eq!(lag, enc.delay as usize + unmark::codec::mp3::DECODER_DELAY);
    let n = processed.channels[0]
        .len()
        .min(back.channels[0].len() - lag);
    let unaligned = unmark::budget::lsd(
        &processed.channels[0][..n],
        &back.channels[0][..n],
        &unmark::budget::LsdParams::PINNED,
    )
    .unwrap();
    assert!(
        unaligned > lsd + 2.0,
        "unaligned {unaligned} vs aligned {lsd}"
    );
}

#[cfg(feature = "audio")]
#[test]
fn a_kept_frame_class_stands_the_audio_path_down() {
    let tagged = fixture("tagged.mp3");
    let count = |b: &[u8]| b.windows(4).filter(|w| w == b"LAME").count();
    let p = pkg();
    for (name, class) in [
        ("MC15", "mp3_ancillary"),
        ("mp3_ancillary", "mp3_ancillary"),
        ("mp3_info", "mp3_info"),
    ] {
        let out = clean(&tagged, &keep(&[name]), &p).unwrap();
        assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
        let bytes = out.output.expect("the strips still write");
        assert!(
            count(&bytes) >= 1,
            "--keep {name}: the kept bytes were discarded"
        );
        assert_eq!(state(&bytes, class), Some(ScanState::ConfirmedPresent));
        for id in ["AU06", "AU03"] {
            let a = out
                .report
                .actions
                .iter()
                .find(|a| a.transform == id)
                .unwrap();
            assert_eq!(a.outcome, "not applied", "{id}");
            assert!(a.result.contains("stands down"), "{}", a.result);
        }
        assert!(out.report.sanity.is_none());
    }
}

/// The acceptance property through ffmpeg when it is on the path: its
/// decode of a --no-degrade output equals its decode of the input bit for
/// bit once aligned by the delay and padding the frame carries. ffmpeg runs
/// with its SIMD paths off: with them on, its own trimmed decode of the
/// input converts the partial first and last frames through a different
/// rounding path than the full frames, a few values off by one, which is
/// ffmpeg's conversion and not the file.
#[cfg(feature = "audio")]
#[test]
fn ffmpeg_decodes_a_no_degrade_output_to_the_input_samples_once_aligned() {
    let Ok(ffmpeg) = std::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
    else {
        eprintln!("ffmpeg not on the path; the ffmpeg leg of the acceptance test is skipped");
        return;
    };
    if !ffmpeg.status.success() {
        return;
    }
    let scratch = std::env::temp_dir().join(format!("unmark-ffmpeg-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let decode = |path: &std::path::Path| -> Vec<i16> {
        let out = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-nostdin", "-cpuflags", "0", "-i"])
            .arg(path)
            .args(["-f", "s16le", "-acodec", "pcm_s16le", "-"])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        out.stdout
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect()
    };
    for name in ["tagged.mp3", "vbr.mp3", "broadband.mp3"] {
        let input = fixture(name);
        let out = clean(&input, &no_degrade(), &pkg()).unwrap();
        let cleaned = out.output.unwrap();
        let in_path = scratch.join(name);
        let out_path = scratch.join(format!("cleaned-{name}"));
        std::fs::write(&in_path, &input).unwrap();
        std::fs::write(&out_path, &cleaned).unwrap();
        let a = decode(&in_path);
        let b = decode(&out_path);
        let channels = unmark::codec::mp3::decode_with_bitrate(&input)
            .unwrap()
            .0
            .channels
            .len();
        // Where the input carried an identity, ffmpeg trimmed it and the
        // output is the untrimmed stream: longer by the delay plus the
        // decoder's own at the head and the padding less that at the tail.
        let info = detect::mp3::layout(&cleaned).info;
        let (head, tail) = match info.as_ref().and_then(|i| i.delay_padding) {
            Some((d, p))
                if detect::mp3::layout(&input)
                    .info
                    .as_ref()
                    .is_some_and(|i| i.identity) =>
            {
                (
                    (d as usize + unmark::codec::mp3::DECODER_DELAY) * channels,
                    (p as usize).saturating_sub(unmark::codec::mp3::DECODER_DELAY) * channels,
                )
            }
            _ => (0, 0),
        };
        assert_eq!(b.len(), a.len() + head + tail, "{name}: length");
        assert!(
            a.iter().zip(b[head..].iter()).all(|(x, y)| x == y),
            "{name}: ffmpeg's decodes differ once aligned"
        );
    }
    let _ = std::fs::remove_dir_all(&scratch);
}

/// The restated gapless acceptance, through nanomp3: a decode of the output
/// after applying the frame's delay and padding fields matches the input
/// sample for sample under --no-degrade, and an untrimmed decode is longer by
/// exactly delay plus padding, as the frame carries them on the rewrite path
/// and as the AU03 row carries them on the re-encode path.
#[cfg(feature = "audio")]
#[test]
fn an_untrimmed_decode_is_longer_by_exactly_delay_plus_padding() {
    let p = pkg();
    for name in ["tagged.mp3", "vbr.mp3", "broadband.mp3", "mpeg2.mp3"] {
        let input = fixture(name);
        let (reference, _) = unmark::codec::mp3::decode_with_bitrate(&input).unwrap();
        // The rewrite path.
        let out = clean(&input, &no_degrade(), &p).unwrap();
        let cleaned = out.output.unwrap();
        let (applied, _) = unmark::codec::mp3::decode_with_bitrate(&cleaned).unwrap();
        assert_eq!(
            applied.channels, reference.channels,
            "{name}: samples differ after the fields"
        );
        let (raw, _) = unmark::codec::mp3::decode_raw_with_bitrate(&cleaned).unwrap();
        if let Some((d, pd)) = detect::mp3::layout(&cleaned)
            .info
            .and_then(|i| i.delay_padding)
        {
            assert_eq!(
                raw.frames(),
                applied.frames() + d as usize + pd as usize,
                "{name}: rewrite length"
            );
        } else {
            assert_eq!(raw.frames(), applied.frames(), "{name}: no fields, no trim");
        }
        // The re-encode path.
        let out = clean(&input, &Options::default(), &p).unwrap();
        assert_eq!(
            out.report.exit_code, EXIT_OK,
            "{name}: {:?}",
            out.report.actions
        );
        let au03 = out
            .report
            .actions
            .iter()
            .find(|a| a.transform == "AU03")
            .unwrap();
        let (d, pd) = (au03.delay.unwrap() as usize, au03.padding.unwrap() as usize);
        let written = out.output.unwrap();
        let (applied, _) = unmark::codec::mp3::decode_with_bitrate(&written).unwrap();
        let (raw, _) = unmark::codec::mp3::decode_raw_with_bitrate(&written).unwrap();
        assert_eq!(
            raw.frames(),
            applied.frames() + d + pd,
            "{name}: re-encode length"
        );
        assert_eq!(
            applied.frames(),
            reference.frames(),
            "{name}: the re-encode decodes to the input's length"
        );
        assert!(
            out.report.sanity.as_ref().unwrap().lsd_db.is_some(),
            "{name}: LSD reported"
        );
    }
}

/// A LAME-encoded MPEG-2 input runs at exit 0 with its distance reported as
/// data; the refusals are structural only.
#[cfg(feature = "audio")]
#[test]
fn an_mpeg2_input_runs_at_exit_0_with_its_lsd_reported() {
    let mpeg2 = fixture("mpeg2.mp3");
    let h = detect::mp3::parse_header(&mpeg2[detect::mp3::layout(&mpeg2).frames_start..]).unwrap();
    assert_eq!(h.version, detect::mp3::Version::Mpeg2);
    let out = clean(&mpeg2, &Options::default(), &pkg()).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
    let sanity = out.report.sanity.as_ref().unwrap();
    assert!(sanity.passed && sanity.refusal.is_none());
    let lsd = sanity.lsd_db.expect("LSD as data");
    assert!(lsd > 0.0 && lsd < 20.0, "LSD {lsd}");
    let au03 = out
        .report
        .actions
        .iter()
        .find(|a| a.transform == "AU03")
        .unwrap();
    assert!(au03.result.contains("64 kbps CBR"), "{}", au03.result);
    let (back, kbps) =
        unmark::codec::mp3::decode_with_bitrate(out.output.as_ref().unwrap()).unwrap();
    assert_eq!((back.rate, kbps), (22050, 64));
}
