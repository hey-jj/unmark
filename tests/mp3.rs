//! MPEG audio, milestone A: the tags strip, the information frame strips
//! only when the next frame draws nothing from the reservoir, the audio
//! frames are byte-identical across the run, and every class has its own
//! keep.

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

fn state(bytes: &[u8], class: &str) -> Option<ScanState> {
    detect::inspect(bytes).get(class).map(|d| d.state)
}

#[test]
fn mp3_is_a_supported_container_with_three_tag_classes() {
    assert!(Format::Mp3.is_supported_container());
    let tagged = fixture("tagged.mp3");
    let det = detect::inspect(&tagged);
    assert_eq!(det.format, Format::Mp3);
    for class in ["id3", "ape", "xing"] {
        assert_eq!(
            det.get(class).map(|d| d.state),
            Some(ScanState::ConfirmedPresent),
            "{class}"
        );
    }
    let id3 = det.get("id3").unwrap();
    assert!(id3.locations.iter().any(|l| l.container == "ID3v2"));
    assert!(id3.locations.iter().any(|l| l.container == "ID3v1"));
    let xing = det.get("xing").unwrap();
    assert!(xing.locations[0]
        .detail
        .starts_with("Info tag, encoder LAME"));
    let plain = fixture("plain.mp3");
    for class in ["id3", "ape", "xing"] {
        assert_eq!(
            state(&plain, class),
            Some(ScanState::ConfirmedAbsent),
            "{class}"
        );
    }
}

#[test]
fn the_default_run_strips_every_tag_and_no_degrade_copies_the_audio_frames() {
    let tagged = fixture("tagged.mp3");
    let before = container::signal_stream(&tagged, Format::Mp3);
    assert!(!before.is_empty());
    let no_degrade = Options {
        no_degrade: true,
        ..Default::default()
    };
    let out = clean(&tagged, &no_degrade, &pkg()).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
    assert_eq!(out.report.output_format, "mp3");
    let bytes = out.output.expect("the run writes");
    assert_eq!(container::signal_stream(&bytes, Format::Mp3), before);
    assert_eq!(bytes, before, "nothing but the audio frames remains");
    for class in ["id3", "ape", "xing"] {
        assert_eq!(
            state(&bytes, class),
            Some(ScanState::ConfirmedAbsent),
            "{class}"
        );
    }
    for label in ["ID3 tag", "APE tag", "Xing/Info frame"] {
        assert!(
            out.report
                .stripped_and_proven_gone
                .iter()
                .any(|l| l == label),
            "{label} not listed: {:?}",
            out.report.stripped_and_proven_gone
        );
    }
    let au06 = out
        .report
        .actions
        .iter()
        .find(|a| a.transform == "AU06")
        .expect("AU06 row");
    assert!(au06.outcome.contains("kept"), "{}", au06.outcome);
    #[cfg(not(feature = "audio"))]
    {
        let out = clean(&tagged, &Options::default(), &pkg()).unwrap();
        let au06 = out
            .report
            .actions
            .iter()
            .find(|a| a.transform == "AU06")
            .expect("AU06 row");
        assert_eq!(au06.outcome, "not_attempted");
        assert!(!au06.result.is_empty());
    }
    for class in ["audioseal", "synthid_audio"] {
        let s = out
            .report
            .survived
            .iter()
            .find(|s| s.class == class)
            .unwrap_or_else(|| panic!("{class} survivor row"));
        assert!(s.citation.is_some());
    }
    // The default run strips the same three classes.
    let default = clean(&tagged, &Options::default(), &pkg()).unwrap();
    assert_eq!(default.report.exit_code, EXIT_OK);
    let bytes = default.output.unwrap();
    for class in ["id3", "ape", "xing"] {
        assert_eq!(
            state(&bytes, class),
            Some(ScanState::ConfirmedAbsent),
            "{class}"
        );
    }
}

