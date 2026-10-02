# unmark transform reference

Regenerate with `unmark policy snapshot` after any policy change. Edits here are overwritten.

- policy version: 0.3.0
- policy digest: sha256:94ae3dafdb398460409722e2007db4dac3ac87b22e236fc4593b59731da1b749

## What each mark kind means

- A confirmable mark has a structural address or a public decoder. The tool removes it and re-inspection proves it gone.
- A blind mark is a keyed statistical signal. The default run is applied to the asset, and the report names the mark as surviving with its citation.
- An unaddressed mark is one the tool knows and leaves in place, and the report names it.
- confirmed_absent over the enumerated supported set is the one state that reads as absent.

## Supported containers

A scan reports confirmed_absent only over these. Everything else reports unsupported_format.

- jpeg
- png
- webp
- riff-wav
- isobmff
- mp3
- flac
- svg
- html
- text

## Mark classes

### c2pa (confirmable)

- C2PA manifest: A Content Credentials manifest. The parse reads the JUMBF box and the claim's content. A manifest is stripped by default and kept under the certified-capture rule or under --keep c2pa.

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

### ape (confirmable)

- APE tag: An APEv1 or APEv2 tag at either end of an MPEG audio file. Its items carry free-form text, so an encoder or tool name rides there.

### xing (confirmable)

- Xing/Info frame: The information frame an encoder writes first: a Xing or Info tag with the frame count, byte count, seek table, and a LAME-style encoder string, or a VBRI tag. The encoder string is tool identity. The frame is dropped only when the next audio frame's main_data_begin is zero, since a nonzero value means that frame draws bits from inside it.

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

- SynthID-Image: A keyed statistical mark in the pixel data of every image the Gemini API generates, vendor-stated. The default run is applied and the mark is reported as surviving.
- survival: applied, survives. The vendor states every Gemini API image carries it. Conventional-transform survival figures exist only for the external SynthID-O variant: worst-category detection 99.99 percent under quality, 99.97 under spatial, and 99.96 under noise transforms at 0.1 percent false positives.
- citation: https://ai.google.dev/gemini-api/docs/image-generation and https://arxiv.org/html/2510.09263

### tree_ring (blind)

- Tree-Ring: A keyed mark in the low-frequency Fourier structure of the initial noise. It survives the default run.
- survival: survives the default run. The cited rotation and blur figures, average detection 0.375 and 0.563 at 0.1 percent false positives, come without a tested strength, so both transforms are held.
- citation: https://arxiv.org/html/2401.08573

### stable_signature (blind)

- Stable Signature: A keyed mark decoded from the pixels. It survives the default run.
- survival: survives the default run. It detects in 84 percent of images after a crop that keeps a tenth of the pixels at a false-positive rate of one in a billion.
- citation: https://arxiv.org/html/2303.15435

### synthid_audio (blind)

- SynthID-Audio: A keyed mark in the audio samples. It survives the default run.
- survival: survives the default run. The vendor states it survives added noise, MP3 compression, and speed changes.
- citation: https://deepmind.google/models/synthid/

### audioseal (blind)

- AudioSeal: A neural audio mark. It survives the default run.
- survival: the 1500 Hz highpass is the one cited transform with an effect: detection accuracy 0.61, true and false positive rates 0.82 and 0.60. The report carries that figure.
- citation: https://arxiv.org/html/2401.17264

### visible_overlay (unaddressed)

- visible overlay: A visible logo or a corner mark. PX03 crops the border it sits in.

### c2pa_soft_binding (unaddressed)

- C2PA soft binding: A soft binding registered with an external service. The specification adds it so a file whose manifest was separated can still be matched back to its credential. Deleting the manifest does not delete that record.

### generative_fingerprint (unaddressed)

- generative fingerprint: Frequency artifacts and upsampling traces that let a detector flag a generated image with no watermark involved.

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

### MC13 strip-ape-tag (metadata, on mp3)

- target: APEv1 and APEv2 tags at either end of an MPEG audio file
- Remove every APE tag with its header, items, and footer, and copy the audio frames byte for byte. Use --keep MC13 or --keep ape to keep them.

### MC14 strip-info-frame (metadata, on mp3)

- target: The Xing, Info, or VBRI information frame and its encoder string
- Remove the information frame when the next audio frame's main_data_begin is zero. When it is not, the frame holds reservoir bits that frame decodes, so it stays and the report shows it kept with that value. Use --keep MC14 or --keep xing to keep it.

### MC09 strip-riff-ancillary (metadata, on riff-wav webp)

- target: LIST INFO and an embedded id3 chunk in WAV, WebP, and AVI
- Remove the named RIFF ancillary chunks and rebuild the enclosing sizes and even-boundary padding.

### MC12 strip-wav-production-metadata (metadata, on riff-wav)

- target: bext, iXML, aXML, _PMX, cue, smpl, and inst chunks
- Drop the production metadata chunks a WAV carries beyond its format and data. Use --keep MC12 to keep them.

### MC10 strip-vorbis-comment (metadata, on flac)

- target: The FLAC Vorbis comment block, encoder tag included
- Remove the whole Vorbis comment block and rebuild the last-block flag. Use --keep vorbis to preserve credits and chapters.

### MC06 strip-unlisted-chunks (metadata, on png webp riff-wav flac)

- target: Any ancillary chunk not required for decoding and valid output
- Drop every ancillary chunk the keep list does not name. The keep list holds only what decoding and valid output require, so an ICC color-profile chunk and a colorimetry chunk stay and everything else goes.

