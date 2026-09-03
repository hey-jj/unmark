---
name: unmark
description: Strip every mark unmark can find on an image, audio file, or text file, keep certified camera captures, and report what was stripped and proven gone, what was kept and why, and what survived. Use when cleaning metadata, C2PA content credentials, EXIF or XMP provenance, PNG generation chunks, ID3 or RIFF tags, the dwtDct pixel mark, or invisible Unicode, and whenever the user mentions unmark, watermark removal, stripping provenance, or de-marking an asset. Runs the unmark CLI and reads its report.
allowed-tools: Bash(unmark *)
---

# unmark

unmark strips every mark it can find by default and keeps provenance only where a
well-formed C2PA claim identifies a camera or sensor capture with no later generative
action. Capture claims are read, not signature-verified. Every policy flag turns a
strip off. The report lists what was stripped and proven gone, what was kept and why,
and what survived. The CLI is the gate. This document is the judgment.

## The rules that carry the honesty

- **Never tell the user the asset is clean.** The strongest true statement is
  "every confirmable mark was removed and proven gone, these items were kept, and
  these marks survive." A keyed mark is invisible without the key, and the report
  names it as surviving.
- **A finding of no marks is a statement about this tool's reach, never about the
  asset's origin.** Refuse the authorship question. If asked whether an asset is
  AI-generated or human-made, decline and offer the mark inventory instead.
- **Missing credentials never prove an asset was uncredentialed.** A deleted manifest
  may still exist in a registry or an earlier copy. The report says what the file now
  contains, never what its history was.
- **Never say "weakened".** A transform either removed a mark and proved it gone, or
  the mark survives with its evidence named. The highpass is a cited reduction of
  AudioSeal and the report says so in those terms.

## The loop

1. Run `unmark inspect` first. It is read-only and always safe. Never clean before
   inspecting.
2. Read the capture line. `Certified capture kept` means the default run is a byte-
   identical no-op. Tell the user and ask before passing `--strip-capture`. `Capture
   uncertain` names a hint from camera EXIF. Tell the user the EXIF strips by default
   and that `--keep exif` preserves it.
3. Read the detections. Separate the confirmable marks from the marks that survive.
   These are different kinds of claim.
4. Run `unmark plan` and show the user the proposal. Every transform carries its
   strength and cited effect.
5. Apply with `unmark clean`, writing to a new file or directory. Keeping the input is
   the backup.
6. `clean` re-inspects the output itself. Read the result: stripped and proven gone,
   kept with reasons, and survived with evidence.
7. Report in three parts, always all three.

## Running the tool

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

`--keep` takes a transform id such as `MC03`, a transform name such as `strip-exif`,
or a mark class such as `exif`, `xmp`, `c2pa`, `png_text`, `id3`, `vorbis`, `ilst`,
or `dwtdct`, and may repeat. `--no-degrade` turns off every pixel and sample
transform, so the encoded stream comes out byte-identical. `--output text` is the
human report and `--output json` is the machine one, the default. A directory input
processes every supported file in it and writes one report per file.

If the binary is missing, stop and say the gate could not run. Do not substitute your
own reading for the tool. Install with `cargo install unmark`, adding
`--features audio` for WAV and FLAC.

## Reading the result

| Exit | Meaning |
|---|---|
| 0 | The run completed |
| 2 | Usage error |
| 10 | A confirmable mark is still present in the output |
| 30 | Measurement, decoding, or a required inspection failed. Fail closed |
| 40 | Unsupported input |
| 50 | The sanity floor refused the operation. Nothing was written |

Every mark class carries a scan state. `confirmed_absent` says a class was scanned
exhaustively in a supported container and is not there. `unsupported_format` and
`malformed` read as warnings, never as clean, and a malformed structure never counts
toward a successful strip.

The `actions` list carries every transform with its strength, its outcome, and its
result. The `kept` list carries every preserved item with the reason: kept by flag,
kept as a certified capture, or kept because `--no-degrade` turned the transform off.
The `survived` list names every mark the run does not reach, with evidence and a
citation.

## When to refuse

- The user wants a mark presented as absent that the tool reports as surviving.
- The user wants the output described as unmarked, human-authored, or safe to pass off
  as an original. The tool removes marks and does not make an asset look untouched.
- The request targets a safety hash or a visible copyright or broadcast mark. Both are
  out of scope, permanently.

## What this build cannot do

`PX01` on lossy input, `PX04`, `PX05`, `AU01`, `AU02`, `AU04`, and `AU05` are held
because there is no cited effect on any mark. Rotation, blur, crop, and JPEG-quality
degrade are held because no cited figure shows an effect at a stated strength. unmark
cannot signature-verify a capture claim. The SynthID and regeneration results are
out-of-tree evidence notes, not product policy.

- **SynthID, Tree-Ring, Stable Signature, and AudioSeal survive.** They are keyed
  marks, and the report names each with its evidence.
- **C2PA soft bindings outlive the manifest.** Deleting the manifest does not delete a
  registry record built to match the file back to its credential.
- **A lossy WebP input is written lossless.** No pure-Rust lossy WebP encoder exists.
- **MP3 and MP4 audio are not decoded.** Only their tags are stripped.
- **Removal can leave its own trace.** A cleaned asset can read as processed even when
  no mark survives to be found.

## Files

- `references/transforms.md`: the transform, mark-class, held-transform, and
  guardrail reference, generated from the policy package by `unmark policy snapshot`.
  Never edited by hand.

Two sibling tools cover neighbouring jobs. unslop cuts AI patterns from writing.
slop-detector reads text someone else sent you. unmark cleans an asset.
