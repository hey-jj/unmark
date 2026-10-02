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
fn the_default_run_strips_every_tag_and_copies_the_audio_frames() {
    let tagged = fixture("tagged.mp3");
    let before = container::signal_stream(&tagged, Format::Mp3);
    assert!(!before.is_empty());
    let out = clean(&tagged, &Options::default(), &pkg()).unwrap();
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
    assert_eq!(au06.outcome, "not_attempted");
    assert!(!au06.result.is_empty());
    for class in ["audioseal", "synthid_audio"] {
        let s = out
            .report
            .survived
            .iter()
            .find(|s| s.class == class)
            .unwrap_or_else(|| panic!("{class} survivor row"));
        assert!(s.citation.is_some());
    }
    // The run is a container rewrite, so it is identical under --no-degrade.
    let nd = clean(
        &tagged,
        &Options {
            no_degrade: true,
            ..Default::default()
        },
        &pkg(),
    )
    .unwrap();
    assert_eq!(nd.output.unwrap(), bytes);
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
    let out = clean(&res, &Options::default(), &pkg()).unwrap();
    assert_eq!(out.report.exit_code, EXIT_OK, "{:?}", out.report.actions);
    assert!(
        out.report.no_op,
        "nothing else to strip, so the run is a no-op"
    );
    let bytes = out.output.unwrap();
    assert_eq!(bytes, res);
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
    let out = clean(&vbr, &Options::default(), &pkg()).unwrap();
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
