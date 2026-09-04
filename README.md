# unmark

unmark strips every mark it can find by default and keeps a content credential only
where a well-formed C2PA claim identifies a camera or sensor capture with no later
generative action. Every policy flag turns a strip off. The report lists what was
stripped and proven gone, what was kept and why, and what survived.

unmark removes container metadata, C2PA content credentials, EXIF and XMP fields, PNG
generation chunks, audio tags, RIFF ancillary chunks, MP4 `ilst` atoms, FLAC Vorbis
comments, and invisible Unicode. It removes the dwtDct pixel mark by a mild resize and
proves the removal with its own detector. It runs offline, it is deterministic, and it
is pure Rust.

## Install

```
cargo install unmark
```

Image support is on by default. Audio support is a feature:

```
cargo install unmark --features audio
```

## Use

unmark has no profiles. Each sniffed container receives one default run. Use `--keep
<id|class>` or `--no-degrade` to preserve selected content. Use `--strip-capture` to
override the certified-capture keep.

```
unmark inspect --output text asset.png
unmark plan    asset.png
unmark clean   --out clean.png asset.png
unmark clean   --out cleaned/ generated/
unmark verify  --report report.json clean.png
```

`clean` requires `--out` and writes to a new file, or to a directory when the input is
a directory. Keeping the input is the backup. The output extension matches the
container the run emits.

| Flag | Effect |
|---|---|
| `--keep <id\|class>` | Repeatable. Turns off the named transform, by id or name, or every strip of the named mark class, and reports the item as kept by flag |
| `--no-degrade` | Turns off every transform that touches pixels or samples, so the encoded stream comes out byte-identical |
| `--strip-capture` | Strips a certified capture claim that the default run keeps |
| `--output json\|text` | The machine report, the default, or the human one |

| Exit | Meaning |
|---|---|
| 0 | The run completed |
| 2 | Usage error |
| 10 | A confirmable mark is still present in the output |
| 30 | Measurement, decoding, or a required inspection failed |
| 40 | Unsupported input |
| 50 | The sanity floor refused the operation and nothing was written |

## The default run

Every transform in the default run has a structural target or a cited effect on a
named mark.

| Container | Transforms |
|---|---|
| JPEG | C2PA, XMP, and EXIF strips, then a resize at ratio 0.95 and a re-encode at quality 92 with 4:4:4 chroma |
| PNG, WebP | C2PA, XMP, EXIF, text-chunk, and unlisted-chunk strips, then the same resize and a lossless write |
| WAV, FLAC | Tag, RIFF ancillary, Vorbis comment, and unlisted-chunk strips, then a 1500 Hz highpass |
| MP3, MP4, M4A | Tag strips |
| Text, SVG, HTML | Invisible Unicode and generator-header strips |

The resize is the removal path for the dwtDct mark that Stable Diffusion pipelines
write. The pin is the mildest ratio that changes both dimensions of every input and
defeats every efficacy fixture. A sweep from 0.995 down to 0.5 found no ratio at which
any fixture survived. At 0.95 the round-trip PSNR against the input is above 31 dB and
the SSIM above 0.93 on every fixture. The highpass is cited against AudioSeal at
accuracy 0.61 with a true-positive rate of 0.82 and a false-positive rate of 0.60, and
the report carries that figure.

## Certified capture

A well-formed C2PA manifest whose `c2pa.created` action carries the `digitalCapture`
source type, or whose signer certificate names a camera vendor, with no later generative
action, is a certified capture. Well-formed means the JUMBF boxes, the claim CBOR, the
assertion references, and the actions assertion all parse. The default run keeps it and
the output is byte-identical. The report quotes the claim and its signature status.
Pass `--strip-capture` to strip it.

Camera EXIF without a claim is uncertain. The report names the hint, the default run
strips the EXIF, and `--keep exif` preserves it. The default run also strips a publisher
manifest without a capture action, and `--keep c2pa` preserves it.

## Reading the report

- `stripped_and_proven_gone`: the marks the run removed and re-inspection found absent.
- `kept`: every preserved item with its reason: kept by flag, kept as a certified capture, or kept under `--no-degrade`.
- `survived`: the marks the run leaves in place, each with its evidence and a citation.
- `actions`: every transform with its strength, outcome, and result.
- `capture`: the capture status, the quoted claim or the named hint, and the line the run prints.
- `scan_states`: one state per mark class. `confirmed_present` and `confirmed_absent` are exhaustive reads over a supported container. `unsupported_format` and `malformed` carry their evidence in `findings`. `not_attempted` marks a class this build reads from citations.
- `sanity`: the PSNR and SSIM of the output against its grid-matched reference and the floor they are held to.

## Fixtures and the oracle

`fixtures/efficacy/` holds builder-rendered images watermarked with both documented
dwtDct payloads through the invisible-watermark package, run as a subprocess oracle.
CI installs the package and requires the oracle agreement checks. A local run without
it skips them and says so. `examples/metrics.rs` reports the
detector's decisions and the resize sweep with PSNR and SSIM over a directory.

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
