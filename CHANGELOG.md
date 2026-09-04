# Changelog

All notable changes to this project are documented here. The format follows
Keep a Changelog, and the project uses semantic versioning.

## [0.2.0] - 2026-09-04

0.2.0 replaces the profiles with one default run per file that strips every
detected mark. A well-formed C2PA camera or sensor capture claim is the one
default keep. Opt-out flags replace the opt-in flags, and the report
separates proven removals, kept items with reasons, and survivors.

### Added

- An in-crate dwtDct detector that decodes both documented payloads from
  the U channel with one Haar level, 4x4 blocks, and quantization step 36.
  The presence rule is a Hamming agreement of 0.80 with a measured
  false-positive fraction of zero over 205 unmarked images.
- `PX02`, a resize at ratio 0.95 with a Lanczos3 filter, the measured removal
  path for the dwtDct mark. `PX01`, a quality-92, 4:4:4 JPEG write of JPEG
  input. `AU06`, a 1500 Hz highpass cited against AudioSeal.
- Certified capture: a well-formed C2PA capture claim with no later generative
  action is kept byte-identical and reported with its quoted claim and
  signature status. `--strip-capture` strips it. Well-formed is structural:
  in-crate JUMBF and CBOR readers parse the store, the claim, its assertion
  references, the actions assertion, and the signer certificate names.
- The capture-uncertain hint for camera EXIF without a claim.
- Every re-encode preserves the kept PNG chunks, JPEG segments, WebP EXIF,
  XMP, and ICC data, FLAC metadata blocks, and WAV chunks.
- A chroma-subsampled JPEG reports its dwtDct row as `unsupported_format`
  with the measured note in `findings`.
- `--keep <id|class>`, repeatable, and `--no-degrade`. Every preserved item is
  reported as kept by flag.
- MP4 `ilst` removal with `stco` and `co64` correction, and `MC10`, the FLAC
  Vorbis comment strip.
- A directory input, processed one file at a time with one report per file.
- Exit 50, the sanity floor: an output whose PSNR or SSIM against its
  grid-matched reference falls below the floor is refused and nothing is
  written.
- Builder-rendered efficacy fixtures under `fixtures/efficacy/`, watermarked
  through the subprocess oracle, and `examples/metrics.rs` over a directory.
- Pure-Rust codecs for PNG, baseline JPEG, lossless WebP, WAV, and FLAC, and
  libm-free sine, cosine, logarithm, and exponential in `dsp`.

### Changed

- A publisher manifest without a capture action is stripped by default.
- Survivors are named with evidence and a citation.
- The report schema is 2.0.0.

### Removed

- `--profile`, `--i-generated-this`, `--opt-in`, `--acknowledge-residual`,
  `--force-provenance-strip`, the residual tier, exit 20, and the one-asset
  limit.
- The fidelity-ceiling calibration harness, its record, its bands and cells,
  and the CI pins.
- `PX03` through `PX08` and `AU01` through `AU05`. `PX01`, `PX02`, and `AU06`
  run, and `PX01` runs on JPEG input.

## [0.1.0] - 2026-08-21

First release, the metadata tier.

### Added

- Container detection and hand-written walkers for JPEG, PNG, WebP, RIFF WAV,
  and ISO base media files, plus standalone ID3 and text files.
- Detection of C2PA manifests, PNG text chunks, EXIF, XMP, IPTC, ID3 tags, MP4
  ilst atoms, RIFF ancillary chunks, and invisible Unicode.
- Four scan states plus not-attempted, with `confirmed_absent` reachable only
  over the enumerated supported-container set.
- Metadata transforms MC01 through MC09, each a byte-level container rewrite
  that leaves the encoded pixel or sample stream identical.
- The verbs inspect, plan, clean, and verify.
- An embedded policy package with a sha256 digest reported in every result, and
  a skill that runs the whole loop.
