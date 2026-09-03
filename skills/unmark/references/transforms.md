# unmark transform reference

Regenerate with `unmark policy snapshot` after any policy change. Edits here are overwritten.

- policy version: 0.2.0
- policy digest: sha256:31736d3ac0528b055680de2cca4c5a7bd19f4593c53ba1685aa6543a8dd93085

## What the tool may say about each mark

- A confirmable mark has a structural address or a public decoder. The tool removes it and re-inspection proves it gone.
- A blind mark is a keyed statistical signal this offline build cannot see. The default run is applied to the asset anyway, and the report names the mark as surviving with its citation.
- An unaddressed mark is one the tool knows and does nothing to, named so its absence from the action list is not read as its absence from the asset.
- Only confirmed_absent over the enumerated supported set licenses the word absent. The build never emits a clean verdict and never renders no marks found as human authorship.

## Supported containers

A scan reports confirmed_absent only over these. Everything else reports unsupported_format.

- jpeg
- png
- webp
- riff-wav
- isobmff
- flac
- svg
- html
- text

## Mark classes

### c2pa (confirmable)

- C2PA manifest: A Content Credentials manifest. The parse reads the JUMBF box and the claim's content. It does not verify the signature. A manifest is stripped by default. It is kept only under the certified-capture rule or under --keep c2pa.

### png_text (confirmable)

- PNG text chunk: A tEXt, iTXt, or zTXt chunk. Local generation pipelines write the prompt, the model, the seed, and the workflow graph here. This is the most identifying data on the asset and it reads in any viewer.

### exif (confirmable)

- EXIF metadata: EXIF Software, ProcessingSoftware, ImageDescription, UserComment, camera fields, and timestamps. A generative tool writes its name into Software. Camera-style EXIF without a qualifying C2PA claim is uncertain and is stripped by default, with the hint named in the report.

### xmp (confirmable)

- XMP packet: An XMP packet carrying CreatorTool and provenance fields. C2PA also references a manifest from XMP.

### iptc (confirmable)

- IPTC block: An IPTC-IIM block carrying caption, credit, and source fields.

### id3 (confirmable)

- ID3 tag: An ID3v1 or ID3v2 tag. TXXX, COMM, PRIV, and GEOB frames carry tool identity and free-form data. ID3v1 sits at the end of the file, so a walker that reads only the front reports a clean strip while the tag is still there.

### vorbis (confirmable)

- Vorbis comment: A Vorbis comment block. It carries the encoder tag and any tool identity. The whole block is stripped by default. Use --keep vorbis to preserve credits and chapters.

### ilst (confirmable)

- MP4 ilst tag: An MP4 ilst metadata atom carrying tool and encoder identity. The atom is removed and the sample offset tables are corrected by the removed byte count.

### riff_ancillary (confirmable)

- RIFF ancillary chunk: A LIST INFO chunk or an embedded id3 chunk in a WAV, WebP, or AVI container. A RIFF C2PA chunk is the manifest class and MC01 removes it. Dropping a RIFF chunk rebuilds every enclosing size field and honors even-boundary padding, or a decoder rejects the file.

### invisibles (confirmable)

- invisible Unicode: Zero-width characters, variation selectors, and unusual spaces in a text file. A variation-selector run can also carry a C2PA text wrapper, so a run is sorted by kind before it is removed and a wrapper is routed to the manifest path.

### dwtdct (confirmable)

- dwtDct pixel mark: The keyless dwtDct mark, embedded by the CompVis Stable Diffusion script as the 136-bit text StableDiffusionV1 and by the Diffusers SDXL pipeline as the fixed 48-bit payload B3EC907BB19E. The decoder reads the U channel after a BGR to YUV conversion, takes one level of Haar wavelet, quantizes the largest non-first value of each 4x4 low-low block with step 36, and majority-votes the repetitions. Presence is a Hamming agreement with a known payload at or above the declared threshold. The resize at the pinned ratio removes it.
- citation: https://github.com/CompVis/stable-diffusion/blob/main/scripts/txt2img.py and https://github.com/huggingface/diffusers/blob/main/src/diffusers/pipelines/stable_diffusion_xl/watermark.py

### synthid_image (blind)

- SynthID-Image: A keyed statistical mark in the pixel data of every image the Gemini API generates, vendor-stated. This offline build carries no authorized detector, so it cannot report presence or absence. The default run is applied and the mark is reported as surviving.
- survival: applied, survives. The vendor states every Gemini API image carries it. Conventional-transform survival figures exist only for the external SynthID-O variant: worst-category detection 99.99 percent under quality, 99.97 under spatial, and 99.96 under noise transforms at 0.1 percent false positives. Production figures are not stated.
- citation: https://ai.google.dev/gemini-api/docs/image-generation and https://arxiv.org/html/2510.09263

