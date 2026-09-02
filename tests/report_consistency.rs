//! Report consistency: the removed-and-proven list describes the file the
//! user now has. When no output was written there is no such file, so the list
//! is empty whatever the in-memory re-inspection found.

mod common;
use common::*;
use unmark::report::{EXIT_INSTRUMENTATION, EXIT_RESIDUAL};
use unmark::{clean, policy, Options};

/// A WAV with a `fmt ` chunk and a LIST INFO chunk and no `data` chunk. The
/// strip succeeds in memory, but the signal stream is empty on both sides, so
/// the identity gate refuses at exit 30 and nothing is written.
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
fn identity_failure_at_exit_30_leaves_removed_and_proven_empty() {
    let wav = no_data_wav();
    let pkg = policy::load().unwrap();
    let opts = Options {
        i_generated_this: true,
        acknowledge_residual: true,
        ..Default::default()
    };
    let out = clean(&wav, "audio-metadata", &opts, &pkg).unwrap();
    assert_eq!(out.report.exit_code, EXIT_INSTRUMENTATION);
    assert!(out.output.is_none());
    assert!(
        out.report.removed_and_proven.is_empty(),
        "nothing was written, so nothing is proven removed: {:?}",
        out.report.removed_and_proven
    );
}

#[test]
fn unacknowledged_residual_at_exit_20_leaves_removed_and_proven_empty() {
    // The strip succeeds in memory and the marks are gone from the candidate
    // output, but the run gates before writing, so the list stays empty.
    let png = build_png(&PngOpts {
        text: true,
        c2pa: true,
        ..Default::default()
    });
    let pkg = policy::load().unwrap();
    let opts = Options {
        i_generated_this: true,
        ..Default::default()
    };
    let out = clean(&png, "image-metadata", &opts, &pkg).unwrap();
    assert_eq!(out.report.exit_code, EXIT_RESIDUAL);
    assert!(out.output.is_none());
    assert!(out.report.removed_and_proven.is_empty());
}

#[test]
fn a_written_output_keeps_its_removed_and_proven_list() {
    let png = build_png(&PngOpts {
        text: true,
        ..Default::default()
    });
    let pkg = policy::load().unwrap();
    let opts = Options {
        i_generated_this: true,
        acknowledge_residual: true,
        ..Default::default()
    };
    let out = clean(&png, "image-metadata", &opts, &pkg).unwrap();
    assert!(out.output.is_some());
    assert!(!out.report.removed_and_proven.is_empty());
}
