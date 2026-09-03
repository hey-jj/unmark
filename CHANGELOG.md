# Changelog

All notable changes to this project are documented here. The format follows
Keep a Changelog, and the project uses semantic versioning.

## [0.2.0] - 2026-09-03

The image degrade profiles `image-safe` and
`image-aggressive` now run. The audio degrade profiles stay a usage error at
exit 2 until the audio ceilings calibrate. Every ceiling is provisional until a
calibration record with qualified cells lands.

### Added

- Pixel transforms PX01 through PX06 and audio transforms AU01, AU02, AU04, and
  AU05, each with pinned parameters in the policy package. AU03 is reserved and
  not runnable.
- Pure-Rust codecs behind the transforms, PNG, JPEG with an in-crate baseline
  encoder, WebP with a lossless write, WAV, and FLAC, and an encoder fingerprint
  naming each crate and version.
- Libm-free sine, cosine, logarithm, and exponential in `dsp`, so every
  transform produces the same bytes on every platform.
- The fidelity-ceiling calibration harness, `examples/calibrate.rs` over the
  `calibrate` module. It carries a cell model over plan, format, and band, a
  nearest-rank percentile derivation, the acceptance properties, a committed
  record and owner table, and a CI suite that pins the fixture subset at three
  decimals.
- The windowed 8x8 SSIM, the log-spectral distance, and the time-averaged
  spectrum distance in `budget`.
- Manifest-driven corpus selection with eligibility classes, a prepare pass,
  lossless container derivations, synthetic clips, documented-marks splits, and
  a report-only ladder over post-processed assets.
- `output_format` and `fidelity` in the report. `clean` exits 2 when the
  `--out` extension contradicts the container the plan emits.

### Changed

- PX01 on a PNG or WebP input emits a baseline JPEG when the image is opaque
  and stands down with a one-line note when it carries alpha. Alpha is never
  flattened.
- The global single-window SSIM is replaced by the windowed form.

## [0.1.0] - 2026-08-21

First release, the metadata tier. Only the `image-metadata`, `audio-metadata`, and
`repo-files` profiles run. Selecting a degrade profile is a usage error at exit 2.
Keyed marks such as SynthID survive every clean in this version. The fidelity
ceilings in the policy package are provisional pending owner review, and the
degrade transforms follow with the reviewed ceilings in 0.2.0.

### Added

- Container detection and hand-written walkers for JPEG, PNG, WebP, RIFF WAV,
  and ISO base media files, plus standalone ID3 and text files.
- Detection of C2PA manifests, PNG text chunks, EXIF, XMP, IPTC, ID3 tags, MP4
  ilst atoms, RIFF ancillary chunks, and invisible Unicode.
- Four scan states plus not-attempted, with `confirmed_absent` reachable only
  over the enumerated supported-container set.
- Metadata transforms MC01 through MC09, each a byte-level container rewrite
  that leaves the encoded pixel or sample stream identical.
- The verbs inspect, plan, clean, and verify, with the exit contract 0, 2, 10,
  20, 30, and 40.
- Guardrails G1 through G7: the ownership assertion, the third-party refusal on a
  capture or publisher manifest, the camera-origin heuristic, one asset per
  invocation, safety hashes out of scope, no keyed-mark scorer, and no fabricated
  credentials.
- The fidelity-budget machinery, with a byte-identity gate for the metadata
  tiers and provisional PSNR, SSIM, and log-spectral-distance ceilings for the
  pixel and audio tiers that arrive in 0.2.0.
- An embedded policy package with a sha256 digest reported in every result, and
  a skill that runs the whole loop.
- The default audio pass strips LIST INFO whole and leaves a FLAC Vorbis comment
  in place, so a default FLAC pass removes no tags and an encoder tag survives
  it. The 0.1.x target is a field-selective symmetric default: a default pass
  removes only tool-identity fields from both blocks, the ISFT and IENG-class
  fields in LIST INFO and the ENCODER, ENCODED_BY, and generator-naming COMMENT
  fields in Vorbis, keeps credits and chapters, and MC06 removes the whole block.
