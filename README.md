# unmark

unmark strips the marks it detects from an image, audio file, or text file: container
metadata, C2PA content credentials, EXIF and XMP fields, PNG generation chunks, audio
tags, RIFF ancillary chunks, MP4 `ilst` atoms, FLAC Vorbis comments, MP3 APE tags,
information frames, and frame ancillary bytes, invisible Unicode, and the dwtDct pixel mark, which a mild resize removes and the built-in detector proves
gone. A well-formed C2PA claim that identifies a camera or sensor capture with no later
generative action is kept, and that output is byte-identical. `--keep` and
`--no-degrade` turn strips off, and `--strip-capture` strips that kept claim. The report
lists the marks removed and proven gone, the items kept with their reasons, and the marks
that survive. Keyed marks such as SynthID survive the run. The report names them. It runs
offline, it is deterministic, and it is pure Rust.

## Install

```
cargo install unmark
```

Image support is on by default. Audio support is a feature:

```
cargo install unmark --features audio
```

## Use

One default run covers each file, applied once per asset. The flags below preserve
selected items.

```
unmark inspect --output text asset.png
unmark plan    asset.png
unmark clean   --out clean.png asset.png > report.json
unmark clean   --out cleaned/ generated/
unmark verify  --report report.json clean.png
```

`clean` requires `--out` and writes to a new file, or to a directory when the input is
a directory. Keeping the input is the backup, so an `--out` that names the input,
through a symlink or a hard link too, is a usage error. Output goes through a temporary
file and is renamed into place. The output extension matches the container the run
emits. A directory input expands recursively and its tree is mirrored under `--out`,
one report per input with failures included, and a second input that would land on a
written output is refused. A run whose output equals its input is a no-op: a single
`--out` file still gets its copy, and a batch writes nothing for it.

| Flag | Effect |
|---|---|
| `--keep <id\|class>` | Repeatable. Turns off the named transform, by id or name, or every strip of the named mark class, and reports the item as kept by flag |
| `--no-degrade` | Turns off every transform that touches pixels or samples, so the encoded stream comes out byte-identical |
| `--strip-capture` | Strips a certified capture claim that the default run keeps |
| `--output json\|text` | The machine report, the default, or the human one. Any other value is a usage error |

| Exit | Meaning |
|---|---|
| 0 | The run completed |
| 2 | Usage error |
| 10 | A confirmable mark is still present in the output |
| 30 | Measurement, decoding, or a required inspection failed, including a walker that left a class malformed |
| 40 | Unsupported input, including an empty file, an image under eight pixels on an edge, and audio without samples |
| 50 | The sanity floor refused the operation and nothing was written |

## The default run

Every transform in the default run has a structural target or a cited effect on a
named mark.

| Container | Transforms |
|---|---|
| JPEG | C2PA, XMP, EXIF, and comment and application segment strips, then a 32-pixel border crop, a resize at ratio 0.95, and a re-encode at quality 92 with 4:4:4 chroma |
| PNG, WebP | C2PA, XMP, EXIF, text-chunk, and unlisted-chunk strips, then the same crop and resize and a lossless write |
| WAV, FLAC | Tag, RIFF ancillary, production metadata, Vorbis comment, and unlisted-chunk strips, then a 1500 Hz highpass |
| MP3 | ID3v2 with its padding, ID3v1, APE, and information-frame strips, the ancillary bytes of every frame zeroed, then a 1500 Hz highpass and a re-encode at the input's bitrate, or its average for a variable-rate input, with the encoder's own information frame dropped. Under `--no-degrade` the audio frames are copied byte for byte, and the information frame stays, reported kept, when the next frame's `main_data_begin` is not zero |
| MP4, M4A | Tag strips |
| Text, SVG, HTML | Invisible Unicode and generator-header strips |

The border crop cuts 32 pixels from every edge, capped at a tenth of the edge, and
removes a corner stamp. The pixels inside are unchanged. The resize removes the dwtDct
mark that Stable Diffusion pipelines write. Ratio 0.95
is the mildest value in the sweep that changes both dimensions of every input and
defeats every efficacy fixture. No ratio between 0.995 and 0.5 let any fixture
survive. At 0.95 the round-trip PSNR against the input is above 31 dB and the SSIM
above 0.93 on every fixture. For the highpass the report cites the AudioSeal figures:
accuracy 0.61, true-positive rate 0.82, false-positive rate 0.60. On MP3 the report
carries the log-spectral distance of the decoded re-encode against the highpassed
samples, held to the ceiling in the policy, and the re-encode row names its bitrate pin
and marks it a second lossy stage.

## Certified capture

A certified capture is a well-formed C2PA manifest with no later generative action and
either a `c2pa.created` action carrying the `digitalCapture` source type or a signer
certificate naming a camera vendor. Well-formed means the JUMBF boxes, the claim CBOR, the
assertion references, and the actions assertion all parse. The default run keeps it and
the output is byte-identical. The report quotes the claim and its signature status.
Pass `--strip-capture` to strip it.

Camera EXIF without a claim counts as a hint. The report names the hint, the default run
strips the EXIF, and `--keep exif` preserves it. The default run also strips a publisher
manifest without a capture action, and `--keep c2pa` preserves it.

## Reading the report

- `stripped_and_proven_gone`: the marks the run removed and re-inspection found absent.
- `kept`: every preserved item with its reason: kept by flag, kept as a certified capture, or kept under `--no-degrade`.
- `survived`: one row per keyed mark class with the transforms applied and a citation.
- `actions`: every transform with its strength, outcome, and result.
- `capture`: the capture status, the quoted claim or the named hint, and the line the run prints.
- `scan_states`: one state per mark class: `confirmed_present`, `confirmed_absent`, `unsupported_format`, `malformed`, or `not_attempted`. `findings` carries the located and malformed marks with their evidence.
- `sanity`: the PSNR and SSIM of an image output against its grid-matched reference and the floors they are held to, or the log-spectral distance of an MP3 re-encode and its ceiling.
- `no_op`, `input`, `output`, `error`: whether the output equals the input, the paths the command line used, and the failure message when a run did not complete.

## Fixtures and the oracle

The efficacy fixtures under `fixtures/efficacy/` are builder-rendered images that the
invisible-watermark subprocess oracle marks with both documented dwtDct payloads. CI
installs the package and requires the oracle agreement checks. Install the package to
run those checks locally. `examples/metrics.rs` reports the detector's decisions and
the resize sweep with PSNR and SSIM over a directory.

## The skill

`skills/unmark/` holds an agent skill that runs the whole loop: inspect first, read the
detections, show the plan, clean to a new file, and report in three parts. The
transform reference is generated from the policy package.

## Two sibling tools

unslop cuts AI patterns from writing. slop-detector reads text someone else sent you.
unmark cleans an asset.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
