# unmark calibration record

Generated from `calibration.json` by the calibration harness. Edits here are overwritten. Cell rows only: per-asset scores live in the record, keyed by doc_id and sha256.

- date: 2026-09-03
- policy version: 0.2.0
- corpus manifest: manifest.json (sha256 816e735fc268d4cd83f53c982140a08a064cb20d5d3366e03b06ad13f1cb2529)
- assets: 15 listed, 15 scored, 0 excluded
- scored by eligibility: floor 15
- encoder fingerprint: unmark 0.1.0; container-rewrite/no-reencode; png 0.18.1; jpeg-decoder 0.3.2; image-webp 0.2.4; claxon 0.4.3; flacenc 0.5.1; jpeg-encoder in-crate v1 baseline 4:4:4; resample in-crate lanczos3 and kaiser-sinc scalar; trig in-crate libm-free
- base seed: 0x5eed202609020001
- derivation: image floors at the 5th percentile rounded down to 0.5 dB and 0.005, audio ceiling at the 95th percentile rounded up to 0.05 dB, n_min 40, class_min 10, no generator over half a cell
- separation: the next tier fails when either metric's median misses the number; the gate refuses on either
- pixel-identical rows lost across the safe cells: 0
- accepted: no, a property failed or nothing qualified

## Derived numbers

| Budget | Provisional | Calibrated | Source | Overrides |
|---|---|---|---|---|
| image-safe | PSNR 38.0 dB, SSIM 0.980 | PSNR 38.0 dB, SSIM 0.980 | provisional | none |
| image-aggressive | PSNR 30.0 dB, SSIM 0.900 | PSNR 30.0 dB, SSIM 0.900 | provisional | none |
| audio-safe | LSD 1.00 dB | LSD 1.00 dB | provisional | none |
| audio-aggressive | LSD 3.00 dB | LSD 3.00 dB | provisional | none |

## Image cells

