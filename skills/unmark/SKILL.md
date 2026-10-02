---
name: unmark
description: Run the unmark CLI on an image, audio file, or text file and relay its report. Use when cleaning metadata, C2PA content credentials, EXIF or XMP fields, PNG generation chunks, ID3 or RIFF tags, the dwtDct pixel mark, or invisible Unicode, and whenever the user mentions unmark, watermark removal, stripping content credentials, or de-marking an asset.
allowed-tools: Bash(unmark *)
---

# unmark

Run the unmark CLI on the user's file, read its report, and relay it. The CLI decides.
This document says how to read its result.

## Rules

- Relay the three lists as the tool prints them.
- If a user asks whether an asset is AI-made, answer that the tool inventories marks
  and does not judge origin.
- Describe what the file now contains.
- Use the tool's own words for each transform.

## The loop

1. Run `unmark inspect` first. It is read-only and always safe. Never clean before
   inspecting.
2. Read the capture line. `Certified capture kept` means the default run leaves the
   file byte-identical. Tell the user and ask before passing `--strip-capture`.
   `Capture uncertain` names a hint from camera EXIF. Tell the user the default run
   strips that EXIF and that `--keep exif` preserves it.
3. Read the detections. Separate the confirmable marks from the marks that survive.
4. Run `unmark plan` and show the user the proposal. Every transform carries its
   strength and cited effect.
5. Apply with `unmark clean`, writing to a new file or directory. Keeping the input is
   the backup.
6. `clean` re-inspects the output itself.
7. Relay the three lists.

## Running the tool

Run the default once per file. Pass `--keep <id|class>` or `--no-degrade` to
preserve an item. `--strip-capture` strips a certified-capture claim. Ask first.

```
unmark inspect --output text asset.png
unmark clean   --out clean.png asset.png > report.json
unmark verify  --report report.json clean.png
```

`--keep` takes a transform id such as `MC03`, a transform name such as `strip-exif`,
or a mark class such as `exif`, `xmp`, `c2pa`, `png_text`, `id3`, `ape`, `xing`,
`vorbis`, `ilst`, or `dwtdct`, and repeats. `--no-degrade` keeps the encoded pixels or samples
byte-identical. `--output text` prints the human report and `--output json`, the
default, prints the machine one. A directory input gets one report per file, failures
included, with `input` and `output` paths. A report with `no_op` true means the file
was already clean of everything the run targets.

If the binary is missing, tell the user to install it with `cargo install unmark`,
adding `--features audio` for WAV and FLAC, and stop.

## Reading the result

Exits: 0 done, 2 bad usage, 10 a confirmable mark is still there, 30 measurement or
decode failure, 40 unsupported input, 50 floor refusal with no file written.

`confirmed_absent` is the one scan state that means absent. Treat `unsupported_format`
and `malformed` as warnings and read their evidence in `findings`.

`actions` shows each transform's strength, outcome, and result. `kept` gives each
preserved item and why: a flag, a certified capture, or `--no-degrade`. `survived`
names each mark left in place with its evidence and citation.

## Scope

A safety hash and a mark a person or broadcaster placed are outside the tool.

## Files

- `references/transforms.md`: the transform, mark-class, held-transform, and
  guardrail reference, generated from the policy package by `unmark policy snapshot`.
  Never edited by hand.

Use unslop for writing, slop-detector for text someone sent you, and unmark for an
asset.