#[test]
fn each_tag_class_has_its_own_keep() {
    let tagged = fixture("tagged.mp3");
    let p = pkg();
    for (name, class, id) in [
        ("id3", "id3", "MC05"),
        ("ape", "ape", "MC13"),
        ("xing", "xing", "MC14"),
        ("MC13", "ape", "MC13"),
        ("strip-info-frame", "xing", "MC14"),
    ] {
        let out = clean(&tagged, &keep(&[name]), &p).unwrap();
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
        for other in ["id3", "ape", "xing"] {
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
fn the_information_frame_stays_when_the_next_frame_draws_from_the_reservoir() {
    let res = fixture("reservoir.mp3");
    let layout = detect::mp3::layout(&res);
    let info = layout.info.as_ref().expect("an Info frame");
    assert!(!info.droppable());
    assert!(info.next_main_data_begin.is_some_and(|v| v > 0));
    // Under --no-degrade the frames stay, so the keep rule is what decides.
    let out = clean(
        &res,
        &Options {
            no_degrade: true,
            ..Default::default()
        },
        &pkg(),
    )
    .unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
    let bytes = out.output.unwrap();
    // Only the ancillary scrub touched the file: same length, the
    // information frame in place, no byte changed to anything but zero.
    assert_eq!(bytes.len(), res.len());
    assert!(bytes.iter().zip(res.iter()).all(|(a, b)| a == b || *a == 0));
    assert_eq!(&bytes[..info.len], &res[..info.len]);
    assert_eq!(state(&bytes, "xing"), Some(ScanState::ConfirmedPresent));
    let kept = out
        .report
        .kept
        .iter()
        .find(|k| k.item.starts_with("MC14"))
        .expect("MC14 reported kept");
    assert!(kept.reason.contains("main_data_begin"), "{}", kept.reason);
    assert!(out
        .report
        .actions
        .iter()
        .any(|a| a.transform == "MC14" && a.outcome.contains("main_data_begin")));
    // The LAME-written file's first audio frame starts fresh, so there the
    // frame goes.
    let tagged = fixture("tagged.mp3");
    let info = detect::mp3::layout(&tagged).info.unwrap();
    assert_eq!(info.next_main_data_begin, Some(0));
    assert!(info.droppable());
}

#[test]
fn a_vbr_xing_frame_strips_and_the_frames_stay() {
    let vbr = fixture("vbr.mp3");
    let before = container::signal_stream(&vbr, Format::Mp3);
    let out = clean(
        &vbr,
        &Options {
            no_degrade: true,
            ..Default::default()
        },
        &pkg(),
    )
    .unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK);
    let bytes = out.output.unwrap();
    assert_eq!(bytes, before);
    assert_eq!(state(&bytes, "xing"), Some(ScanState::ConfirmedAbsent));
}

#[test]
fn a_truncated_stream_is_malformed_and_fails_closed() {
    let tagged = fixture("tagged.mp3");
    // Cut inside the last audio frame, before the trailing tags.
    let layout = detect::mp3::layout(&tagged);
    let mut cut = tagged[..layout.frames_end - 7].to_vec();
    cut.extend_from_slice(&tagged[layout.frames_end..]);
    let det = detect::inspect(&cut);
    assert_eq!(det.get("xing").map(|d| d.state), Some(ScanState::Malformed));
    let r = inspect(&cut, &Options::default(), &pkg()).unwrap();
    assert_eq!(r.exit_code, EXIT_INSTRUMENTATION);
    let err = clean(&cut, &Options::default(), &pkg()).unwrap_err();
    assert!(matches!(err, UnmarkError::Inspection(_)), "{err}");
    let spec = container::DropSpec {
        id3: true,
        ..Default::default()
    };
    assert!(matches!(
        container::rewrite(&cut, Format::Mp3, &spec),
        Err(container::RewriteError::Malformed(_))
    ));
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
    let sanity = out.report.sanity.as_ref().expect("the sanity block");
    assert!(sanity.passed);
    assert!(sanity
        .lsd_db
        .is_some_and(|v| v > 0.0 && v <= p.sanity.lsd_ceiling_db));
    assert_eq!(sanity.lsd_ceiling_db, Some(p.sanity.lsd_ceiling_db));
    for class in ["id3", "ape", "xing"] {
        assert_eq!(
            state(&bytes, class),
            Some(ScanState::ConfirmedAbsent),
            "{class}"
        );
    }
    assert!(out
        .report
        .stripped_and_proven_gone
        .iter()
        .any(|l| l == "Xing/Info frame"));
    let (back, kbps) = unmark::codec::mp3::decode_with_bitrate(&bytes).unwrap();
    assert_eq!(kbps, 128);
    assert_eq!(back.channels.len(), 1);
    assert_eq!(back.rate, 44100);
    // The encoder's ancillary bytes are gone with the re-encode.
    assert!(!bytes.windows(4).any(|w| w == b"LAME"));
    // Under --no-degrade the frames are the input's frames, untouched.
    let nd = clean(
        &tagged,
        &Options {
            no_degrade: true,
            ..Default::default()
        },
        &p,
    )
    .unwrap();
    assert_eq!(
        nd.output.unwrap(),
        container::signal_stream(&tagged, Format::Mp3)
    );
    assert!(nd.report.sanity.is_none());
    assert!(nd.report.kept.iter().any(|k| k.item.starts_with("AU06")));
}

#[cfg(feature = "audio")]
#[test]
fn a_variable_rate_input_is_re_encoded_at_its_average_and_a_reservoir_frame_goes_with_the_re_encode(
) {
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
    // The pin is the input's average, snapped to the layer III table, and
    // the output carries that rate.
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
    // The reservoir fixture's information frame is kept only for the frames
    // it feeds; the re-encode replaces those frames, so it goes too.
    let res = fixture("reservoir.mp3");
    let out = clean(&res, &Options::default(), &p).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
    let bytes = out.output.unwrap();
    assert_eq!(state(&bytes, "xing"), Some(ScanState::ConfirmedAbsent));
    assert!(out
        .report
        .stripped_and_proven_gone
        .iter()
        .any(|l| l == "Xing/Info frame"));
    assert!(!out.report.kept.iter().any(|k| k.item.starts_with("MC14")));
    // --keep xing keeps the encoder's information frame on the re-encode.
    let out = clean(&res, &keep(&["xing"]), &p).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
    let bytes = out.output.unwrap();
    assert_eq!(state(&bytes, "xing"), Some(ScanState::ConfirmedPresent));
}

// --- MC15: the ancillary bytes ----------------------------------------------

#[test]
fn the_ancillary_bytes_are_zeroed_by_default_and_kept_under_the_flag() {
    let tagged = fixture("tagged.mp3");
    let p = pkg();
    let count = |b: &[u8]| b.windows(4).filter(|w| w == b"LAME").count();
    assert!(
        count(&tagged) > 1,
        "the fixture carries the encoder name in its frames"
    );
    let before = detect::inspect(&tagged);
    let anc = before.get("mp3_ancillary").expect("the class is scanned");
    assert_eq!(anc.state, ScanState::ConfirmedPresent);
    assert!(anc.evidence.iter().any(|e| e.starts_with("LAME in ")));
    for opts in [
        Options::default(),
        Options {
            no_degrade: true,
            ..Default::default()
        },
    ] {
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
    // --keep MC15 leaves the bytes, and so does --keep by class.
    for name in ["MC15", "mp3_ancillary", "scrub-ancillary-data"] {
        let mut opts = keep(&[name]);
        opts.no_degrade = true;
        let out = clean(&tagged, &opts, &p).unwrap();
        let bytes = out.output.unwrap();
        assert!(count(&bytes) > 1, "--keep {name} zeroed the bytes");
        assert_eq!(
            state(&bytes, "mp3_ancillary"),
            Some(ScanState::ConfirmedPresent)
        );
        assert!(out.report.kept.iter().any(|k| k.item.starts_with("MC15")));
    }
    // The scrub touches only unreferenced bytes: every frame header and
    // side information byte is unchanged, and the file length is the same.
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
    for (a, b) in scrubbed.iter().zip(tagged.iter()) {
        assert!(
            a == b || *a == 0,
            "a byte was changed to something other than zero"
        );
    }
}

#[cfg(feature = "audio")]
#[test]
fn zeroed_frames_decode_to_the_same_samples() {
    for name in ["tagged.mp3", "vbr.mp3", "plain.mp3", "reservoir.mp3"] {
        let original = fixture(name);
        let scrubbed = container::mp3::scrub_ancillary(&original).unwrap();
        assert_ne!(original, scrubbed, "{name}: nothing was zeroed");
        let (a, _) = unmark::codec::mp3::decode_with_bitrate(&original).unwrap();
        let (b, _) = unmark::codec::mp3::decode_with_bitrate(&scrubbed).unwrap();
        assert_eq!(a.rate, b.rate);
        assert_eq!(a.channels.len(), b.channels.len());
        for (ca, cb) in a.channels.iter().zip(b.channels.iter()) {
            assert_eq!(ca.len(), cb.len(), "{name}: sample count changed");
            assert!(
                ca.iter()
                    .zip(cb.iter())
                    .all(|(x, y)| x.to_bits() == y.to_bits()),
                "{name}: the samples differ after the scrub"
            );
        }
    }
}

// --- M1 and M2: aligned LSD on broadband content, and a kept MC15 ----------

#[cfg(feature = "audio")]
#[test]
fn a_lame_encoded_broadband_input_passes_the_default_run_with_the_lsd_aligned() {
    let wide = fixture("broadband.mp3");
    let p = pkg();
    let out = clean(&wide, &Options::default(), &p).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
    let sanity = out.report.sanity.as_ref().unwrap();
    let lsd = sanity.lsd_db.unwrap();
    assert!(sanity.passed && lsd <= p.sanity.lsd_ceiling_db, "LSD {lsd}");
    // Broadband content reads above a tone and well under the ceiling.
    assert!(lsd > 1.0 && lsd < 4.0, "LSD {lsd}");
    let au06 = out
        .report
        .actions
        .iter()
        .find(|a| a.transform == "AU06")
        .unwrap();
    assert!(au06.result.contains("at lag "), "{}", au06.result);
    // The lag the alignment finds is the encoder and decoder delay, the
    // same for every fixture, and an unaligned measure reads several dB.
    let (audio, kbps) = unmark::codec::mp3::decode_with_bitrate(&wide).unwrap();
    let processed = unmark::transform::audio::apply(
        &audio,
        &unmark::transform::audio::AudioParams {
            highpass_hz: Some(1500.0),
        },
    );
    let (encoded, _) = unmark::codec::mp3::encode(&processed, kbps).unwrap();
    let (back, _) = unmark::codec::mp3::decode_with_bitrate(&encoded).unwrap();
    let lag = unmark::budget::align_lag(&processed.channels[0], &back.channels[0], 2304, 16384);
    assert!(lag > 0 && lag < 2304, "lag {lag}");
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
fn a_kept_ancillary_range_stands_the_audio_path_down() {
    let tagged = fixture("tagged.mp3");
    let count = |b: &[u8]| b.windows(4).filter(|w| w == b"LAME").count();
    let p = pkg();
    for name in ["MC15", "mp3_ancillary"] {
        let out = clean(&tagged, &keep(&[name]), &p).unwrap();
        assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
        let bytes = out.output.expect("the strips still write");
        assert!(
            count(&bytes) > 1,
            "--keep {name}: the ancillary bytes were discarded"
        );
        assert_eq!(
            state(&bytes, "mp3_ancillary"),
            Some(ScanState::ConfirmedPresent)
        );
        assert!(out
            .report
            .kept
            .iter()
            .any(|k| k.item.starts_with("MC15") && k.reason.contains("kept by flag")));
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
        for class in ["id3", "ape", "xing"] {
            assert_eq!(
                state(&bytes, class),
                Some(ScanState::ConfirmedAbsent),
                "{class}"
            );
        }
    }
}