| Plan | Format | Band | Derived | n | Bases | Pixel-identical lost | Gen. max share | Classes | Missing classes | Held | Status | P5 PSNR | P5 SSIM | Median PSNR | Median SSIM | Cell floor | Next tier median |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| image-safe | png | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | unqualified | 35.302 | 0.955 | 35.302 | 0.955 | 35.0 dB, 0.955 | image-aggressive: 30.562 dB, 0.807 |
| image-safe | png | large | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | unqualified | 35.287 | 0.977 | 35.287 | 0.977 | 35.0 dB, 0.975 | image-aggressive: 30.738 dB, 0.893 |
| image-aggressive | png | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | unqualified | 30.562 | 0.807 | 30.562 | 0.807 | 30.5 dB, 0.805 | image-two-opt-in: 30.017 dB, 0.781 |
| image-aggressive | png | large | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | unqualified | 30.738 | 0.893 | 30.738 | 0.893 | 30.5 dB, 0.890 | image-two-opt-in: 30.179 dB, 0.878 |
| image-aggressive+PX03 | png | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 35.432 | 0.956 | 35.432 | 0.956 | 35.0 dB, 0.955 | n/a |
| image-aggressive+PX03 | png | large | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 35.364 | 0.978 | 35.364 | 0.978 | 35.0 dB, 0.975 | n/a |
| image-aggressive+PX04 | png | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 34.494 | 0.937 | 34.494 | 0.937 | 34.0 dB, 0.935 | n/a |
| image-aggressive+PX04 | png | large | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 34.510 | 0.968 | 34.510 | 0.968 | 34.5 dB, 0.965 | n/a |
| image-aggressive+PX05 | png | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 30.562 | 0.807 | 30.562 | 0.807 | 30.5 dB, 0.805 | n/a |
| image-aggressive+PX05 | png | large | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 30.738 | 0.893 | 30.738 | 0.893 | 30.5 dB, 0.890 | n/a |
| image-aggressive+PX06 | png | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 35.302 | 0.955 | 35.302 | 0.955 | 35.0 dB, 0.955 | n/a |
| image-aggressive+PX06 | png | large | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 35.298 | 0.977 | 35.298 | 0.977 | 35.0 dB, 0.975 | n/a |
| image-two-opt-in | png | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 30.017 | 0.781 | 30.017 | 0.781 | 30.0 dB, 0.780 | n/a |
| image-two-opt-in | png | large | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 30.179 | 0.878 | 30.179 | 0.878 | 30.0 dB, 0.875 | n/a |
| PX01 | png | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 34.395 | 0.938 | 34.395 | 0.938 | 34.0 dB, 0.935 | n/a |
| PX01 | png | large | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 34.431 | 0.964 | 34.431 | 0.964 | 34.0 dB, 0.960 | n/a |
| PX02 | png | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| PX02 | png | large | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| PX03 | png | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| PX03 | png | large | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| PX04 | png | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 40.498 | 0.972 | 40.498 | 0.972 | 40.0 dB, 0.970 | n/a |
| PX04 | png | large | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 40.518 | 0.984 | 40.518 | 0.984 | 40.5 dB, 0.980 | n/a |
| PX05 | png | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 32.549 | 0.846 | 32.549 | 0.846 | 32.5 dB, 0.845 | n/a |
| PX05 | png | large | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 32.556 | 0.905 | 32.556 | 0.905 | 32.5 dB, 0.905 | n/a |
| PX06 | png | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| PX06 | png | large | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| image-safe | png-alpha | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | unqualified | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | image-aggressive: 33.830 dB, 0.892 |
| image-aggressive | png-alpha | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | unqualified | 33.830 | 0.892 | 33.830 | 0.892 | 33.5 dB, 0.890 | image-two-opt-in: 33.166 dB, 0.877 |
| image-aggressive+PX03 | png-alpha | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| image-aggressive+PX04 | png-alpha | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 41.762 | 0.981 | 41.762 | 0.981 | 41.5 dB, 0.980 | n/a |
| image-aggressive+PX05 | png-alpha | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 33.830 | 0.892 | 33.830 | 0.892 | 33.5 dB, 0.890 | n/a |
| image-aggressive+PX06 | png-alpha | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| image-two-opt-in | png-alpha | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 33.166 | 0.877 | 33.166 | 0.877 | 33.0 dB, 0.875 | n/a |
| PX01 | png-alpha | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| PX02 | png-alpha | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| PX03 | png-alpha | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| PX04 | png-alpha | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 41.752 | 0.980 | 41.752 | 0.980 | 41.5 dB, 0.980 | n/a |
| PX05 | png-alpha | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 33.848 | 0.890 | 33.848 | 0.890 | 33.5 dB, 0.890 | n/a |
| PX06 | png-alpha | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| image-safe | jpeg | small+medium | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | unqualified | 34.702 | 0.952 | 34.702 | 0.952 | 34.5 dB, 0.950 | image-aggressive: 30.431 dB, 0.808 |
| image-safe | jpeg | large | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | unqualified | 34.666 | 0.976 | 34.666 | 0.976 | 34.5 dB, 0.975 | image-aggressive: 30.572 dB, 0.893 |
| image-aggressive | jpeg | small+medium | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | unqualified | 30.431 | 0.808 | 30.431 | 0.808 | 30.0 dB, 0.805 | image-two-opt-in: 29.913 dB, 0.784 |
| image-aggressive | jpeg | large | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | unqualified | 30.572 | 0.893 | 30.572 | 0.893 | 30.5 dB, 0.890 | image-two-opt-in: 30.051 dB, 0.878 |
| image-aggressive+PX03 | jpeg | small+medium | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 34.765 | 0.951 | 34.765 | 0.951 | 34.5 dB, 0.950 | n/a |
| image-aggressive+PX03 | jpeg | large | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 34.730 | 0.975 | 34.730 | 0.975 | 34.5 dB, 0.975 | n/a |
| image-aggressive+PX04 | jpeg | small+medium | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 33.970 | 0.933 | 33.970 | 0.933 | 33.5 dB, 0.930 | n/a |
| image-aggressive+PX04 | jpeg | large | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 33.972 | 0.965 | 33.972 | 0.965 | 33.5 dB, 0.965 | n/a |
| image-aggressive+PX05 | jpeg | small+medium | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 30.431 | 0.808 | 30.431 | 0.808 | 30.0 dB, 0.805 | n/a |
| image-aggressive+PX05 | jpeg | large | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 30.572 | 0.893 | 30.572 | 0.893 | 30.5 dB, 0.890 | n/a |
| image-aggressive+PX06 | jpeg | small+medium | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 34.702 | 0.952 | 34.702 | 0.952 | 34.5 dB, 0.950 | n/a |
| image-aggressive+PX06 | jpeg | large | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 34.679 | 0.975 | 34.679 | 0.975 | 34.5 dB, 0.975 | n/a |
| image-two-opt-in | jpeg | small+medium | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 29.913 | 0.784 | 29.913 | 0.784 | 29.5 dB, 0.780 | n/a |
| image-two-opt-in | jpeg | large | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 30.051 | 0.878 | 30.051 | 0.878 | 30.0 dB, 0.875 | n/a |
| PX01 | jpeg | small+medium | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 36.218 | 0.971 | 36.218 | 0.971 | 36.0 dB, 0.970 | n/a |
| PX01 | jpeg | large | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 36.260 | 0.982 | 36.260 | 0.982 | 36.0 dB, 0.980 | n/a |
| PX02 | jpeg | small+medium | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 34.702 | 0.952 | 34.702 | 0.952 | 34.5 dB, 0.950 | n/a |
| PX02 | jpeg | large | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 34.666 | 0.976 | 34.666 | 0.976 | 34.5 dB, 0.975 | n/a |
| PX03 | jpeg | small+medium | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 33.936 | 0.938 | 33.936 | 0.938 | 33.5 dB, 0.935 | n/a |
| PX03 | jpeg | large | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 33.820 | 0.963 | 33.820 | 0.963 | 33.5 dB, 0.960 | n/a |
| PX04 | jpeg | small+medium | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 35.009 | 0.952 | 35.009 | 0.952 | 35.0 dB, 0.950 | n/a |
| PX04 | jpeg | large | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 35.057 | 0.971 | 35.057 | 0.971 | 35.0 dB, 0.970 | n/a |
| PX05 | jpeg | small+medium | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 31.049 | 0.831 | 31.049 | 0.831 | 31.0 dB, 0.830 | n/a |
| PX05 | jpeg | large | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 31.078 | 0.894 | 31.078 | 0.894 | 31.0 dB, 0.890 | n/a |
| PX06 | jpeg | small+medium | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 36.217 | 0.971 | 36.217 | 0.971 | 36.0 dB, 0.970 | n/a |
| PX06 | jpeg | large | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 36.260 | 0.982 | 36.260 | 0.982 | 36.0 dB, 0.980 | n/a |
| image-safe | webp | medium+large | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | unqualified | 35.278 | 0.972 | 35.278 | 0.972 | 35.0 dB, 0.970 | image-aggressive: 30.728 dB, 0.871 |
| image-aggressive | webp | medium+large | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | unqualified | 30.728 | 0.871 | 30.728 | 0.871 | 30.5 dB, 0.870 | image-two-opt-in: 30.151 dB, 0.854 |
| image-aggressive+PX03 | webp | medium+large | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 35.350 | 0.973 | 35.350 | 0.973 | 35.0 dB, 0.970 | n/a |
| image-aggressive+PX04 | webp | medium+large | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 34.525 | 0.960 | 34.525 | 0.960 | 34.5 dB, 0.960 | n/a |
| image-aggressive+PX05 | webp | medium+large | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 30.728 | 0.871 | 30.728 | 0.871 | 30.5 dB, 0.870 | n/a |
| image-aggressive+PX06 | webp | medium+large | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 35.274 | 0.972 | 35.274 | 0.972 | 35.0 dB, 0.970 | n/a |
| image-two-opt-in | webp | medium+large | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 30.151 | 0.854 | 30.151 | 0.854 | 30.0 dB, 0.850 | n/a |
| PX01 | webp | medium+large | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 34.363 | 0.957 | 34.363 | 0.957 | 34.0 dB, 0.955 | n/a |
| PX02 | webp | medium+large | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| PX03 | webp | medium+large | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| PX04 | webp | medium+large | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 40.529 | 0.980 | 40.529 | 0.980 | 40.5 dB, 0.980 | n/a |
| PX05 | webp | medium+large | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 32.550 | 0.889 | 32.550 | 0.889 | 32.5 dB, 0.885 | n/a |
| PX06 | webp | medium+large | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| image-safe | webp-alpha | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | unqualified | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | image-aggressive: 33.825 dB, 0.839 |
| image-aggressive | webp-alpha | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | unqualified | 33.825 | 0.839 | 33.825 | 0.839 | 33.5 dB, 0.835 | image-two-opt-in: 33.166 dB, 0.817 |
| image-aggressive+PX03 | webp-alpha | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| image-aggressive+PX04 | webp-alpha | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 41.777 | 0.970 | 41.777 | 0.970 | 41.5 dB, 0.970 | n/a |
| image-aggressive+PX05 | webp-alpha | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 33.825 | 0.839 | 33.825 | 0.839 | 33.5 dB, 0.835 | n/a |
| image-aggressive+PX06 | webp-alpha | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| image-two-opt-in | webp-alpha | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 33.166 | 0.817 | 33.166 | 0.817 | 33.0 dB, 0.815 | n/a |
| PX01 | webp-alpha | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| PX02 | webp-alpha | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| PX03 | webp-alpha | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| PX04 | webp-alpha | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 41.764 | 0.972 | 41.764 | 0.972 | 41.5 dB, 0.970 | n/a |
| PX05 | webp-alpha | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 33.834 | 0.848 | 33.834 | 0.848 | 33.5 dB, 0.845 | n/a |
| PX06 | webp-alpha | small+medium | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |

## Audio cells

| Plan | Format | Band | Derived | n | Bases | Pixel-identical lost | Gen. max share | Classes | Missing classes | Held | Status | P95 LSD | Median LSD | Max LSD | Cell ceiling | Next tier median |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| audio-safe | wav | short+medium | none | 2 | 2 | 0 | 1.00 | tonal synthetic 2 | music, speech, ambient, tonal-synthetic | 0 | unqualified | 1.558 | 1.130 | 1.558 | 1.60 dB | audio-aggressive: 5.253 dB |
| audio-safe | wav | long | none | 1 | 1 | 0 | 1.00 | tonal synthetic 1 | music, speech, ambient, tonal-synthetic | 0 | unqualified | 1.605 | 1.605 | 1.605 | 1.65 dB | audio-aggressive: 4.670 dB |
| audio-aggressive | wav | short+medium | none | 2 | 2 | 0 | 1.00 | tonal synthetic 2 | music, speech, ambient, tonal-synthetic | 0 | unqualified | 9.047 | 5.253 | 9.047 | 9.05 dB | audio-two-opt-in: 5.483 dB |
| audio-aggressive | wav | long | none | 1 | 1 | 0 | 1.00 | tonal synthetic 1 | music, speech, ambient, tonal-synthetic | 0 | unqualified | 4.670 | 4.670 | 4.670 | 4.70 dB | audio-two-opt-in: 4.888 dB |
| audio-aggressive+AU04 | wav | short+medium | none | 2 | 2 | 0 | 1.00 | tonal synthetic 2 | music, speech, ambient, tonal-synthetic | 0 | informational | 2.006 | 1.690 | 2.006 | 2.05 dB | n/a |
| audio-aggressive+AU04 | wav | long | none | 1 | 1 | 0 | 1.00 | tonal synthetic 1 | music, speech, ambient, tonal-synthetic | 0 | informational | 2.033 | 2.033 | 2.033 | 2.05 dB | n/a |
| audio-aggressive+AU05 | wav | short+medium | none | 2 | 2 | 0 | 1.00 | tonal synthetic 2 | music, speech, ambient, tonal-synthetic | 0 | informational | 9.047 | 5.253 | 9.047 | 9.05 dB | n/a |
| audio-aggressive+AU05 | wav | long | none | 1 | 1 | 0 | 1.00 | tonal synthetic 1 | music, speech, ambient, tonal-synthetic | 0 | informational | 4.670 | 4.670 | 4.670 | 4.70 dB | n/a |
| audio-two-opt-in | wav | short+medium | none | 2 | 2 | 0 | 1.00 | tonal synthetic 2 | music, speech, ambient, tonal-synthetic | 0 | informational | 9.106 | 5.483 | 9.106 | 9.15 dB | n/a |
| audio-two-opt-in | wav | long | none | 1 | 1 | 0 | 1.00 | tonal synthetic 1 | music, speech, ambient, tonal-synthetic | 0 | informational | 4.888 | 4.888 | 4.888 | 4.90 dB | n/a |
| AU01 | wav | short+medium | none | 2 | 2 | 0 | 1.00 | tonal synthetic 2 | music, speech, ambient, tonal-synthetic | 0 | informational | 1.557 | 1.130 | 1.557 | 1.60 dB | n/a |
| AU01 | wav | long | none | 1 | 1 | 0 | 1.00 | tonal synthetic 1 | music, speech, ambient, tonal-synthetic | 0 | informational | 1.604 | 1.604 | 1.604 | 1.65 dB | n/a |
| AU02 | wav | short+medium | none | 2 | 2 | 0 | 1.00 | tonal synthetic 2 | music, speech, ambient, tonal-synthetic | 0 | informational | 0.056 | 0.045 | 0.056 | 0.10 dB | n/a |
| AU02 | wav | long | none | 1 | 1 | 0 | 1.00 | tonal synthetic 1 | music, speech, ambient, tonal-synthetic | 0 | informational | 0.053 | 0.053 | 0.053 | 0.10 dB | n/a |
| AU03 | wav | short+medium | none | 0 | 0 | 0 | 0.00 | none | music, speech, ambient, tonal-synthetic | 2 | informational | n/a | n/a | n/a | n/a | n/a |
| AU03 | wav | long | none | 0 | 0 | 0 | 0.00 | none | music, speech, ambient, tonal-synthetic | 1 | informational | n/a | n/a | n/a | n/a | n/a |
| AU04 | wav | short+medium | none | 2 | 2 | 0 | 1.00 | tonal synthetic 2 | music, speech, ambient, tonal-synthetic | 0 | informational | 1.243 | 1.226 | 1.243 | 1.25 dB | n/a |
| AU04 | wav | long | none | 1 | 1 | 0 | 1.00 | tonal synthetic 1 | music, speech, ambient, tonal-synthetic | 0 | informational | 1.186 | 1.186 | 1.186 | 1.20 dB | n/a |
| AU05 | wav | short+medium | none | 2 | 2 | 0 | 1.00 | tonal synthetic 2 | music, speech, ambient, tonal-synthetic | 0 | informational | 9.044 | 5.252 | 9.044 | 9.05 dB | n/a |
| AU05 | wav | long | none | 1 | 1 | 0 | 1.00 | tonal synthetic 1 | music, speech, ambient, tonal-synthetic | 0 | informational | 4.239 | 4.239 | 4.239 | 4.25 dB | n/a |
| audio-safe | flac | short+medium | none | 2 | 2 | 0 | 1.00 | tonal synthetic 2 | music, speech, ambient, tonal-synthetic | 0 | unqualified | 1.913 | 1.591 | 1.913 | 1.95 dB | audio-aggressive: 5.158 dB |
| audio-safe | flac | long | none | 1 | 1 | 0 | 1.00 | tonal synthetic 1 | music, speech, ambient, tonal-synthetic | 0 | unqualified | 1.553 | 1.553 | 1.553 | 1.60 dB | audio-aggressive: 3.637 dB |
| audio-aggressive | flac | short+medium | none | 2 | 2 | 0 | 1.00 | tonal synthetic 2 | music, speech, ambient, tonal-synthetic | 0 | unqualified | 10.216 | 5.158 | 10.216 | 10.25 dB | audio-two-opt-in: 5.370 dB |
| audio-aggressive | flac | long | none | 1 | 1 | 0 | 1.00 | tonal synthetic 1 | music, speech, ambient, tonal-synthetic | 0 | unqualified | 3.637 | 3.637 | 3.637 | 3.65 dB | audio-two-opt-in: 3.715 dB |
| audio-aggressive+AU04 | flac | short+medium | none | 2 | 2 | 0 | 1.00 | tonal synthetic 2 | music, speech, ambient, tonal-synthetic | 0 | informational | 2.260 | 2.038 | 2.260 | 2.30 dB | n/a |
| audio-aggressive+AU04 | flac | long | none | 1 | 1 | 0 | 1.00 | tonal synthetic 1 | music, speech, ambient, tonal-synthetic | 0 | informational | 2.006 | 2.006 | 2.006 | 2.05 dB | n/a |
| audio-aggressive+AU05 | flac | short+medium | none | 2 | 2 | 0 | 1.00 | tonal synthetic 2 | music, speech, ambient, tonal-synthetic | 0 | informational | 10.216 | 5.158 | 10.216 | 10.25 dB | n/a |
| audio-aggressive+AU05 | flac | long | none | 1 | 1 | 0 | 1.00 | tonal synthetic 1 | music, speech, ambient, tonal-synthetic | 0 | informational | 3.637 | 3.637 | 3.637 | 3.65 dB | n/a |
| audio-two-opt-in | flac | short+medium | none | 2 | 2 | 0 | 1.00 | tonal synthetic 2 | music, speech, ambient, tonal-synthetic | 0 | informational | 10.279 | 5.370 | 10.279 | 10.30 dB | n/a |
| audio-two-opt-in | flac | long | none | 1 | 1 | 0 | 1.00 | tonal synthetic 1 | music, speech, ambient, tonal-synthetic | 0 | informational | 3.715 | 3.715 | 3.715 | 3.75 dB | n/a |
| AU01 | flac | short+medium | none | 2 | 2 | 0 | 1.00 | tonal synthetic 2 | music, speech, ambient, tonal-synthetic | 0 | informational | 1.913 | 1.590 | 1.913 | 1.95 dB | n/a |
| AU01 | flac | long | none | 1 | 1 | 0 | 1.00 | tonal synthetic 1 | music, speech, ambient, tonal-synthetic | 0 | informational | 1.552 | 1.552 | 1.552 | 1.60 dB | n/a |
| AU02 | flac | short+medium | none | 2 | 2 | 0 | 1.00 | tonal synthetic 2 | music, speech, ambient, tonal-synthetic | 0 | informational | 0.056 | 0.044 | 0.056 | 0.10 dB | n/a |
| AU02 | flac | long | none | 1 | 1 | 0 | 1.00 | tonal synthetic 1 | music, speech, ambient, tonal-synthetic | 0 | informational | 0.055 | 0.055 | 0.055 | 0.10 dB | n/a |
| AU03 | flac | short+medium | none | 0 | 0 | 0 | 0.00 | none | music, speech, ambient, tonal-synthetic | 2 | informational | n/a | n/a | n/a | n/a | n/a |
| AU03 | flac | long | none | 0 | 0 | 0 | 0.00 | none | music, speech, ambient, tonal-synthetic | 1 | informational | n/a | n/a | n/a | n/a | n/a |
| AU04 | flac | short+medium | none | 2 | 2 | 0 | 1.00 | tonal synthetic 2 | music, speech, ambient, tonal-synthetic | 0 | informational | 1.245 | 1.224 | 1.245 | 1.25 dB | n/a |
| AU04 | flac | long | none | 1 | 1 | 0 | 1.00 | tonal synthetic 1 | music, speech, ambient, tonal-synthetic | 0 | informational | 1.227 | 1.227 | 1.227 | 1.25 dB | n/a |
| AU05 | flac | short+medium | none | 2 | 2 | 0 | 1.00 | tonal synthetic 2 | music, speech, ambient, tonal-synthetic | 0 | informational | 10.214 | 4.982 | 10.214 | 10.25 dB | n/a |
| AU05 | flac | long | none | 1 | 1 | 0 | 1.00 | tonal synthetic 1 | music, speech, ambient, tonal-synthetic | 0 | informational | 3.628 | 3.628 | 3.628 | 3.65 dB | n/a |