### MC11 strip-jpeg-segments (metadata, on jpeg)

- target: COM segments and application segments decoding does not need: every APPn other than APP0 JFIF, APP2 ICC, and APP14 Adobe
- Drop every JPEG COM segment and every application segment outside the three decoding needs. EXIF, XMP, IPTC, and C2PA segments answer to their own transforms. Use --keep MC11 to keep them.

### MC07 strip-invisible-unicode (text, on text svg html)

- target: Zero-width characters, variation selectors, unusual spaces in text files
- Sort each variation-selector run by kind before removing it. A run that parses as a C2PA text wrapper is routed to the manifest path. A run that looks like a wrapper and fails to parse reports malformed and is left alone. Everything else is removed as invisible-character hygiene.

### MC08 strip-generator-headers (text, on text svg html)

- target: SVG metadata, HTML X-Generator, PDF Producer and Creator, comment banners
- Remove generator identity headers from text and document files.

### PX03 border-crop (pixel, on png jpeg webp)

- target: A visible stamp or logo in the border
- parameters: cap=0.1, pixels=32
- strength: 32 pixels off every edge, capped at a tenth of the edge
- cited effect: A 32-pixel border crop removed all 858 pixels of a 32-pixel corner stamp on a 512 by 384 image that the metadata strips and the resize had left in place, and kept 73 percent of the area with its pixels unchanged.
- citation: the corner-stamp probe of the 2026-09-05 efficacy audit, reproduced in tests/fix_0_2_1.rs
- Cut the pinned pixels from every edge, capped at a tenth of the edge. The pixels inside the crop are unchanged. Use --no-degrade or --keep PX03 to skip it.

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
- cited effect: the same-container write for a JPEG input. The pin is reported in the result.
- Write a JPEG input back as a baseline JPEG at the pinned quality. A PNG or WebP input is written losslessly in its own container.

### AU06 highpass (audio, on riff-wav flac mp3)

- target: AudioSeal
- parameters: cutoff_hz=1500.0, order=2
- strength: second-order highpass at 1500 Hz
- cited effect: AudioSeal detection accuracy falls to 0.61 under a 1500 Hz highpass, true and false positive rates 0.82 and 0.60.
- citation: https://arxiv.org/html/2401.17264
- Apply the highpass on the scalar path and write the samples back as emitted, integer or float32. The report names the cited figure. Use --no-degrade or --keep AU06 to skip it.

### AU03 re-encode (audio, on mp3)

- target: The MP3 container after the highpass
- parameters: bitrate=input
- strength: constant bitrate at the input's bitrate, or its average for a variable-rate input, snapped to the layer III table
- cited effect: the same-container write for an MP3 input, a second lossy stage after the one that made the input. The bitrate pin is reported in the result.
- citation: ISO/IEC 11172-3 layer III bitrate table
- Write the highpassed samples back as MPEG audio layer III at the pinned bitrate, then drop the encoder's own information frame. Use --no-degrade or --keep AU06 to skip the highpass and the re-encode together.

## Held transforms

| Id | Transform | Reason |
|---|---|---|
| PX01-lossy | re-encode as a degrade on lossy input | no cited effect on any mark |
| PX04 | requantize | no cited effect on any mark |
| PX05 | add-noise | no cited effect on any mark |
| PX06 | rotate-or-flip | no cited figure shows an effect at a stated strength. Tree-Ring average detection 0.375 under rotation is cited without its tested strength. |
| PX07 | blur | no cited figure shows an effect at a stated strength. Tree-Ring average detection 0.563 under blur is cited without its tested strength. |
| PX08 | jpeg-quality-degrade | no cited figure shows an effect. Tree-Ring AUC 0.999 at quality 25 against 1.000 clean is survival. |
| AU01 | resample-round-trip | no cited effect on any mark |
| AU02 | dither-requantize | no cited effect on any mark |
| AU04 | eq-tilt | no cited effect on any mark |
| AU05 | speed-change | no cited effect on any mark |

## Sanity floor

Status confirmed by owner review 2026-09-03. An output scoring below PSNR 25.0 dB or SSIM 0.60 against its grid-matched reference is a broken encode: clean exits 50 and writes nothing.

## Guardrails

### G2 certified-capture

- An asset is certified capture when its active, well-formed C2PA manifest carries a c2pa.created action whose digital source type is digitalCapture, or a signer naming a camera vendor, and no later action naming a generative tool. The default run is a no-op on it and exits 0, and the report quotes the claim with its signature status. --strip-capture overrides the keep and is recorded. A captured image with a later generative action is stripped by default.

### G3 capture-uncertain

- Camera-style EXIF without a qualifying C2PA claim is uncertain. The asset is stripped by default and the report names the hint and says --keep exif preserves it. A generative tool can write fake EXIF and a real photograph can be model-edited, so EXIF alone certifies nothing.

### G5 safety-hashes-out-of-scope

- Perceptual hashes used for abuse-material matching are out of scope. No transform targets them, no flag reaches them, and a transform proposed because it degrades such a hash is rejected on that basis alone.

### G6 no-keyed-mark-scorer

- A blind class is reported as applied and surviving with its citation. The dwtDct detector is different in kind: the mark is keyless and its decoder is public, so presence is a declared agreement rule with a measured false-positive fraction.

### G7 never-fabricate-provenance

- Remove marks and never write them. Never synthesize a manifest, forge a claim generator, backdate a timestamp, or write camera EXIF onto an asset.