### tree_ring (blind)

- Tree-Ring: A keyed mark in the low-frequency Fourier structure of the initial noise. This build cannot see it.
- survival: survives the default run. The cited figures show an effect only from rotation and blur, at average detection 0.375 and 0.563 at 0.1 percent false positives, and their tested strengths are not stated, so neither is runnable.
- citation: https://arxiv.org/html/2401.08573

### stable_signature (blind)

- Stable Signature: A keyed mark decoded from the pixels. This build cannot see it.
- survival: survives the default run. It detects in 84 percent of images after a crop that keeps a tenth of the pixels at a false-positive rate of one in a billion, which is survival, not an effect.
- citation: https://arxiv.org/html/2303.15435

### synthid_audio (blind)

- SynthID-Audio: A keyed mark in the audio samples. This build cannot see it.
- survival: survives the default run. The vendor states it survives added noise, MP3 compression, and speed changes.
- citation: https://deepmind.google/models/synthid/

### audioseal (blind)

- AudioSeal: A neural audio mark. This build cannot see it.
- survival: the 1500 Hz highpass is the one cited transform with an effect: detection accuracy 0.61, true and false positive rates 0.82 and 0.60. The mark is reported as applied against with that figure, never as removed.
- citation: https://arxiv.org/html/2401.17264

### visible_overlay (unaddressed)

- visible overlay: A visible logo or a corner mark. Removing it needs inpainting, which is model-based work. No transform here touches it.

### c2pa_soft_binding (unaddressed)

- C2PA soft binding: A soft binding registered with an external service. The specification adds it so a file whose manifest was separated can still be matched back to its credential. Deleting the manifest does not delete that record.

### generative_fingerprint (unaddressed)

- generative fingerprint: Frequency artifacts and upsampling traces that let a detector flag a generated image with no watermark involved. No transform here touches them.

## Transforms in the default run

### MC01 strip-c2pa-manifest (metadata, on jpeg png webp riff-wav isobmff text svg html)

- target: JUMBF in JPEG APP11, PNG caBX, MP4 uuid box, RIFF C2PA chunk, a C2PA text wrapper
- Remove the C2PA manifest. The certified-capture rule keeps it when the claim identifies a camera or sensor capture with no later generative action, and --strip-capture overrides that keep. A publisher manifest without a capture action is stripped by default.

### MC02 strip-xmp (metadata, on jpeg png webp)

- target: XMP packet and IPTC block with provenance and generator fields
- Remove the XMP packet and the IPTC block.

### MC03 strip-exif (metadata, on jpeg png webp)

- target: EXIF Software, ProcessingSoftware, ImageDescription, camera fields, timestamps
- Remove the EXIF IFD. Camera-style EXIF without a qualifying C2PA claim is uncertain and goes with everything else. Use --keep exif to preserve it.

### MC04 strip-png-text (metadata, on png)

- target: PNG tEXt, zTXt, iTXt chunks carrying prompts and workflow JSON
- Remove the PNG text chunks, the highest-value strip in the catalog, at zero fidelity cost.

### MC05 strip-audio-tags (metadata, on riff-wav mp3 isobmff)

- target: ID3v1, ID3v2 including PRIV and GEOB, MP4 ilst
- Remove the audio tags at both the leading and the trailing location, and the MP4 ilst atom with its sample offsets corrected.

### MC09 strip-riff-ancillary (metadata, on riff-wav webp)

- target: LIST INFO and an embedded id3 chunk in WAV, WebP, and AVI
- Remove the named RIFF ancillary chunks and rebuild the enclosing sizes and even-boundary padding.

### MC10 strip-vorbis-comment (metadata, on flac)

- target: The FLAC Vorbis comment block, encoder tag included
- Remove the whole Vorbis comment block and rebuild the last-block flag. Use --keep vorbis to preserve credits and chapters.

### MC06 strip-unlisted-chunks (metadata, on png webp riff-wav flac)

- target: Any ancillary chunk not required for decoding and valid output
- Drop every ancillary chunk the keep list does not name. The keep list holds only what decoding and valid output require, so an ICC color-profile chunk and a colorimetry chunk stay and everything else goes.

### MC07 strip-invisible-unicode (text, on text svg html)

- target: Zero-width characters, variation selectors, unusual spaces in text files
- Sort each variation-selector run by kind before removing it. A run that parses as a C2PA text wrapper is routed to the manifest path. A run that looks like a wrapper and fails to parse reports malformed and is left alone. Everything else is removed as invisible-character hygiene.

### MC08 strip-generator-headers (text, on text svg html)

- target: SVG metadata, HTML X-Generator, PDF Producer and Creator, comment banners
- Remove generator identity headers from text and document files.

### PX02 resize (pixel, on png jpeg webp)