## Report-only rows

Controls, MCU-padded images, short clips, report-only groups by reason, and the documented-marks splits. Reported beside the cells and never used to set a number.

| Plan | Format | Band | Derived | n | Bases | Pixel-identical lost | Gen. max share | Classes | Missing classes | Held | Status | P5 PSNR | P5 SSIM | Median PSNR | Median SSIM | Cell floor | Next tier median |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| image-safe | png | mcu-padded | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 35.287 | 0.955 | 35.287 | 0.955 | 35.0 dB, 0.955 | image-aggressive: 30.562 dB, 0.807 |
| image-aggressive | png | mcu-padded | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 30.562 | 0.807 | 30.562 | 0.807 | 30.5 dB, 0.805 | image-two-opt-in: 30.017 dB, 0.781 |
| image-safe | png-alpha | mcu-padded | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | image-aggressive: 33.830 dB, 0.892 |
| image-aggressive | png-alpha | mcu-padded | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 33.830 | 0.892 | 33.830 | 0.892 | 33.5 dB, 0.890 | image-two-opt-in: 33.166 dB, 0.877 |
| image-safe | jpeg | mcu-padded | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 34.783 | 0.970 | 34.783 | 0.970 | 34.5 dB, 0.970 | image-aggressive: 30.555 dB, 0.872 |
| image-aggressive | jpeg | mcu-padded | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 30.555 | 0.872 | 30.555 | 0.872 | 30.5 dB, 0.870 | image-two-opt-in: 30.030 dB, 0.854 |
| image-safe | webp | mcu-padded | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 35.278 | 0.972 | 35.278 | 0.972 | 35.0 dB, 0.970 | image-aggressive: 30.728 dB, 0.871 |
| image-aggressive | webp | mcu-padded | none | 2 | 2 | 0 | 1.00 | dense texture 2 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 30.728 | 0.871 | 30.728 | 0.871 | 30.5 dB, 0.870 | image-two-opt-in: 30.151 dB, 0.854 |
| image-safe | webp-alpha | mcu-padded | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | image-aggressive: 33.825 dB, 0.839 |
| image-aggressive | webp-alpha | mcu-padded | none | 1 | 1 | 0 | 1.00 | dense texture 1 | photographic, flat-illustration, dense-texture, text-ui | 0 | informational | 33.825 | 0.839 | 33.825 | 0.839 | 33.5 dB, 0.835 | image-two-opt-in: 33.166 dB, 0.817 |

