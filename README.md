# unmark

unmark strips every mark it can find by default and keeps provenance only where a
well-formed C2PA claim identifies a camera or sensor capture with no later generative
action. Capture claims are read, not signature-verified. Every policy flag turns a
strip off. The report lists what was stripped and proven gone, what was kept and why,
and what survived.

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
a directory. There is no in-place mode, because keeping the input is the backup. The
output extension must match the container the run emits.

| Flag | Effect |
|---|---|
| `--keep <id\|class>` | Repeatable. Turns off the named transform, by id or name, or every strip of the named mark class, and reports the item as kept by flag |
| `--no-degrade` | Turns off every transform that touches pixels or samples, so the encoded stream comes out byte-identical |
| `--strip-capture` | Strips a certified capture claim that the default run would keep |
| `--output json\|text` | The machine report, the default, or the human one |

| Exit | Meaning |
|---|---|
| 0 | The run completed |
| 2 | Usage error |
| 10 | A confirmable mark is still present in the output |
| 30 | Measurement, decoding, or a required inspection failed. Fail closed |
| 40 | Unsupported input |
| 50 | The sanity floor refused the operation. Nothing was written |

## The default run

Every transform in the default run has a structural target or a cited effect on a
named mark.

| Container | Transforms |
|---|---|
| JPEG | C2PA, XMP, and EXIF strips, then a resize at ratio 0.95 and a re-encode at quality 92 with 4:4:4 chroma |
| PNG, WebP | C2PA, XMP, EXIF, text-chunk, and unlisted-chunk strips, then a resize at ratio 0.95 and a lossless write |
| WAV, FLAC | Tag, RIFF ancillary, Vorbis comment, and unlisted-chunk strips, then a 1500 Hz highpass |
| MP3, MP4, M4A | Tag strips only. The stream is not decoded |
| Text, SVG, HTML | Invisible Unicode and generator-header strips |

The resize is the removal path for the dwtDct mark that Stable Diffusion pipelines
write. The pin is the mildest ratio that changes both dimensions of every input and
defeats every efficacy fixture. A sweep from 0.995 down to the cited bound of 0.5
found no ratio at which any fixture survived. At 0.95 the round-trip PSNR against the
input is above 31 dB and the SSIM above 0.93 on every fixture. The highpass is cited
against AudioSeal at accuracy 0.61 with a true-positive rate of 0.82 and a
false-positive rate of 0.60, so the report calls it a reduction, never a removal.

## Certified capture

A well-formed C2PA manifest whose `c2pa.created` action carries the `digitalCapture`
source type, or whose signer is a camera vendor, with no later generative action, is a
certified capture. The default run keeps it and the output is byte-identical. The
report says `Certified capture kept. Claim: ...` and adds `Signature status: not
signature-verified.` Pass `--strip-capture` to strip it.

Camera EXIF without a claim is uncertain. The report names the hint, strips by
default, and says `Use --keep exif to preserve EXIF.` A publisher manifest without a
capture action is stripped by default, and `--keep c2pa` preserves it.

## What survives

A keyed mark is invisible without the key. SynthID, Tree-Ring, Stable Signature, and
AudioSeal are reported as surviving, each with its evidence and citation. A finding of
no marks is a statement about this tool's reach over an enumerated set of containers,
never about the asset's origin. A deleted manifest can persist in a registry, and a
C2PA soft binding outlives the manifest. A cleaned asset can read as processed even
when no mark survives to be found.

## What it cannot do

`PX01` on lossy input, `PX04`, `PX05`, `AU01`, `AU02`, `AU04`, and `AU05` are held
because there is no cited effect on any mark. Rotation, blur, crop, and JPEG-quality
degrade are held because no cited figure shows an effect at a stated strength. unmark
cannot signature-verify a capture claim. The SynthID and regeneration results are
out-of-tree evidence notes, not product policy.

A lossy WebP input is written lossless, because no pure-Rust lossy WebP encoder exists.
MP3 and MP4 audio are not decoded, so only their tags are stripped. Safety hashes used
for abuse-material matching are permanently out of scope. unmark removes marks and
never writes them: no manifest, no timestamp, and no camera EXIF is fabricated.

## Fixtures and the oracle

`fixtures/efficacy/` holds builder-rendered images watermarked with both documented
dwtDct payloads through the invisible-watermark package, run as a subprocess oracle
and never linked. CI installs the package and requires the oracle agreement checks. A
local run without it skips them and says so. `examples/metrics.rs` reports the detector's
decisions and the resize sweep with PSNR and SSIM over a directory.

## Labeling still applies

Removing a mark from your own synthetic media is a privacy and hygiene operation. It
does not discharge a disclosure duty you already had under a platform policy or a
transparency law.

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