- target: The dwtDct pixel mark
- parameters: filter=lanczos3, ratio=0.95
- strength: both edges to 95 percent, Lanczos3 on the scalar path
- cited effect: dwtDct fails to decode after a resize. The sweep over the efficacy fixtures found no fixture decoding at any ratio from 99.5 percent down to 50 percent, the floor of the sweep, and 95 percent is the mildest ratio that changes both dimensions of every image at or above 10 pixels, so the resample can never round back onto the input grid.
- citation: examples/metrics.rs sweep over fixtures/efficacy on 2026-09-03, with the oracle agreement checks in tests/efficacy.rs
- Resample both edges by the pinned ratio with a separable Lanczos3 kernel. This is the removal path for the dwtDct mark, and re-inspection proves the payload no longer decodes. Use --no-degrade or --keep PX02 to skip it.

### PX01 re-encode (pixel, on jpeg)

- target: The container write for a JPEG input
- parameters: jpeg_chroma=4:4:4, jpeg_quality=92
- strength: quality 92, 4:4:4 chroma, the crate's own baseline encoder
- cited effect: none claimed. A JPEG has no lossless write, so this is the same-container path for a JPEG input and the pin is reported in the result.
- Write a JPEG input back as a baseline JPEG at the pinned quality. No removal is claimed for the encode itself. A PNG or WebP input is written losslessly in its own container.

### AU06 highpass (audio, on riff-wav flac)

- target: AudioSeal
- parameters: cutoff_hz=1500.0, order=2
- strength: second-order highpass at 1500 Hz
- cited effect: AudioSeal detection accuracy falls to 0.61 under a 1500 Hz highpass, true and false positive rates 0.82 and 0.60.
- citation: https://arxiv.org/html/2401.17264
- Apply the highpass on the scalar path and write the samples back as emitted, integer or float32. The report names the cited figure and never says the mark was removed. Use --no-degrade or --keep AU06 to skip it.

## Held, not runnable

| Id | Transform | Reason |
|---|---|---|
| PX01-lossy | re-encode as a degrade on lossy input | no cited effect on any mark |
| PX03 | border-crop | no cited figure shows an effect at a stated strength. A crop keeping a tenth of the pixels leaves Stable Signature at 84 percent detection. |
| PX04 | requantize | no cited effect on any mark |
| PX05 | add-noise | no cited effect on any mark |
| PX06 | rotate-or-flip | no cited figure shows an effect at a stated strength. Tree-Ring average detection 0.375 under rotation is cited without its tested strength. |
| PX07 | blur | no cited figure shows an effect at a stated strength. Tree-Ring average detection 0.563 under blur is cited without its tested strength. |
| PX08 | jpeg-quality-degrade | no cited figure shows an effect. Tree-Ring AUC 0.999 at quality 25 against 1.000 clean is survival. |
| AU01 | resample-round-trip | no cited effect on any mark |
| AU02 | dither-requantize | no cited effect on any mark |
| AU03 | lossy-transcode-round-trip | dropped from this release. No pure Rust MP3 or AAC encoder meets the dependency bar. |
| AU04 | eq-tilt | no cited effect on any mark |
| AU05 | speed-change | no cited effect on any mark |

## Sanity floor

Status proposed. An output scoring below PSNR 25.0 dB or SSIM 0.60 against its grid-matched reference is a broken encode: clean exits 50 and writes nothing.

## Guardrails

### G2 certified-capture

- An asset is certified capture when its active, well-formed C2PA manifest carries a c2pa.created action whose digital source type is digitalCapture, or a signer naming a camera vendor, and no later action naming a generative tool. The default run is a no-op on it and exits 0, and the report quotes the claim and says it is not signature-verified. --strip-capture overrides the keep and is recorded. A captured image with a later generative action is stripped by default.

### G3 capture-uncertain

- Camera-style EXIF without a qualifying C2PA claim is uncertain. The asset is stripped by default and the report names the hint and says --keep exif preserves it. A generative tool can write fake EXIF and a real photograph can be model-edited, so EXIF alone certifies nothing.

### G5 safety-hashes-out-of-scope

- Perceptual hashes used for abuse-material matching are not generative watermarks. No transform targets them, none is tuned against them, and none would help. This boundary is permanent, and no flag reaches it. A future transform proposed because it degrades such a hash is rejected on that basis alone.

### G6 no-keyed-mark-scorer

- This build ships no estimator that scores how likely a keyed mark is present. A blind class is reported as applied and surviving with its citation, never with a number that would read as a detection. The dwtDct detector is different in kind: the mark is keyless and its decoder is public, so presence is a declared agreement rule with a measured false-positive fraction.

### G7 never-fabricate-provenance

- Remove marks and never write them. Never synthesize a manifest, forge a claim generator, backdate a timestamp, or write camera EXIF onto an asset. Removing a record is in scope. Manufacturing a false one has no code path.