| Plan | Format | Band | Derived | n | Bases | Pixel-identical lost | Gen. max share | Classes | Missing classes | Held | Status | P95 LSD | Median LSD | Max LSD | Cell ceiling | Next tier median |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| audio-safe | wav | short-clip | none | 1 | 1 | 0 | 1.00 | tonal synthetic 1 | music, speech, ambient, tonal-synthetic | 0 | informational | 1.130 | 1.130 | 1.130 | 1.15 dB | audio-aggressive: 5.253 dB |
| audio-aggressive | wav | short-clip | none | 1 | 1 | 0 | 1.00 | tonal synthetic 1 | music, speech, ambient, tonal-synthetic | 0 | informational | 5.253 | 5.253 | 5.253 | 5.30 dB | audio-two-opt-in: 5.483 dB |
| audio-safe | flac | short-clip | none | 1 | 1 | 0 | 1.00 | tonal synthetic 1 | music, speech, ambient, tonal-synthetic | 0 | informational | 1.913 | 1.913 | 1.913 | 1.95 dB | audio-aggressive: 5.158 dB |
| audio-aggressive | flac | short-clip | none | 1 | 1 | 0 | 1.00 | tonal synthetic 1 | music, speech, ambient, tonal-synthetic | 0 | informational | 5.158 | 5.158 | 5.158 | 5.20 dB | audio-two-opt-in: 5.370 dB |

