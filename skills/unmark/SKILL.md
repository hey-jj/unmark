---
name: unmark
description: Strip the confirmable marks a generative tool left on an asset you made. Use when cleaning metadata, C2PA content credentials, EXIF or XMP provenance, PNG generation chunks, ID3 or RIFF tags, or invisible Unicode from an image, an audio file, or a repository file you generated, and whenever the user mentions unmark, watermark removal, stripping provenance, or de-marking their own output. Runs the unmark CLI, reads the detections, and reports what was removed and proven gone, what could not be confirmed, and what was left untouched.
allowed-tools: Bash(unmark *)
---

# unmark

Remove the marks a generative tool wrote onto an asset the user made, and report
honestly about the marks the tool cannot confirm. The CLI is the gate. This
document is the judgment.

The tool serves a person cleaning an asset they generated. It is not for stripping
provenance from someone else's work, and two guardrails enforce that: `clean`
demands an ownership assertion, and it refuses a manifest that carries a capture or
a third-party-publisher signal.

## The rules that carry the honesty

Read these before running anything. They are the difference between an honest report
and a dangerous one.

- **Never tell the user the asset is clean.** The strongest true statement is
  "every confirmable mark I know was removed and proven gone, these blind classes
  were not addressed, and these classes were left untouched." A keyed mark could be
  present in most assets, and the tool cannot see it.
- **A finding of no marks is a statement about this tool's reach, never about the
  asset's origin.** Refuse the authorship question. If asked whether an asset is
  AI-generated or human-made, decline and offer the mark inventory instead. The tool
  scans an enumerated set of containers, and silence over that set is not proof of a
  human hand.
- **Missing credentials never prove an asset was uncredentialed.** A manifest the
  tool deleted may still exist in a repository, a registry, or an earlier copy. The
  report says what the file now contains, never what its history was.
- **A keyed mark is invisible without the key.** SynthID, Tree-Ring, Stable
  Signature, and AudioSeal report as not attempted. The tool applies no degrade
  transform to them in this build and makes no claim about them.

## The loop

1. Establish what the asset is before running anything. Where it came from, which
   tool made it, and whether the user made it. If the answer is not clearly that the
   user generated it, stop and say why.
2. Run `unmark inspect` first. It is read-only and always safe. Never clean before
   inspecting.
3. Read the detections. Separate the confirmable marks from the blind classes. These
   are different kinds of claim, and conflating them is the main way the tool could
   mislead someone.
4. Pick the profile. Start at the metadata tier. The metadata profiles remove the
   prompt text and tool identity at zero fidelity cost and prove every claim.
5. Run `unmark plan` and show the user the proposal before applying it.
6. Apply with `unmark clean`, writing to a new file. Keeping the input is the backup.
7. Re-inspect the output and confirm the confirmable marks are gone. `clean` does
   this itself and reports the result.
8. Report in three parts, always all three: removed and proven, not confirmed, and
   not addressed. Never say the asset is clean.

## Profiles

One profile per run, always declared. Milestone one ships the metadata tier.

| Asset | Profile | Fidelity cost |
|---|---|---|
| An image you generated | `image-metadata` | None. Encoded image data byte-identical |
| An audio file you generated (needs the audio feature) | `audio-metadata` | None. Sample data byte-identical |
| A repository or text file you generated | `repo-files` | None. Strips invisible Unicode and generator headers |

For anything unclear, use `image-metadata` and say that you did. The pixel and audio
degrade profiles arrive in a later release. When the user asks for them, explain that
this build removes container metadata and does not yet re-encode or resample.

## Running the tool

```
unmark inspect --profile image-metadata --output text asset.png
unmark plan    --profile image-metadata asset.png
unmark clean   --profile image-metadata --out clean.png --i-generated-this asset.png
unmark verify  --report report.json clean.png
```

`--profile` is required and has no default. `--output text` is the human report and
`--output json` is the machine one, the default. `clean` requires `--out` and
`--i-generated-this`. There is no in-place mode, because keeping the input is the
backup. One asset per invocation.

The ownership assertion is itself evidence that the asset is generated, so every
`clean` run carries a residual to acknowledge. Pass `--acknowledge-residual` once the
user has read the not-confirmed section and accepts that a keyed mark may remain.

If the binary is missing, stop and say the gate could not run. Do not substitute your
own reading for the tool. Install with `cargo install unmark`.

## Reading the result

| Exit | Meaning |
|---|---|
| 0 | Plan applied, every confirmable mark removed and re-proven, residuals acknowledged |
| 2 | Usage error, including a missing ownership assertion |
| 10 | A confirmable mark is still present in the output |
| 20 | An unacknowledged residual |
| 30 | Instrumentation error, fail closed |
| 40 | Unsupported input, or the guardrail refused the asset, fail closed |

Every mark class carries a scan state. `confirmed_absent` says a class was scanned
exhaustively in a supported container and is not there. `unsupported_format` and
`malformed` read as warnings, never as clean, and a malformed structure never counts
toward a successful strip.

The C2PA refusal fires when a manifest names a capture, a camera vendor as signer, or
a third-party publisher. A user's own generated output names the software as its
claim generator, which does not fire the refusal. When the refusal fires on a real
own-asset case, `--force-provenance-strip` overrides it, and the report says a manifest
was removed, naming the credential it destroyed.

## When to refuse

- The asset is not the user's own generative output.
- The user wants a mark presented as absent that the tool reports as not attempted.
- The user wants the output described as unmarked, human-authored, or safe to pass off
  as an original. The tool reduces marks and does not make an asset look untouched.
- The request targets a safety hash or a visible copyright or broadcast mark. Both are
  out of scope, permanently.

## What this build cannot do

- **SynthID, Tree-Ring, Stable Signature, and AudioSeal survive.** They are keyed
  marks in the pixel or sample data, and breaking them reliably needs damage that
  ruins the asset. This build does not attempt them and reports them as not attempted.
- **C2PA soft bindings outlive the manifest.** Deleting the manifest does not delete a
  registry record built to match the file back to its credential.
- **Regeneration is the technique that works, and it is not this tool.** Passing an
  image through a diffusion model drops the keyed marks and returns a different picture.
  A deterministic CLI does not do it.
- **Removal can leave its own trace.** A cleaned asset can read as processed even when
  no mark survives to be found.

## Files

- `references/transforms.md`: the transform, mark-class, profile, and guardrail
  reference, generated from the policy package by `unmark policy snapshot`. Never edited
  by hand.

Two sibling tools cover neighbouring jobs. unslop cuts AI patterns from writing.
slop-detector reads text someone else sent you. unmark cleans an asset you made.
