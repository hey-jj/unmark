# unmark transform reference

Regenerate with `unmark policy snapshot` after any policy change. Edits here are overwritten.

- policy version: 0.2.0
- policy digest: sha256:498ecdb0d0923e3512f5821bf659394e3bc3612774f7d40b9aeb918892340a21

## What the tool may say about each mark

- A confirmable mark has a structural address. The tool removes it and re-inspection proves it gone.
- A blind mark is a keyed statistical signal this offline build cannot see, so it reports the mark as not_attempted, the scan state for a class this build cannot examine.
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

- C2PA manifest: A Content Credentials manifest. The parse reads the JUMBF box header and the claim generator. It does not verify the signature, so a refusal keys on the manifest's content: capture actions, camera signers, and publisher names.
- judge: The manifest records who made this asset and how. Keeping it proves the asset is yours. Confirm the asset is your own generative output before removing it.

### png_text (confirmable)

- PNG text chunk: A tEXt, iTXt, or zTXt chunk. Local generation pipelines write the prompt, the model, the seed, and the workflow graph here. This is the most identifying data on the asset and it reads in any viewer.

### exif (confirmable)

- EXIF metadata: EXIF Software, ProcessingSoftware, ImageDescription, UserComment, and timestamps. A generative tool writes its name into Software.

### xmp (confirmable)

- XMP packet: An XMP packet carrying CreatorTool and provenance fields. C2PA also references a manifest from XMP.

### iptc (confirmable)

- IPTC block: An IPTC-IIM block carrying caption, credit, and source fields.

### id3 (confirmable)

- ID3 tag: An ID3v1 or ID3v2 tag. TXXX, COMM, PRIV, and GEOB frames carry tool identity and free-form data. ID3v1 sits at the end of the file, so a walker that reads only the front reports a clean strip while the tag is still there.

### vorbis (confirmable)

- Vorbis comment: A Vorbis comment block. It routinely holds legitimate credits and chapter structure, so it stays on the preservation allowlist unless an explicit strip asks for it.

### ilst (confirmable)

- MP4 ilst tag: An MP4 ilst metadata atom carrying tool and encoder identity.

### riff_ancillary (confirmable)

- RIFF ancillary chunk: A LIST INFO chunk or an embedded id3 chunk in a WAV, WebP, or AVI container. A RIFF C2PA chunk is the manifest class and MC01 removes it. Dropping a RIFF chunk rebuilds every enclosing size field and honors even-boundary padding, or a decoder rejects the file.

### invisibles (confirmable)

- invisible Unicode: Zero-width characters, variation selectors, and unusual spaces in a text file. A variation-selector run can also carry a C2PA text wrapper, so a run is classified before it is removed and a wrapper is routed to the manifest path.

### synthid_image (blind)

- SynthID-Image: A keyed statistical mark in the pixel data. This build is offline and carries no authorized detector, so it cannot see the mark, cannot report its presence, and cannot report its absence. SynthID-Image survives the entire safe catalog, holding above 99 percent detection through re-encode, resize, crop, and noise.

### tree_ring (blind)

- Tree-Ring: A keyed mark in the low-frequency Fourier structure of the initial noise. Compression and resampling preserve low frequencies, and a circular Fourier key rotates with the picture, so the mark holds through the safe catalog. This build cannot see it.

### stable_signature (blind)

- Stable Signature: A keyed mark decoded from the pixels. It survives severe cropping, detecting in about 84 percent of images after a crop that keeps a tenth of the pixels. This build cannot see it.

### synthid_audio (blind)

- SynthID-Audio: A keyed mark in the audio samples, specified to survive MP3 compression, added noise, and speed change. The same offline limit as SynthID-Image applies: no authorized detector, so presence and absence both go unreported.

### audioseal (blind)

- AudioSeal: A neural audio mark reporting near-perfect accuracy after resampling and after MP3 and AAC transcoding. This build cannot see it.

### visible_overlay (unaddressed)

- visible overlay: A visible logo or a corner mark. Removing it needs inpainting, which is model-based work. No transform here touches it.

### c2pa_soft_binding (unaddressed)

- C2PA soft binding: A soft binding registered with an external service. The specification adds it so a file whose manifest was separated can still be matched back to its credential. Deleting the manifest does not delete that record.

### generative_fingerprint (unaddressed)

- generative fingerprint: Frequency artifacts and upsampling traces that let a classifier flag a generated image with no watermark involved. No transform here touches them.

## Transforms

### MC01 strip-c2pa-manifest (tier removable, fidelity none, default true, milestone 1)

- target: JUMBF in JPEG APP11, PNG caBX, MP4 uuid box, RIFF C2PA chunk
- Remove the C2PA manifest. Removal is gated at runtime by the ownership assertion and by the third-party-provenance refusal, which exits 40 unless the override flag is passed.

### MC02 strip-xmp (tier removable, fidelity none, default true, milestone 1)

- target: XMP packet and IPTC block with provenance and generator fields
- Remove the XMP packet and the IPTC block.

### MC03 strip-exif (tier removable, fidelity none, default true, milestone 1)

