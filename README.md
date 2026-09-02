# unmark

Strip the confirmable marks a generative tool left on an asset you made, and get an
honest report about the marks the tool cannot confirm. unmark removes container
metadata, C2PA content credentials, EXIF and XMP fields, PNG generation chunks,
audio tags, RIFF ancillary chunks, and invisible Unicode. It degrades only, it runs
offline, and it is deterministic.

unmark serves a person cleaning an asset they generated. It is not a tool for
stripping the marks off an asset someone else made, and the design enforces that in
the program.

## What 0.1.0 covers

This release is the metadata tier. Only the three metadata profiles run:
`image-metadata`, `audio-metadata`, and `repo-files`. Selecting a degrade profile,
`image-safe`, `image-aggressive`, `audio-safe`, or `audio-aggressive`, is a usage
error in this version and exits 2. Keyed marks such as SynthID survive every 0.1.0
clean. No transform in this release touches pixel or sample data. The fidelity
ceilings in the policy package are provisional pending owner review, and the degrade
transforms ship with the reviewed ceilings in 0.2.0.

## Install

```
cargo install unmark
```

Image support is on by default. Audio support is a feature:

```
cargo install unmark --features audio
```

## Use

```
unmark inspect --profile image-metadata --output text asset.png
unmark plan    --profile image-metadata asset.png
unmark clean   --profile image-metadata --out clean.png --i-generated-this asset.png
```

`--profile` is required and has no default. `clean` requires `--out` and the ownership
assertion `--i-generated-this`, and it writes the cleaned asset to a new file. There is
no in-place mode, because keeping the input is the backup.

| Exit | Meaning |
|---|---|
| 0 | Plan applied, every confirmable mark removed and re-proven, residuals acknowledged |
| 2 | Usage error, including a missing ownership assertion |
| 10 | A confirmable mark is still present in the output |
| 20 | An unacknowledged residual |
| 30 | Instrumentation error, fail closed |
| 40 | Unsupported input, or the guardrail refused the asset, fail closed |

## Profiles

| Profile | Asset | Fidelity cost |
|---|---|---|
| `image-metadata` | An image you generated | None. Encoded image data byte-identical |
| `audio-metadata` | An audio file you generated | None. Sample data byte-identical |
| `repo-files` | A repository or text file you generated | None. Strips invisible Unicode and generator headers |

The metadata profiles prove every claim they make and cost nothing. Most people should
run one and stop. Pixel and audio degrade profiles, which re-encode and resample to
weaken marks tied to the exact grid, arrive in 0.2.0.

## What it removes and proves gone

A confirmable mark has a structural address. unmark removes it and re-inspects the
output to prove it is gone. The confirmable set is C2PA manifests, PNG text chunks,
EXIF, XMP and IPTC, ID3 tags, RIFF ancillary chunks, and invisible Unicode. A metadata
strip is a byte-level container rewrite, so the encoded pixel or sample stream comes out
identical, and the tool verifies that too. Bytes that sit after a PNG's IEND chunk are
dropped by a clean, a known behavior in this release.

Two audio cases are narrower in this release. An MP4 or M4A `ilst` tag is not stripped
yet. A clean that targets one declines and exits 40, and most M4A files carry an
`ilst`. A FLAC Vorbis comment is production data on the preservation allowlist, so it
stays by default and comes off only when you opt in with `--opt-in MC06`.

## What it reports and does not remove

- **A keyed mark is invisible without the key.** SynthID, Tree-Ring, Stable
  Signature, and AudioSeal are keyed statistical signals in the pixel or sample data.
  This build is offline and carries no authorized detector, so it reports these as not
  attempted, before and after a clean.
- **A finding of no marks is a statement about this tool's reach.** unmark scans an
  enumerated set of containers and says `confirmed_absent` only over that set. It
  reports the marks it examined, and it names neither human authorship nor a clean
  verdict.
- **A missing credential is not proof the asset was never credentialed.** A deleted
  manifest can persist in a repository, a registry, or an earlier copy. The report
  describes the file's current contents.
- **C2PA soft bindings outlive the manifest.** Deleting a manifest does not delete a
  registry record built to match the file back to its credential.
- **Regeneration is the technique that works, and it is not this tool.** Passing an
  image through a diffusion model drops the keyed marks and returns a different picture.
  A deterministic CLI does not do it, and unmark does not pretend to.
- **Removal can leave its own trace.** A cleaned asset can read as processed even when
  no mark survives to be found. unmark reduces marks and does not make an asset look
  untouched.

## Guardrails

- The ownership assertion is required on `clean`. Nobody removes a credential without
  stating the asset is theirs.
- The third-party refusal stops a strip when a manifest names a capture, a camera
  vendor as signer, or a news or stock organization. A generative tool as claim
  generator is the normal own-asset case and does not fire it.
- The camera-origin heuristic warns when EXIF describes a photograph.
- One asset per invocation. There is no aggressive batch mode.
- Safety hashes used for abuse-material matching are permanently out of scope. No
  transform targets them or is tuned against them.
- unmark ships no keyed-mark scorer, because the honest position on a mark it cannot see
  is to say so plainly.
- unmark removes marks and never writes them. It fabricates no manifest, no timestamp,
  and no camera EXIF.

## Calibration

The pixel and audio budgets are set by a calibration pass over a corpus of
first-generation generated assets. `examples/calibrate.rs` runs
the harness over a manifest, scores every calibrated plan per (plan, format, band)
cell, derives each number by a nearest-rank percentile, checks the acceptance
properties, and writes `policy/calibration.json`, the owner table
`policy/calibration.md`, and the derived numbers into `policy/policy.toml`:

```
cargo run --release --example calibrate --all-features -- \
    --manifest corpus/manifest.json --out policy --date 2026-09-02
cargo run -- policy snapshot --out skills/unmark/references/transforms.md
```

The committed record is provisional until a corpus run lands cells that reach
`n_min`. `tests/calibration.rs` pins the fixture subset under
`fixtures/calibration/` to the record at three decimals, and a codec crate bump
changes the encoder fingerprint and fails CI until recalibration.

## Fidelity metrics

The metadata tiers gate on byte-identity of the pixel or sample stream, so every claim
is provable. The pixel and audio tiers, in a later release, gate on PSNR and SSIM for
images and log-spectral distance for audio, all pure Rust and deterministic. PESQ was
considered and excluded on its license, the ITU-T P.862 distribution terms. ViSQOL was
considered and excluded on doctrine and domain: it is a C++ dependency that breaks the
pure-Rust build, it maps similarity to quality through a trained model that would put a
learned component inside a deterministic gate, and it is speech-tuned. Its license is
not the reason. ViSQOL is Apache-2.0 and license-compatible.

## Labeling still applies

Removing a mark from your own synthetic media is a privacy and hygiene operation. It
does not discharge a disclosure duty you already had under a platform policy or a
transparency law.

## The skill

`skills/unmark/` holds an agent skill that runs the whole loop: establish ownership,
inspect first, read the detections, pick the metadata profile, show the plan, clean to
a new file, re-inspect, and report in three parts. The transform reference is generated
from the policy package.

## Two sibling tools

unslop cuts AI patterns from writing. slop-detector reads text someone else sent you.
unmark cleans an asset you made.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
