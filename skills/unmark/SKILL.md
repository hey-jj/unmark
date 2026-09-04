---
name: unmark
description: Strip every mark unmark can find on an image, audio file, or text file, keep certified camera captures, and report the stripped, kept, and surviving marks. Use when cleaning metadata, C2PA content credentials, EXIF or XMP fields, PNG generation chunks, ID3 or RIFF tags, the dwtDct pixel mark, or invisible Unicode, and whenever the user mentions unmark, watermark removal, stripping content credentials, or de-marking an asset. Runs the unmark CLI and reads its report.
allowed-tools: Bash(unmark *)
---

# unmark

unmark strips every mark it can find by default and keeps a content credential only
where a well-formed C2PA claim identifies a camera or sensor capture with no later
generative action. Every policy flag turns a strip off. The report lists what was
stripped and proven gone, what was kept and why, and what survived. The CLI is the
gate. This document is the judgment.

## Rules

- Report the three lists as the tool emits them. Never say the asset is clean.
- Refuse the authorship question. If asked whether an asset is AI-generated or
  human-made, offer the mark inventory instead.
- Describe what the file now contains. The report says nothing about its history.
- Use the tool's own words for each transform. Never say "weakened".

## The loop

1. Run `unmark inspect` first. It is read-only and always safe. Never clean before
   inspecting.
2. Read the capture line. `Certified capture kept` means the default run is a byte-
   identical no-op. Tell the user and ask before passing `--strip-capture`. `Capture
   uncertain` names a hint from camera EXIF. Tell the user the EXIF strips by default
   and that `--keep exif` preserves it.
3. Read the detections. Separate the confirmable marks from the marks that survive.
4. Run `unmark plan` and show the user the proposal. Every transform carries its
   strength and cited effect.
5. Apply with `unmark clean`, writing to a new file or directory. Keeping the input is
   the backup.
6. `clean` re-inspects the output itself. Read the three lists.
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
or `dwtdct`, and repeats. `--no-degrade` turns off every pixel and sample transform,
so the encoded stream comes out byte-identical. `--output text` is the human report
and `--output json` is the machine one, the default. A directory input processes every
supported file in it and writes one report per file.

If the binary is missing, stop and say the gate could not run. Install with
`cargo install unmark`, adding `--features audio` for WAV and FLAC.

## Reading the result

| Exit | Meaning |
|---|---|
| 0 | The run completed |
| 2 | Usage error |
| 10 | A confirmable mark is still present in the output |
| 30 | Measurement, decoding, or a required inspection failed |
| 40 | Unsupported input |
| 50 | The sanity floor refused the operation and nothing was written |

Every mark class carries a scan state. `confirmed_absent` says a class was scanned
exhaustively in a supported container and is absent. `unsupported_format` and
`malformed` read as warnings and carry their evidence in `findings`.

The `actions` list carries every transform with its strength, its outcome, and its
result. The `kept` list carries every preserved item with the reason: kept by flag,
kept as a certified capture, or kept under `--no-degrade`. The `survived` list names
every mark the run leaves in place, with evidence and a citation.

## When to refuse

- The user wants a mark presented as absent that the tool reports as surviving.
- The user wants the output described as unmarked, human-authored, or safe to pass off
  as an original.
- The request targets a safety hash or a visible copyright or broadcast mark. Both are
  out of scope.

## Files

- `references/transforms.md`: the transform, mark-class, held-transform, and
  guardrail reference, generated from the policy package by `unmark policy snapshot`.
  Never edited by hand.

Two sibling tools cover neighbouring jobs. unslop cuts AI patterns from writing.
slop-detector reads text someone else sent you. unmark cleans an asset.