## Post-processed ladder

Pass and refuse counts of the safe and aggressive plans over post-processed assets, per degradation step, against the budget in force. Report only; nothing here feeds a percentile.

None.

## Metadata identity

| Plan | Format | n | Identical | Failures | Declined |
|---|---|---|---|---|---|
| image-metadata | png | 2 | yes | 0 | 0 |
| image-metadata | png-alpha | 1 | yes | 0 | 0 |
| image-metadata | jpeg | 3 | yes | 0 | 0 |
| image-metadata | webp | 2 | yes | 0 | 0 |
| image-metadata | webp-alpha | 1 | yes | 0 | 0 |
| audio-metadata | wav | 3 | yes | 0 | 0 |
| audio-metadata | flac | 3 | yes | 0 | 0 |

## Properties

| Property | Cell | Pass | Detail |
|---|---|---|---|
| qualified-cells-present | all | no | no cell reached n_min with every required class at class_min under the generator cap; every number stays provisional |
| aggressive-never-stricter-than-safe | all | yes | holds for every format |
| metadata-byte-identity | all | yes | 7 identity cells, every decoded stream identical |
| encoder-fingerprint | all | yes | unmark 0.1.0; container-rewrite/no-reencode; png 0.18.1; jpeg-decoder 0.3.2; image-webp 0.2.4; claxon 0.4.3; flacenc 0.5.1; jpeg-encoder in-crate v1 baseline 4:4:4; resample in-crate lanczos3 and kaiser-sinc scalar; trig in-crate libm-free |
| determinism | all | yes | two passes over the manifest produced byte-identical records |

## Exclusions by reason

None.

## Fixture pins

The fixture subset is synthetic broadband, worst case: procedural patterns and tones with a noise bed, so its scores sit below what generated content scores and set no number.

192 fixture rows pinned by `tests/calibration.rs` at three decimals: 171 scored, 6 held, 15 identity.