- target: EXIF Software, ProcessingSoftware, ImageDescription, timestamps
- Remove the EXIF IFD.

### MC04 strip-png-text (tier removable, fidelity none, default true, milestone 1)

- target: PNG tEXt, zTXt, iTXt chunks carrying prompts and workflow JSON
- Remove the PNG text chunks, the highest-value strip in the catalog, at zero fidelity cost.

### MC05 strip-audio-tags (tier removable, fidelity none, default true, milestone 1)

- target: ID3v1, ID3v2 including PRIV and GEOB, MP4 ilst
- Remove the audio tags at both the leading and the trailing location. Broadcast production data on the allowlist stays unless an explicit strip asks for it.

### MC09 strip-riff-ancillary (tier removable, fidelity none, default true, milestone 1)

- target: LIST INFO and an embedded id3 chunk in WAV, WebP, and AVI
- Remove the named RIFF ancillary chunks and rebuild the enclosing sizes and even-boundary padding.

### MC06 strip-unlisted-chunks (tier removable, fidelity none, default false, milestone 1)

- target: Any ancillary chunk not on the preservation keep list
- It reads the preservation keep list and drops every ancillary chunk the list does not name, so the metadata tier stays lossless.

### MC07 strip-invisible-unicode (tier removable, fidelity none, default false, milestone 1)

- target: Zero-width characters, variation selectors, unusual spaces in text files
- Classify each variation-selector run before removing it. A run that parses as a C2PA text wrapper is routed to the manifest path where the refusal applies. A run that looks like a wrapper and fails to parse reports malformed and is left alone. Everything else is removed as invisible-character hygiene.

### MC08 strip-generator-headers (tier removable, fidelity none, default false, milestone 1)

- target: SVG metadata, HTML X-Generator, PDF Producer and Creator, comment banners
- Remove generator identity headers from text and document files.

### PX06 flip-or-rotate (tier residual, fidelity geometry, default false, milestone 2)

- target: Marks without geometric invariance
- parameters: op=flip-horizontal
- Apply the pinned lossless flip. The reference for the signal metric is the input under the same flip, so the signal cost is the encode alone. A flip changes what the picture says, and a rotate buys nothing against a mark whose key is circular in Fourier space. The catalog states this plainly. Rotation is not an easy win.

### PX03 border-crop (tier residual, fidelity composition, default false, milestone 2)

- target: Marks with spatial registration
- parameters: anchor=center, area=0.9
- Crop a centered window that keeps the pinned fraction of the area. The composition change is the cost and the crop floor bounds it. The reference for the signal metric is the input cropped to the same window.

### PX02 resample (tier residual, fidelity softening, default true, milestone 2)

- target: Marks tied to the exact pixel grid
- parameters: filter=lanczos3, ratio=0.9
- Resample both edges by the pinned ratio with a separable Lanczos3 kernel on the scalar path. The geometry cost is the ratio and the profile's resample floor bounds it. The signal cost is measured against the input resampled by the same kernel, so the metric scores only the encode.

### PX05 add-noise (tier residual, fidelity grain, default false, milestone 2)

- target: Weak correlation marks
- parameters: distribution=gaussian, sigma=6.0
- Add Gaussian noise at the pinned sigma to every color channel, alpha untouched. The seed is the policy base seed combined with the input's hash, so one asset always gets the same grain and two assets never share a pattern. A shared pattern would itself be a correlational mark.

### PX04 requantize (tier residual, fidelity banding, default false, milestone 2)

- target: LSB marks in the pixel values
- parameters: bits=5
- Round every channel to the pinned bit depth and scale back to eight bits. Banding on a smooth gradient is the visible cost. Opt in one transform at a time, since stacking the aggressive set lowers quality faster than it lowers detection.

### PX01 re-encode (tier residual, fidelity one-encode, default true, milestone 2)

- target: LSB marks and fragile high-frequency DCT structure
- parameters: jpeg_chroma=4:4:4, jpeg_quality=92
- Decode and encode once at the pinned quality. A JPEG input comes back as a baseline JPEG at quality 92 with 4:4:4 chroma from the crate's own encoder. A PNG or WebP input has no lossy pure Rust encoder in this build, so the emitted format on those inputs awaits the owner's ruling and the plan reports the transform as held.

### AU01 resample-round-trip (tier residual, fidelity mild, default true, milestone 2)

- target: Marks tied to the sample grid
- parameters: beta=10.0, cutoff=0.97, filter=kaiser-sinc, half_taps=64
- Resample to the partner rate and back with a Kaiser-windowed sinc on the scalar path, 44.1 kHz to 48 kHz and 48 kHz to 44.1 kHz, any other rate through 48 kHz. The output keeps the input rate and length, so the reference is the input and the cost sits in the band near Nyquist.

### AU04 eq-tilt (tier residual, fidelity tonal, default false, milestone 2)

- target: Fixed-band spread-spectrum marks
- parameters: high_gain_db=-1.5, low_gain_db=1.5, pivot_hz=1000.0, slope=1.0
- Tilt the spectrum around the pivot with a low shelf and a high shelf of opposite gain. The tonal change is audible on careful listening and the log-spectral distance measures it directly.

