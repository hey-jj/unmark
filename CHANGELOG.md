# Changelog

All notable changes to this project are documented here. The format follows
Keep a Changelog, and the project uses semantic versioning.

## [0.3.0] - 2026-10-02

MP3 joins the supported containers.

### Added

- A ground-up MPEG audio frame parser from ISO/IEC 11172-3 and 13818-3:
  frame headers, lengths, side-information sizes, and `main_data_begin`.
- The default run on MP3 strips ID3v2 with its padding and footer, ID3v1,
  and APE tags at either end, and rewrites the Xing, Info, or VBRI
  information frame in place as its minimal form: the frame count, byte
  count, and seek table it had, the encoder delay and padding, and zero
  everywhere else, the encoder identity included. A file without an
  information frame gets none. Under `--no-degrade` the referenced frame
  bytes are copied as they are, and a decode of the output with the
  frame's delay and padding applied matches a decode of the input sample
  for sample.
- Mark classes `ape` and `mp3_info`, transforms `MC13` (strip-ape-tag) and
  `MC14` (rewrite-info-frame), and `--keep` by class or id for each. The
  `MC14` row reports `rewritten` with the fields removed and kept.
- `MC15` (scrub-ancillary-data) and the class `mp3_ancillary`: the bytes
  inside each frame's main-data region that no frame's main data covers,
  located from the side information with the bit reservoir honoured, are
  zeroed. An encoder writes its name there. The frames decode to the same
  samples, which a test proves bit for bit.
- The default run on MP3 decodes, applies the 1500 Hz highpass, and
  re-encodes at the input's bitrate, or its average for a variable-rate
  input, snapped to the layer III table. The input is padded through the
  encoder's delay, and the encoder's information frame is rewritten as the
  minimal one carrying the frame count, byte count, and the measured
  encoder delay and padding (528 samples, the same at every pin), so a
  decode of the output with those fields applied matches the input's
  length, and an untrimmed decode is longer by exactly delay plus
  padding. Kept
  tags ride around the fresh frames. `AU03` (re-encode) is the transform,
  reported as a second lossy stage with its bitrate pin, delay, and
  padding, and `AU06` reports the log-spectral distance against the
  highpassed reference as data. An audio write is refused, at exit 50,
  only when the output fails to decode, its audio frame count differs from
  the input's by more than the frames the codec delay needs, or its
  duration after the delay and padding fields differs by more than one
  frame. A `--keep` of the information
  frame or the ancillary bytes stands the highpass and the re-encode down
  with a reported reason, since a re-encode would discard them.
- The audio feature adds nanomp3 0.2.0 (MIT OR Apache-2.0, without its
  SIMD feature) as the decoder and rusty_mp3 0.8.0 (Apache-2.0, no
  dependencies) as the encoder. The minimum supported Rust version is 1.89.
- `tests/mp3_identity.rs` pins the sha256 of the decoded samples, the
  highpassed samples, and the re-encoded bytes of every MP3 fixture, so the
  macOS and Linux CI legs prove the path byte-identical across platforms.
- Without the audio feature the `AU06` row on MP3 reads `not_attempted`
  with the reason as its result field.
- Builder-rendered MP3 fixtures under `fixtures/mp3/` from a synthetic tone
  and a broadband noise-plus-chirp signal, with tests per tag class and for
  the reservoir rule. Report schema 2.3.0.

## [0.2.1] - 2026-09-05

### Added

- `PX03`, a 32-pixel border crop capped at a tenth of each edge, in the
  default run for PNG, JPEG, and WebP. It removes a corner stamp and leaves
  the pixels inside unchanged. `--keep PX03` turns it off.
- `MC11`, the JPEG COM and application segment strip, and `MC12`, the WAV
  production metadata strip, each with its own `--keep` id.
- The in-crate dwtDct read matches the reference decoder to the bit on PNG
  and 4:4:4 JPEG input: the colour conversion uses its fixed-point
  arithmetic and the wavelet its double-precision tap order.
- One report per input in a batch, failures included, with `input`,
  `output`, and `error` fields. Report schema 2.1.0.
- `no_op` in the report: an output equal to its input is a no-op, written
  only when a single `--out` names a file.
- Directory inputs expand recursively and mirror their trees under `--out`.
  Several directory arguments mirror under their own names.
- `survived` rows carry the class, the transforms applied, and the citation.
- Generator `<meta>` elements and comments are cut as spans in either
  quoting. Only banner lines are dropped whole.

### Changed

- `--out` that names the input, through a symlink or a hard link too, is a
  usage error and the input is never opened for writing.
- Output is written through a temporary file and renamed into place. A
  failed write leaves nothing behind.
- Empty input, an image narrower or shorter than eight pixels, and audio
  without samples exit 40. A fidelity check that did not run exits 30.
- `inspect` and `plan` exit 30 when a walker left a confirmable class
  malformed.
- The WAV `bext`, `iXML`, `aXML`, `_PMX`, `cue `, `smpl`, and `inst` chunks
  strip by default under `MC12`. JPEG COM and every APPn outside JFIF, the
  ICC profile, and the Adobe transform strip by default under `MC11`.
- The text report drops the warning label, the empty capture line, and the
  removal disclaimers on actions.
- A WebP VP8X header drops the flags of the metadata chunks the run removed,
  so the output decodes.
- An opt-out of the dwtDct removal (`--keep dwtdct`, `--keep PX02`, or
  `--no-degrade`) writes the file with the other strips applied.
- A colliding output in a batch is refused, never overwritten.
- `verify` refuses an unsupported, malformed, or wrong-format output.
- `--output` takes `json` or `text`. Any other value is a usage error.
- The efficacy fixtures keep their short edge at or above 336 pixels, so
  the default run's crop and resize leave an output the oracle still reads.

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
