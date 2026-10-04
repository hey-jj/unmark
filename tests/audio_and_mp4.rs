//! Audio cleaning and the ISOBMFF rewrite: tags stripped whole by default,
//! the highpass applied unless --no-degrade, and the MP4 ilst atom removed
//! with the sample offsets corrected.

mod common;
use common::*;
use unmark::asset::Format;
use unmark::container;
use unmark::detect;
use unmark::scan::ScanState;

#[cfg(feature = "audio")]
#[test]
fn wav_default_run_strips_tags_and_applies_the_highpass() {
    use unmark::{clean, policy, Options};
    let wav = build_wav(&WavOpts {
        list_info: true,
        id3: true,
        c2pa: true,
        ..Default::default()
    });
    let pkg = policy::load().unwrap();
    let before_sig = container::signal_stream(&wav, Format::RiffWav);
    // --no-degrade: samples byte-identical, tags gone.
    let opts = Options {
        no_degrade: true,
        ..Default::default()
    };
    let out = clean(&wav, &opts, &pkg).unwrap();
    assert_eq!(out.report.exit_code, 0, "{:?}", out.report.actions);
    let bytes = out.output.expect("the run writes");
    assert_eq!(
        before_sig,
        container::signal_stream(&bytes, Format::RiffWav)
    );
    let det = detect::inspect(&bytes);
    for class in ["riff_ancillary", "id3", "c2pa"] {
        assert_eq!(
            det.get(class).map(|d| d.state),
            Some(ScanState::ConfirmedAbsent)
        );
    }
    // The default run applies AU06 and reports it as applied, surviving.
    let out = clean(&wav, &Options::default(), &pkg).unwrap();
    assert_eq!(out.report.exit_code, 0, "{:?}", out.report.actions);
    let a = out
        .report
        .actions
        .iter()
        .find(|a| a.transform == "AU06")
        .expect("AU06 reported");
    assert_eq!(a.outcome, "applied");
    assert!(a.result.starts_with("applied, survives"));
    assert!(a.strength.as_deref().unwrap().contains("1500"));
    assert!(out
        .report
        .survived
        .iter()
        .any(|s| s.class == "audioseal" && s.transform == "AU06" && s.citation.is_some()));
}