### AU05 time-stretch (tier residual, fidelity tempo, default false, milestone 2)

- target: Echo hiding and time-correlated marks
- parameters: factor=1.03, method=resample
- Change the speed by the pinned factor through the same sinc resampler, which shifts tempo and pitch together. The output has no sample grid in common with the input, so its signal cost is the distance between the time-averaged log power spectra in place of the frame-wise distance.

### AU03 lossy-transcode-round-trip (tier residual, fidelity held, default false, milestone 2)

- target: Fragile spectral marks
- Reserved and not runnable in this version. No pure Rust MP3 or AAC encoder meets the dependency bar, so the entry holds the id, pins no parameters, and sits in no profile. A plan that names it is refused and reported as held.

### AU02 dither-requantize (tier residual, fidelity noise-floor, default true, milestone 2)

- target: LSB marks in PCM samples
- parameters: bits=16, dither=tpdf, dither_lsb=1.0
- Requantize to the pinned bit depth with triangular dither at one step. The raised noise floor is the cost. The dither seed derives from the base seed and the input hash, as the PX05 seed does.

## Profiles

| Profile | Media | Tier | Milestone | Transforms | Budget |
|---|---|---|---|---|---|
| audio-aggressive | audio | aggressive | 2 | MC05 MC09 AU01 AU02 | audio-aggressive |
| audio-metadata | audio | metadata | 1 | MC01 MC05 MC09 | exact |
| audio-safe | audio | safe | 2 | MC05 MC09 AU01 AU02 | audio-safe |
| image-aggressive | image | aggressive | 2 | MC01 MC02 MC03 MC04 PX01 PX02 | image-aggressive |
| image-metadata | image | metadata | 1 | MC01 MC02 MC03 MC04 | exact |
| image-safe | image | safe | 2 | MC01 MC02 MC03 MC04 PX01 PX02 | image-safe |
| repo-files | files | metadata | 1 | MC01 MC07 MC08 | exact |

## Fidelity budgets

Calibration status: provisional. Record: policy/calibration.json. Record sha256: b117d968418d8a0962ee1cb743b2d055979de91fad2c1a3697a2126a7722a1ae. Corpus manifest sha256: 606d1be8deda21f6cecaeac6ce6d13ca133c313e408e0074d6461b372ba4f834. Date: 2026-09-02.

| Budget | Kind | Signal floor or ceiling | Geometry |
|---|---|---|---|
| audio-aggressive | audio | log-spectral distance at or below 3.00 dB | resample ratio at or above 0.50, time-stretch within 0.90 to 1.10 |
| audio-safe | audio | log-spectral distance at or below 1.00 dB | resample ratio at or above 0.90, time-stretch within 1.00 to 1.00 |
| image-aggressive | image | PSNR at or above 30.0 dB, SSIM at or above 0.900 | resample ratio at or above 0.50, crop keeps at or above 0.85 of the area |
| image-safe | image | PSNR at or above 38.0 dB, SSIM at or above 0.980 | resample ratio at or above 0.75, crop keeps at or above 1.00 of the area |

## Guardrails

### G1 ownership-assertion

- clean requires the ownership flag. Its absence is a usage error naming the scope. Anyone can type the flag, and that is the point. Nobody strips provenance without stating the asset is theirs. The assertion is itself positive evidence of generative origin, so a clean run always carries a residual to acknowledge.

### G2 third-party-provenance-refusal

- Refuse the manifest strip and exit 40 when the manifest carries a capture signal or a third-party publisher signal. Capture signals are a capture action, a camera vendor as signer, or capture-shaped assertions naming a lens and an exposure. Publisher signals are a news organization, an agency, or a named author who is not the user. A generative tool as claim generator does not fire the refusal, because that is the normal case for an asset in scope. The override is a single explicit flag.

### G3 camera-origin-heuristic

- EXIF naming a real camera body with lens data and a shutter, aperture, and ISO triple describes a photograph. Warn and hold at the metadata tier unless overridden. A generative tool can write fake EXIF and a real photograph can be model-edited, so the warning asks for a second look at the asset's origin, and the override proceeds.

### G4 no-aggressive-batch

- One asset per invocation for anything that touches provenance or pixels. A directory of images is the shape of the misuse case. A metadata strip over a user's own output directory can take a glob.

### G5 safety-hashes-out-of-scope

- Perceptual hashes used for abuse-material matching are not generative watermarks. No transform targets them, none is tuned against them, and none would help. This boundary is permanent, and no flag or profile reaches it. A future transform proposed because it degrades such a hash is rejected on that basis alone.

### G6 no-keyed-mark-scorer

- This build ships no estimator that scores how likely a keyed mark is present, and no reverse-detector. The blind class rests on saying plainly that this build cannot see the mark, and a number next to that sentence would erase it. The honest way to make a keyed mark confirmable is an authorized detector that actually knows.

### G7 never-fabricate-provenance

- Remove marks and never write them. Never synthesize a manifest, forge a claim generator, backdate a timestamp, or write camera EXIF onto a generated asset. Removing a true record on a user's own asset is in scope. Manufacturing a false one has no code path.

