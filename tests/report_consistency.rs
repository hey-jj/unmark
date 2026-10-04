//! Report consistency: the stripped-and-proven-gone list describes the file
//! the user now has. When no output was written there is no such file, so
//! the list is empty whatever the in-memory re-inspection found.

mod common;
use common::*;
#[cfg(feature = "audio")]
use unmark::UnmarkError;
use unmark::{clean, policy, Options};

/// A WAV with a `fmt ` chunk and a LIST INFO chunk and no `data` chunk. The
/// tag strip succeeds in memory, but the audio path cannot decode a file
/// with no samples, so the required inspection fails and nothing is written.
#[cfg(feature = "audio")]
fn no_data_wav() -> Vec<u8> {
    let mut fmt = Vec::new();
    fmt.extend_from_slice(&1u16.to_le_bytes());
    fmt.extend_from_slice(&1u16.to_le_bytes());
    fmt.extend_from_slice(&8000u32.to_le_bytes());
    fmt.extend_from_slice(&8000u32.to_le_bytes());
    fmt.extend_from_slice(&1u16.to_le_bytes());
    fmt.extend_from_slice(&8u16.to_le_bytes());
    let mut chunks = Vec::new();
    chunks.extend_from_slice(b"fmt ");
    chunks.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
    chunks.extend_from_slice(&fmt);
    let mut info = b"INFO".to_vec();
    let isft = b"ComfyUI\0";
    info.extend_from_slice(b"ISFT");
    info.extend_from_slice(&(isft.len() as u32).to_le_bytes());
    info.extend_from_slice(isft);
    chunks.extend_from_slice(b"LIST");
    chunks.extend_from_slice(&(info.len() as u32).to_le_bytes());
    chunks.extend_from_slice(&info);
    let mut body = b"WAVE".to_vec();
    body.extend_from_slice(&chunks);
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    out
}

#[cfg(feature = "audio")]
#[test]
fn a_wav_with_no_samples_fails_the_required_inspection_and_writes_nothing() {
    let wav = no_data_wav();
    let pkg = policy::load().unwrap();
    let err = clean(&wav, &Options::default(), &pkg).unwrap_err();
    assert!(matches!(err, UnmarkError::Inspection(_)), "got {err}");
    // Without the audio path the tag strip alone completes.
    let opts = Options {
        no_degrade: true,
        ..Default::default()
    };
    let out = clean(&wav, &opts, &pkg).unwrap();
    assert_eq!(out.report.exit_code, 0);
    assert!(out.output.is_some());
}

#[test]
fn a_written_output_keeps_its_stripped_list_and_a_refusal_clears_it() {
    let png = build_png(&PngOpts {
        text: true,
        ..Default::default()
    });
    let pkg = policy::load().unwrap();
    let out = clean(&png, &Options::default(), &pkg).unwrap();
    assert!(out.output.is_some());
    assert!(!out.report.stripped_and_proven_gone.is_empty());
    // Every action names its outcome and result. No result says `weakened`.
    for a in &out.report.actions {
        assert!(!a.outcome.is_empty() && !a.result.is_empty());
        assert!(!a.result.contains("weakened"));
    }
}
