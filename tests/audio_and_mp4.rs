//! Audio metadata cleaning and the ISOBMFF rewrite. The audio-metadata profile
//! is behind the audio feature, so these run under `--features audio`.

mod common;
use common::*;
use unmark::asset::Format;
use unmark::container;
use unmark::detect;
use unmark::scan::ScanState;

#[cfg(feature = "audio")]
#[test]
fn wav_clean_strips_tags_and_proves_sample_stream_identical() {
    use unmark::{clean, policy, Options};
    let wav = build_wav(&WavOpts {
        list_info: true,
        id3: true,
        c2pa: true,
        ..Default::default()
    });
    let pkg = policy::load().unwrap();
    let before_sig = container::signal_stream(&wav, Format::RiffWav);
    let opts = Options {
        i_generated_this: true,
        acknowledge_residual: true,
        ..Default::default()
    };
    let out = clean(&wav, "audio-metadata", &opts, &pkg).unwrap();
    assert_eq!(out.report.exit_code, 0);
    let bytes = out.output.expect("exit 0 writes output");
    // The audio samples are byte-identical.
    assert_eq!(
        before_sig,
        container::signal_stream(&bytes, Format::RiffWav)
    );
    let det = detect::inspect(&bytes);
    assert_eq!(
        det.get("riff_ancillary").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
    assert_eq!(
        det.get("id3").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
    assert_eq!(
        det.get("c2pa").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
}

#[cfg(feature = "audio")]
#[test]
fn mp4_ilst_strip_declines_and_fails_closed() {
    use unmark::report::EXIT_UNSUPPORTED;
    use unmark::{clean, policy, Options};
    let mp4 = build_mp4_with_ilst();
    assert_eq!(unmark::asset::sniff(&mp4), Format::Isobmff);
    let pkg = policy::load().unwrap();
    let opts = Options {
        i_generated_this: true,
        acknowledge_residual: true,
        ..Default::default()
    };
    let out = clean(&mp4, "audio-metadata", &opts, &pkg).unwrap();
    // The ilst strip is not supported in this build, so the run fails closed
    // rather than claiming a removal it did not perform.
    assert_eq!(out.report.exit_code, EXIT_UNSUPPORTED);
    assert!(out.output.is_none());
    assert!(out
        .report
        .actions
        .iter()
        .any(|a| a.outcome.contains("declined")));
}

/// The C2PA uuid removal corrects the stco sample-offset table and leaves mdat
/// byte-identical. This test does not need the audio feature: it drives the
/// container rewriter directly.
#[test]
fn mp4_c2pa_uuid_removal_corrects_offsets_and_preserves_mdat() {
    let (mp4, sample) = build_mp4_with_c2pa_uuid();
    assert_eq!(
        detect::inspect(&mp4).get("c2pa").map(|d| d.state),
        Some(ScanState::ConfirmedPresent)
    );

    let spec = container::DropSpec {
        c2pa: true,
        ..Default::default()
    };
    let out = container::rewrite(&mp4, Format::Isobmff, &spec).unwrap();

    // The manifest is gone.
    assert_eq!(
        detect::inspect(&out).get("c2pa").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
    // The mdat sample payload is byte-identical.
    assert_eq!(container::signal_stream(&out, Format::Isobmff), sample);
    // The stco entry now points at the sample's new absolute offset.
    let idx = find_subslice(&out, &sample).expect("sample present in output");
    // stco layout: type(4) version-flags(4) count(4) then the first entry.
    let stco_at = find_subslice(&out, b"stco").expect("stco present") + 4 + 4 + 4;
    let stored = u32::from_be_bytes([
        out[stco_at],
        out[stco_at + 1],
        out[stco_at + 2],
        out[stco_at + 3],
    ]);
    assert_eq!(stored as usize, idx, "stco offset was not corrected");
}

fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}