#[test]
fn mp4_ilst_removal_keeps_mdat_and_proves_the_atom_gone() {
    let mp4 = build_mp4_with_ilst();
    assert_eq!(unmark::asset::sniff(&mp4), Format::Isobmff);
    assert_eq!(
        detect::inspect(&mp4).get("ilst").map(|d| d.state),
        Some(ScanState::ConfirmedPresent)
    );
    let before = container::signal_stream(&mp4, Format::Isobmff);
    let spec = container::DropSpec {
        ilst: true,
        ..Default::default()
    };
    let out = container::rewrite(&mp4, Format::Isobmff, &spec).unwrap();
    assert_eq!(container::signal_stream(&out, Format::Isobmff), before);
    assert_eq!(
        detect::inspect(&out).get("ilst").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
    // Every enclosing box size still reaches the end of the file.
    let total = u32::from_be_bytes([out[0], out[1], out[2], out[3]]) as usize;
    let moov_at = find_subslice(&out, b"moov").unwrap() - 4;
    let moov_size = u32::from_be_bytes([
        out[moov_at],
        out[moov_at + 1],
        out[moov_at + 2],
        out[moov_at + 3],
    ]) as usize;
    assert!(total + moov_size <= out.len());
    assert!(out.len() < mp4.len());
}

#[cfg(feature = "audio")]
#[test]
fn mp4_default_run_strips_ilst() {
    use unmark::{clean, policy, Options};
    let mp4 = build_mp4_with_ilst();
    let pkg = policy::load().unwrap();
    let out = clean(&mp4, &Options::default(), &pkg).unwrap();
    assert_eq!(out.report.exit_code, 0, "{:?}", out.report.actions);
    assert!(out
        .report
        .stripped_and_proven_gone
        .iter()
        .any(|l| l == "MP4 ilst tag"));
}

/// The C2PA uuid removal corrects the stco sample-offset table and leaves mdat
/// byte-identical.
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
    assert_eq!(
        detect::inspect(&out).get("c2pa").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
    assert_eq!(container::signal_stream(&out, Format::Isobmff), sample);
    let idx = find_subslice(&out, &sample).expect("sample present in output");
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

/// A decodable FLAC carrying a Vorbis comment: the crate's own encode of a
/// builder WAV, with a comment block spliced in after the encoder's blocks.
#[cfg(feature = "audio")]
fn flac_with_vorbis() -> Vec<u8> {
    let wav = build_wav(&WavOpts::default());
    let mut audio = unmark::codec::decode_audio(&wav, Format::RiffWav).unwrap();
    // The builder WAV is a few frames. A FLAC block needs more.
    for ch in audio.channels.iter_mut() {
        let base = ch.clone();
        for i in 0..4096usize {
            let s = base[i % base.len()] * 0.5 + unmark::dsp::sin(i as f64 * 0.05) * 0.3;
            ch.push(s);
        }
    }
    let fresh = unmark::codec::flac::encode(&audio, 16).unwrap();
    let (blocks, audio_start, complete) = unmark::detect::flac::blocks(&fresh);
    assert!(complete);
    let mut out = b"fLaC".to_vec();
    for b in &blocks {
        let mut blk = fresh[b.start..b.start + b.total].to_vec();
        blk[0] &= 0x7F;
        out.extend_from_slice(&blk);
    }
    let vendor = b"test writer";
    let comment = b"ENCODER=ComfyUI";
    let mut vc = Vec::new();
    vc.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    vc.extend_from_slice(vendor);
    vc.extend_from_slice(&1u32.to_le_bytes());
    vc.extend_from_slice(&(comment.len() as u32).to_le_bytes());
    vc.extend_from_slice(comment);
    out.push(0x80 | 4);
    out.extend_from_slice(&(vc.len() as u32).to_be_bytes()[1..]);
    out.extend_from_slice(&vc);
    out.extend_from_slice(&fresh[audio_start..]);
    out
}

#[cfg(feature = "audio")]
#[test]
fn a_kept_vorbis_comment_rides_through_the_flac_highpass_re_encode() {
    use unmark::{clean, policy, Options};
    let flac = flac_with_vorbis();
    let pkg = policy::load().unwrap();
    assert_eq!(
        detect::inspect(&flac).get("vorbis").map(|d| d.state),
        Some(ScanState::ConfirmedPresent)
    );
    // The default run strips it and re-encodes.
    let out = clean(&flac, &Options::default(), &pkg).unwrap();
    assert_eq!(out.report.exit_code, 0, "{:?}", out.report.actions);
    let bytes = out.output.unwrap();
    assert_eq!(
        detect::inspect(&bytes).get("vorbis").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
    // --keep vorbis preserves the comment in the output file and the report.
    // The highpass still runs.
    let keep = Options {
        keep: vec!["vorbis".to_string()],
        ..Default::default()
    };
    let out = clean(&flac, &keep, &pkg).unwrap();
    assert_eq!(out.report.exit_code, 0, "{:?}", out.report.actions);
    assert!(out
        .report
        .actions
        .iter()
        .any(|a| a.transform == "AU06" && a.outcome == "applied"));
    assert!(out
        .report
        .kept
        .iter()
        .any(|k| k.item.starts_with("MC10") && k.reason.contains("kept by flag --keep vorbis")));
    let bytes = out.output.unwrap();
    let det = detect::inspect(&bytes);
    assert_eq!(
        det.get("vorbis").map(|d| d.state),
        Some(ScanState::ConfirmedPresent),
        "the kept comment is absent from the written file"
    );
    let back = unmark::codec::decode_audio(&bytes, Format::Flac).unwrap();
    assert!(back.frames() > 0);
}

#[cfg(feature = "audio")]
#[test]
fn kept_wav_chunks_ride_through_the_highpass_re_encode() {
    use unmark::{clean, policy, Options};
    let wav = build_wav(&WavOpts {
        list_info: true,
        id3: true,
        c2pa: true,
        ..Default::default()
    });
    let pkg = policy::load().unwrap();
    let keep = Options {
        keep: vec!["riff_ancillary".to_string(), "id3".to_string()],
        ..Default::default()
    };
    let out = clean(&wav, &keep, &pkg).unwrap();
    assert_eq!(out.report.exit_code, 0, "{:?}", out.report.actions);
    assert!(out
        .report
        .actions
        .iter()
        .any(|a| a.transform == "AU06" && a.outcome == "applied"));
    let bytes = out.output.unwrap();
    let det = detect::inspect(&bytes);
    assert_eq!(
        det.get("riff_ancillary").map(|d| d.state),
        Some(ScanState::ConfirmedPresent),
        "LIST INFO was reported kept but is absent"
    );
    assert_eq!(
        det.get("id3").map(|d| d.state),
        Some(ScanState::ConfirmedPresent)
    );
    assert_eq!(
        det.get("c2pa").map(|d| d.state),
        Some(ScanState::ConfirmedAbsent)
    );
    let back = unmark::codec::decode_audio(&bytes, Format::RiffWav).unwrap();
    let original = unmark::codec::decode_audio(&wav, Format::RiffWav).unwrap();
    assert_eq!(back.frames(), original.frames());
}
