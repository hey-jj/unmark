# unmark calibration record

Generated from `calibration.json` by the calibration harness. Edits here are overwritten.

- date: 2026-09-02
- policy version: 0.2.0
- corpus manifest: manifest.json (sha256 cc4a4fa43d5189384ca60b8fff7b4985a000b28a34c7a40c6a56e12c69689f47)
- assets: 15 listed, 15 scored, 0 excluded
- encoder fingerprint: unmark 0.1.0; container-rewrite/no-reencode; png 0.18.1; jpeg-decoder 0.3.2; image-webp 0.2.4; claxon 0.4.3; flacenc 0.5.1; jpeg-encoder in-crate baseline 4:4:4; resample in-crate lanczos3 and kaiser-sinc scalar
- base seed: 0x5eed202609020001
- derivation: image floors at the 5th percentile rounded down to 0.5 dB and 0.005, audio ceiling at the 95th percentile rounded up to 0.05 dB, n_min 40
- accepted: no, a property failed or nothing qualified

## Derived numbers

| Budget | Provisional | Calibrated | Source | Overrides |
|---|---|---|---|---|
| image-safe | PSNR 38.0 dB, SSIM 0.980 | PSNR 38.0 dB, SSIM 0.980 | provisional | none |
| image-aggressive | PSNR 30.0 dB, SSIM 0.900 | PSNR 30.0 dB, SSIM 0.900 | provisional | none |
| audio-safe | LSD 1.00 dB | LSD 1.00 dB | provisional | none |
| audio-aggressive | LSD 3.00 dB | LSD 3.00 dB | provisional | none |

## Image cells

| Plan | Format | Band | n | Held | Status | P5 PSNR | P5 SSIM | Median PSNR | Median SSIM | Cell floor | Next tier median |
|---|---|---|---|---|---|---|---|---|---|---|---|
| image-safe | png | small+medium | 0 | 2 | held | n/a | n/a | n/a | n/a | n/a | image-aggressive: n/a dB, n/a |
| image-safe | png | large | 0 | 1 | held | n/a | n/a | n/a | n/a | n/a | image-aggressive: n/a dB, n/a |
| image-aggressive | png | small+medium | 0 | 2 | held | n/a | n/a | n/a | n/a | n/a | image-two-opt-in: n/a dB, n/a |
| image-aggressive | png | large | 0 | 1 | held | n/a | n/a | n/a | n/a | n/a | image-two-opt-in: n/a dB, n/a |
| image-aggressive+PX03 | png | small+medium | 0 | 2 | informational | n/a | n/a | n/a | n/a | n/a | n/a |
| image-aggressive+PX03 | png | large | 0 | 1 | informational | n/a | n/a | n/a | n/a | n/a | n/a |
| image-aggressive+PX04 | png | small+medium | 0 | 2 | informational | n/a | n/a | n/a | n/a | n/a | n/a |
| image-aggressive+PX04 | png | large | 0 | 1 | informational | n/a | n/a | n/a | n/a | n/a | n/a |
| image-aggressive+PX05 | png | small+medium | 0 | 2 | informational | n/a | n/a | n/a | n/a | n/a | n/a |
| image-aggressive+PX05 | png | large | 0 | 1 | informational | n/a | n/a | n/a | n/a | n/a | n/a |
| image-aggressive+PX06 | png | small+medium | 0 | 2 | informational | n/a | n/a | n/a | n/a | n/a | n/a |
| image-aggressive+PX06 | png | large | 0 | 1 | informational | n/a | n/a | n/a | n/a | n/a | n/a |
| image-two-opt-in | png | small+medium | 0 | 2 | informational | n/a | n/a | n/a | n/a | n/a | n/a |
| image-two-opt-in | png | large | 0 | 1 | informational | n/a | n/a | n/a | n/a | n/a | n/a |
| PX01 | png | small+medium | 0 | 2 | informational | n/a | n/a | n/a | n/a | n/a | n/a |
| PX01 | png | large | 0 | 1 | informational | n/a | n/a | n/a | n/a | n/a | n/a |
| PX02 | png | small+medium | 2 | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| PX02 | png | large | 1 | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| PX03 | png | small+medium | 2 | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| PX03 | png | large | 1 | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| PX04 | png | small+medium | 2 | 0 | informational | 40.498 | 0.972 | 40.498 | 0.972 | 40.0 dB, 0.970 | n/a |
| PX04 | png | large | 1 | 0 | informational | 40.518 | 0.984 | 40.518 | 0.984 | 40.5 dB, 0.980 | n/a |
| PX05 | png | small+medium | 2 | 0 | informational | 32.549 | 0.846 | 32.549 | 0.846 | 32.5 dB, 0.845 | n/a |
| PX05 | png | large | 1 | 0 | informational | 32.556 | 0.905 | 32.556 | 0.905 | 32.5 dB, 0.905 | n/a |
| PX06 | png | small+medium | 2 | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| PX06 | png | large | 1 | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| image-safe | jpeg | small+medium | 2 | 0 | unqualified | 34.702 | 0.952 | 34.702 | 0.952 | 34.5 dB, 0.950 | image-aggressive: 30.431 dB, 0.808 |
| image-safe | jpeg | large | 1 | 0 | unqualified | 34.666 | 0.976 | 34.666 | 0.976 | 34.5 dB, 0.975 | image-aggressive: 30.572 dB, 0.893 |
| image-aggressive | jpeg | small+medium | 2 | 0 | unqualified | 30.431 | 0.808 | 30.431 | 0.808 | 30.0 dB, 0.805 | image-two-opt-in: 29.913 dB, 0.784 |
| image-aggressive | jpeg | large | 1 | 0 | unqualified | 30.572 | 0.893 | 30.572 | 0.893 | 30.5 dB, 0.890 | image-two-opt-in: 30.051 dB, 0.878 |
| image-aggressive+PX03 | jpeg | small+medium | 2 | 0 | informational | 34.765 | 0.951 | 34.765 | 0.951 | 34.5 dB, 0.950 | n/a |
| image-aggressive+PX03 | jpeg | large | 1 | 0 | informational | 34.730 | 0.975 | 34.730 | 0.975 | 34.5 dB, 0.975 | n/a |
| image-aggressive+PX04 | jpeg | small+medium | 2 | 0 | informational | 33.970 | 0.933 | 33.970 | 0.933 | 33.5 dB, 0.930 | n/a |
| image-aggressive+PX04 | jpeg | large | 1 | 0 | informational | 33.972 | 0.965 | 33.972 | 0.965 | 33.5 dB, 0.965 | n/a |
| image-aggressive+PX05 | jpeg | small+medium | 2 | 0 | informational | 30.431 | 0.808 | 30.431 | 0.808 | 30.0 dB, 0.805 | n/a |
| image-aggressive+PX05 | jpeg | large | 1 | 0 | informational | 30.572 | 0.893 | 30.572 | 0.893 | 30.5 dB, 0.890 | n/a |
| image-aggressive+PX06 | jpeg | small+medium | 2 | 0 | informational | 34.702 | 0.952 | 34.702 | 0.952 | 34.5 dB, 0.950 | n/a |
| image-aggressive+PX06 | jpeg | large | 1 | 0 | informational | 34.679 | 0.975 | 34.679 | 0.975 | 34.5 dB, 0.975 | n/a |
| image-two-opt-in | jpeg | small+medium | 2 | 0 | informational | 29.913 | 0.784 | 29.913 | 0.784 | 29.5 dB, 0.780 | n/a |
| image-two-opt-in | jpeg | large | 1 | 0 | informational | 30.051 | 0.878 | 30.051 | 0.878 | 30.0 dB, 0.875 | n/a |
| PX01 | jpeg | small+medium | 2 | 0 | informational | 36.218 | 0.971 | 36.218 | 0.971 | 36.0 dB, 0.970 | n/a |
| PX01 | jpeg | large | 1 | 0 | informational | 36.260 | 0.982 | 36.260 | 0.982 | 36.0 dB, 0.980 | n/a |
| PX02 | jpeg | small+medium | 2 | 0 | informational | 34.702 | 0.952 | 34.702 | 0.952 | 34.5 dB, 0.950 | n/a |
| PX02 | jpeg | large | 1 | 0 | informational | 34.666 | 0.976 | 34.666 | 0.976 | 34.5 dB, 0.975 | n/a |
| PX03 | jpeg | small+medium | 2 | 0 | informational | 33.936 | 0.938 | 33.936 | 0.938 | 33.5 dB, 0.935 | n/a |
| PX03 | jpeg | large | 1 | 0 | informational | 33.820 | 0.963 | 33.820 | 0.963 | 33.5 dB, 0.960 | n/a |
| PX04 | jpeg | small+medium | 2 | 0 | informational | 35.009 | 0.952 | 35.009 | 0.952 | 35.0 dB, 0.950 | n/a |
| PX04 | jpeg | large | 1 | 0 | informational | 35.057 | 0.971 | 35.057 | 0.971 | 35.0 dB, 0.970 | n/a |
| PX05 | jpeg | small+medium | 2 | 0 | informational | 31.049 | 0.831 | 31.049 | 0.831 | 31.0 dB, 0.830 | n/a |
| PX05 | jpeg | large | 1 | 0 | informational | 31.078 | 0.894 | 31.078 | 0.894 | 31.0 dB, 0.890 | n/a |
| PX06 | jpeg | small+medium | 2 | 0 | informational | 36.217 | 0.971 | 36.217 | 0.971 | 36.0 dB, 0.970 | n/a |
| PX06 | jpeg | large | 1 | 0 | informational | 36.260 | 0.982 | 36.260 | 0.982 | 36.0 dB, 0.980 | n/a |
| image-safe | webp | small+medium | 0 | 2 | held | n/a | n/a | n/a | n/a | n/a | image-aggressive: n/a dB, n/a |
| image-safe | webp | large | 0 | 1 | held | n/a | n/a | n/a | n/a | n/a | image-aggressive: n/a dB, n/a |
| image-aggressive | webp | small+medium | 0 | 2 | held | n/a | n/a | n/a | n/a | n/a | image-two-opt-in: n/a dB, n/a |
| image-aggressive | webp | large | 0 | 1 | held | n/a | n/a | n/a | n/a | n/a | image-two-opt-in: n/a dB, n/a |
| image-aggressive+PX03 | webp | small+medium | 0 | 2 | informational | n/a | n/a | n/a | n/a | n/a | n/a |
| image-aggressive+PX03 | webp | large | 0 | 1 | informational | n/a | n/a | n/a | n/a | n/a | n/a |
| image-aggressive+PX04 | webp | small+medium | 0 | 2 | informational | n/a | n/a | n/a | n/a | n/a | n/a |
| image-aggressive+PX04 | webp | large | 0 | 1 | informational | n/a | n/a | n/a | n/a | n/a | n/a |
| image-aggressive+PX05 | webp | small+medium | 0 | 2 | informational | n/a | n/a | n/a | n/a | n/a | n/a |
| image-aggressive+PX05 | webp | large | 0 | 1 | informational | n/a | n/a | n/a | n/a | n/a | n/a |
| image-aggressive+PX06 | webp | small+medium | 0 | 2 | informational | n/a | n/a | n/a | n/a | n/a | n/a |
| image-aggressive+PX06 | webp | large | 0 | 1 | informational | n/a | n/a | n/a | n/a | n/a | n/a |
| image-two-opt-in | webp | small+medium | 0 | 2 | informational | n/a | n/a | n/a | n/a | n/a | n/a |
| image-two-opt-in | webp | large | 0 | 1 | informational | n/a | n/a | n/a | n/a | n/a | n/a |
| PX01 | webp | small+medium | 0 | 2 | informational | n/a | n/a | n/a | n/a | n/a | n/a |
| PX01 | webp | large | 0 | 1 | informational | n/a | n/a | n/a | n/a | n/a | n/a |
| PX02 | webp | small+medium | 2 | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| PX02 | webp | large | 1 | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| PX03 | webp | small+medium | 2 | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| PX03 | webp | large | 1 | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| PX04 | webp | small+medium | 2 | 0 | informational | 40.539 | 0.972 | 40.539 | 0.972 | 40.5 dB, 0.970 | n/a |
| PX04 | webp | large | 1 | 0 | informational | 40.529 | 0.983 | 40.529 | 0.983 | 40.5 dB, 0.980 | n/a |
| PX05 | webp | small+medium | 2 | 0 | informational | 32.582 | 0.848 | 32.582 | 0.848 | 32.5 dB, 0.845 | n/a |
| PX05 | webp | large | 1 | 0 | informational | 32.550 | 0.904 | 32.550 | 0.904 | 32.5 dB, 0.900 | n/a |
| PX06 | webp | small+medium | 2 | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |
| PX06 | webp | large | 1 | 0 | informational | 120.000 | 1.000 | 120.000 | 1.000 | 120.0 dB, 1.000 | n/a |

## Audio cells

| Plan | Format | Band | n | Held | Status | P95 LSD | Median LSD | Max LSD | Cell ceiling | Next tier median |
|---|---|---|---|---|---|---|---|---|---|---|
| audio-safe | wav | short+medium | 2 | 0 | unqualified | 1.558 | 1.130 | 1.558 | 1.60 dB | audio-aggressive: 5.253 dB |
| audio-safe | wav | long | 1 | 0 | unqualified | 1.605 | 1.605 | 1.605 | 1.65 dB | audio-aggressive: 4.670 dB |
| audio-aggressive | wav | short+medium | 2 | 0 | unqualified | 9.047 | 5.253 | 9.047 | 9.05 dB | audio-two-opt-in: 5.483 dB |
| audio-aggressive | wav | long | 1 | 0 | unqualified | 4.670 | 4.670 | 4.670 | 4.70 dB | audio-two-opt-in: 4.888 dB |
| audio-aggressive+AU04 | wav | short+medium | 2 | 0 | informational | 2.006 | 1.690 | 2.006 | 2.05 dB | n/a |
| audio-aggressive+AU04 | wav | long | 1 | 0 | informational | 2.033 | 2.033 | 2.033 | 2.05 dB | n/a |
| audio-aggressive+AU05 | wav | short+medium | 2 | 0 | informational | 9.047 | 5.253 | 9.047 | 9.05 dB | n/a |
| audio-aggressive+AU05 | wav | long | 1 | 0 | informational | 4.670 | 4.670 | 4.670 | 4.70 dB | n/a |
| audio-two-opt-in | wav | short+medium | 2 | 0 | informational | 9.106 | 5.483 | 9.106 | 9.15 dB | n/a |
| audio-two-opt-in | wav | long | 1 | 0 | informational | 4.888 | 4.888 | 4.888 | 4.90 dB | n/a |
| AU01 | wav | short+medium | 2 | 0 | informational | 1.557 | 1.130 | 1.557 | 1.60 dB | n/a |
| AU01 | wav | long | 1 | 0 | informational | 1.604 | 1.604 | 1.604 | 1.65 dB | n/a |
| AU02 | wav | short+medium | 2 | 0 | informational | 0.056 | 0.045 | 0.056 | 0.10 dB | n/a |
| AU02 | wav | long | 1 | 0 | informational | 0.053 | 0.053 | 0.053 | 0.10 dB | n/a |
| AU03 | wav | short+medium | 0 | 2 | informational | n/a | n/a | n/a | n/a | n/a |
| AU03 | wav | long | 0 | 1 | informational | n/a | n/a | n/a | n/a | n/a |
| AU04 | wav | short+medium | 2 | 0 | informational | 1.243 | 1.226 | 1.243 | 1.25 dB | n/a |
| AU04 | wav | long | 1 | 0 | informational | 1.186 | 1.186 | 1.186 | 1.20 dB | n/a |
| AU05 | wav | short+medium | 2 | 0 | informational | 9.044 | 5.252 | 9.044 | 9.05 dB | n/a |
| AU05 | wav | long | 1 | 0 | informational | 4.239 | 4.239 | 4.239 | 4.25 dB | n/a |
| audio-safe | flac | short+medium | 2 | 0 | unqualified | 1.913 | 1.591 | 1.913 | 1.95 dB | audio-aggressive: 5.158 dB |
| audio-safe | flac | long | 1 | 0 | unqualified | 1.553 | 1.553 | 1.553 | 1.60 dB | audio-aggressive: 3.637 dB |
| audio-aggressive | flac | short+medium | 2 | 0 | unqualified | 10.216 | 5.158 | 10.216 | 10.25 dB | audio-two-opt-in: 5.370 dB |
| audio-aggressive | flac | long | 1 | 0 | unqualified | 3.637 | 3.637 | 3.637 | 3.65 dB | audio-two-opt-in: 3.715 dB |
| audio-aggressive+AU04 | flac | short+medium | 2 | 0 | informational | 2.260 | 2.038 | 2.260 | 2.30 dB | n/a |
| audio-aggressive+AU04 | flac | long | 1 | 0 | informational | 2.006 | 2.006 | 2.006 | 2.05 dB | n/a |
| audio-aggressive+AU05 | flac | short+medium | 2 | 0 | informational | 10.216 | 5.158 | 10.216 | 10.25 dB | n/a |
| audio-aggressive+AU05 | flac | long | 1 | 0 | informational | 3.637 | 3.637 | 3.637 | 3.65 dB | n/a |
| audio-two-opt-in | flac | short+medium | 2 | 0 | informational | 10.279 | 5.370 | 10.279 | 10.30 dB | n/a |
| audio-two-opt-in | flac | long | 1 | 0 | informational | 3.715 | 3.715 | 3.715 | 3.75 dB | n/a |
| AU01 | flac | short+medium | 2 | 0 | informational | 1.913 | 1.590 | 1.913 | 1.95 dB | n/a |
| AU01 | flac | long | 1 | 0 | informational | 1.552 | 1.552 | 1.552 | 1.60 dB | n/a |
| AU02 | flac | short+medium | 2 | 0 | informational | 0.056 | 0.044 | 0.056 | 0.10 dB | n/a |
| AU02 | flac | long | 1 | 0 | informational | 0.055 | 0.055 | 0.055 | 0.10 dB | n/a |
| AU03 | flac | short+medium | 0 | 2 | informational | n/a | n/a | n/a | n/a | n/a |
| AU03 | flac | long | 0 | 1 | informational | n/a | n/a | n/a | n/a | n/a |
| AU04 | flac | short+medium | 2 | 0 | informational | 1.245 | 1.224 | 1.245 | 1.25 dB | n/a |
| AU04 | flac | long | 1 | 0 | informational | 1.227 | 1.227 | 1.227 | 1.25 dB | n/a |
| AU05 | flac | short+medium | 2 | 0 | informational | 10.214 | 4.982 | 10.214 | 10.25 dB | n/a |
| AU05 | flac | long | 1 | 0 | informational | 3.628 | 3.628 | 3.628 | 3.65 dB | n/a |

## Strata

Control assets, MCU-padded images, and short clips, reported beside the cells and never used to set a number.

| Plan | Format | Band | n | Held | Status | P5 PSNR | P5 SSIM | Median PSNR | Median SSIM | Cell floor | Next tier median |
|---|---|---|---|---|---|---|---|---|---|---|---|
| image-safe | png | mcu-padded | 0 | 3 | informational | n/a | n/a | n/a | n/a | n/a | image-aggressive: n/a dB, n/a |
| image-aggressive | png | mcu-padded | 0 | 3 | informational | n/a | n/a | n/a | n/a | n/a | image-two-opt-in: n/a dB, n/a |
| image-safe | jpeg | mcu-padded | 1 | 0 | informational | 34.783 | 0.970 | 34.783 | 0.970 | 34.5 dB, 0.970 | image-aggressive: 30.555 dB, 0.872 |
| image-aggressive | jpeg | mcu-padded | 1 | 0 | informational | 30.555 | 0.872 | 30.555 | 0.872 | 30.5 dB, 0.870 | image-two-opt-in: 30.030 dB, 0.854 |
| image-safe | webp | mcu-padded | 0 | 3 | informational | n/a | n/a | n/a | n/a | n/a | image-aggressive: n/a dB, n/a |
| image-aggressive | webp | mcu-padded | 0 | 3 | informational | n/a | n/a | n/a | n/a | n/a | image-two-opt-in: n/a dB, n/a |

| Plan | Format | Band | n | Held | Status | P95 LSD | Median LSD | Max LSD | Cell ceiling | Next tier median |
|---|---|---|---|---|---|---|---|---|---|---|
| audio-safe | wav | short-clip | 1 | 0 | informational | 1.130 | 1.130 | 1.130 | 1.15 dB | audio-aggressive: 5.253 dB |
| audio-aggressive | wav | short-clip | 1 | 0 | informational | 5.253 | 5.253 | 5.253 | 5.30 dB | audio-two-opt-in: 5.483 dB |
| audio-safe | flac | short-clip | 1 | 0 | informational | 1.913 | 1.913 | 1.913 | 1.95 dB | audio-aggressive: 5.158 dB |
| audio-aggressive | flac | short-clip | 1 | 0 | informational | 5.158 | 5.158 | 5.158 | 5.20 dB | audio-two-opt-in: 5.370 dB |

## Metadata identity

| Plan | Format | n | Identical | Failures |
|---|---|---|---|---|
| image-metadata | png | 3 | yes | none |
| image-metadata | jpeg | 3 | yes | none |
| image-metadata | webp | 3 | yes | none |
| audio-metadata | wav | 3 | yes | none |
| audio-metadata | flac | 3 | yes | none |

## Properties

| Property | Cell | Pass | Detail |
|---|---|---|---|
| qualified-cells-present | all | no | no cell reached n_min; every number stays provisional |
| aggressive-never-stricter-than-safe | all | yes | holds for every format |
| metadata-byte-identity | all | yes | 5 identity cells, every decoded stream identical |
| encoder-fingerprint | all | yes | unmark 0.1.0; container-rewrite/no-reencode; png 0.18.1; jpeg-decoder 0.3.2; image-webp 0.2.4; claxon 0.4.3; flacenc 0.5.1; jpeg-encoder in-crate baseline 4:4:4; resample in-crate lanczos3 and kaiser-sinc scalar |
| determinism | all | yes | two passes over the manifest produced byte-identical records |

## Exclusions

None.

## Fixture scores

Per-asset scores for the committed subset, pinned by `tests/calibration.rs` at three decimals.

| Path | Plan | Status | PSNR dB | SSIM | LSD dB | Note |
|---|---|---|---|---|---|---|
| png-small.png | image-metadata | identical |  |  |  | decoded stream identical |
| png-small.png | PX01 | held |  |  |  | PX01 on PNG input: the ruling asks for a lossy WebP write and no pure Rust lossy WebP encoder exists |
| png-small.png | PX02 | scored | 120.000 | 1.000 |  |  |
| png-small.png | PX03 | scored | 120.000 | 1.000 |  |  |
| png-small.png | PX04 | scored | 40.498 | 0.972 |  |  |
| png-small.png | PX05 | scored | 32.549 | 0.846 |  |  |
| png-small.png | PX06 | scored | 120.000 | 1.000 |  |  |
| png-small.png | image-safe | held |  |  |  | PX01 on PNG input: the ruling asks for a lossy WebP write and no pure Rust lossy WebP encoder exists |
| png-small.png | image-aggressive+PX03 | held |  |  |  | PX01 on PNG input: the ruling asks for a lossy WebP write and no pure Rust lossy WebP encoder exists |
| png-small.png | image-aggressive+PX04 | held |  |  |  | PX01 on PNG input: the ruling asks for a lossy WebP write and no pure Rust lossy WebP encoder exists |
| png-small.png | image-aggressive+PX05 | held |  |  |  | PX01 on PNG input: the ruling asks for a lossy WebP write and no pure Rust lossy WebP encoder exists |
| png-small.png | image-aggressive+PX06 | held |  |  |  | PX01 on PNG input: the ruling asks for a lossy WebP write and no pure Rust lossy WebP encoder exists |
| png-small.png | image-aggressive | held |  |  |  | PX01 on PNG input: the ruling asks for a lossy WebP write and no pure Rust lossy WebP encoder exists |
| png-small.png | image-two-opt-in | held |  |  |  | PX01 on PNG input: the ruling asks for a lossy WebP write and no pure Rust lossy WebP encoder exists |
| png-medium-alpha.png | image-metadata | identical |  |  |  | decoded stream identical |
| png-medium-alpha.png | PX01 | held |  |  |  | PX01 on PNG input: the ruling asks for a lossy WebP write and no pure Rust lossy WebP encoder exists |
| png-medium-alpha.png | PX02 | scored | 120.000 | 1.000 |  |  |
| png-medium-alpha.png | PX03 | scored | 120.000 | 1.000 |  |  |
| png-medium-alpha.png | PX04 | scored | 41.752 | 0.980 |  |  |
| png-medium-alpha.png | PX05 | scored | 33.848 | 0.890 |  |  |
| png-medium-alpha.png | PX06 | scored | 120.000 | 1.000 |  |  |
| png-medium-alpha.png | image-safe | held |  |  |  | PX01 on PNG input: the ruling asks for a lossy WebP write and no pure Rust lossy WebP encoder exists |
| png-medium-alpha.png | image-aggressive+PX03 | held |  |  |  | PX01 on PNG input: the ruling asks for a lossy WebP write and no pure Rust lossy WebP encoder exists |
| png-medium-alpha.png | image-aggressive+PX04 | held |  |  |  | PX01 on PNG input: the ruling asks for a lossy WebP write and no pure Rust lossy WebP encoder exists |
| png-medium-alpha.png | image-aggressive+PX05 | held |  |  |  | PX01 on PNG input: the ruling asks for a lossy WebP write and no pure Rust lossy WebP encoder exists |
| png-medium-alpha.png | image-aggressive+PX06 | held |  |  |  | PX01 on PNG input: the ruling asks for a lossy WebP write and no pure Rust lossy WebP encoder exists |
| png-medium-alpha.png | image-aggressive | held |  |  |  | PX01 on PNG input: the ruling asks for a lossy WebP write and no pure Rust lossy WebP encoder exists |
| png-medium-alpha.png | image-two-opt-in | held |  |  |  | PX01 on PNG input: the ruling asks for a lossy WebP write and no pure Rust lossy WebP encoder exists |
| png-large.png | image-metadata | identical |  |  |  | decoded stream identical |
| png-large.png | PX01 | held |  |  |  | PX01 on PNG input: the ruling asks for a lossy WebP write and no pure Rust lossy WebP encoder exists |
| png-large.png | PX02 | scored | 120.000 | 1.000 |  |  |
| png-large.png | PX03 | scored | 120.000 | 1.000 |  |  |
| png-large.png | PX04 | scored | 40.518 | 0.984 |  |  |
| png-large.png | PX05 | scored | 32.556 | 0.905 |  |  |
| png-large.png | PX06 | scored | 120.000 | 1.000 |  |  |
| png-large.png | image-safe | held |  |  |  | PX01 on PNG input: the ruling asks for a lossy WebP write and no pure Rust lossy WebP encoder exists |
| png-large.png | image-aggressive+PX03 | held |  |  |  | PX01 on PNG input: the ruling asks for a lossy WebP write and no pure Rust lossy WebP encoder exists |
| png-large.png | image-aggressive+PX04 | held |  |  |  | PX01 on PNG input: the ruling asks for a lossy WebP write and no pure Rust lossy WebP encoder exists |
| png-large.png | image-aggressive+PX05 | held |  |  |  | PX01 on PNG input: the ruling asks for a lossy WebP write and no pure Rust lossy WebP encoder exists |
| png-large.png | image-aggressive+PX06 | held |  |  |  | PX01 on PNG input: the ruling asks for a lossy WebP write and no pure Rust lossy WebP encoder exists |
| png-large.png | image-aggressive | held |  |  |  | PX01 on PNG input: the ruling asks for a lossy WebP write and no pure Rust lossy WebP encoder exists |
| png-large.png | image-two-opt-in | held |  |  |  | PX01 on PNG input: the ruling asks for a lossy WebP write and no pure Rust lossy WebP encoder exists |
| jpeg-small.jpg | image-metadata | identical |  |  |  | decoded stream identical |
| jpeg-small.jpg | PX01 | scored | 36.218 | 0.971 |  |  |
| jpeg-small.jpg | PX02 | scored | 34.702 | 0.952 |  |  |
| jpeg-small.jpg | PX03 | scored | 33.997 | 0.938 |  |  |
| jpeg-small.jpg | PX04 | scored | 35.009 | 0.952 |  |  |
| jpeg-small.jpg | PX05 | scored | 31.049 | 0.831 |  |  |
| jpeg-small.jpg | PX06 | scored | 36.217 | 0.971 |  |  |
| jpeg-small.jpg | image-safe | scored | 34.702 | 0.952 |  |  |
| jpeg-small.jpg | image-aggressive+PX03 | scored | 34.765 | 0.951 |  |  |
| jpeg-small.jpg | image-aggressive+PX04 | scored | 33.970 | 0.933 |  |  |
| jpeg-small.jpg | image-aggressive+PX05 | scored | 30.431 | 0.808 |  |  |
| jpeg-small.jpg | image-aggressive+PX06 | scored | 34.702 | 0.952 |  |  |
| jpeg-small.jpg | image-aggressive | scored | 30.431 | 0.808 |  |  |
| jpeg-small.jpg | image-two-opt-in | scored | 29.913 | 0.784 |  |  |
| jpeg-medium.jpg | image-metadata | identical |  |  |  | decoded stream identical |
| jpeg-medium.jpg | PX01 | scored | 36.249 | 0.979 |  |  |
| jpeg-medium.jpg | PX02 | scored | 34.783 | 0.970 |  |  |
| jpeg-medium.jpg | PX03 | scored | 33.936 | 0.958 |  |  |
| jpeg-medium.jpg | PX04 | scored | 35.021 | 0.966 |  |  |
| jpeg-medium.jpg | PX05 | scored | 31.054 | 0.876 |  |  |
| jpeg-medium.jpg | PX06 | scored | 36.249 | 0.979 |  |  |
| jpeg-medium.jpg | image-safe | scored | 34.783 | 0.970 |  |  |
| jpeg-medium.jpg | image-aggressive+PX03 | scored | 34.788 | 0.971 |  |  |
| jpeg-medium.jpg | image-aggressive+PX04 | scored | 34.077 | 0.959 |  |  |
| jpeg-medium.jpg | image-aggressive+PX05 | scored | 30.555 | 0.872 |  |  |
| jpeg-medium.jpg | image-aggressive+PX06 | scored | 34.839 | 0.970 |  |  |
| jpeg-medium.jpg | image-aggressive | scored | 30.555 | 0.872 |  |  |
| jpeg-medium.jpg | image-two-opt-in | scored | 30.030 | 0.854 |  |  |
| jpeg-large.jpg | image-metadata | identical |  |  |  | decoded stream identical |
| jpeg-large.jpg | PX01 | scored | 36.260 | 0.982 |  |  |
| jpeg-large.jpg | PX02 | scored | 34.666 | 0.976 |  |  |
| jpeg-large.jpg | PX03 | scored | 33.820 | 0.963 |  |  |
| jpeg-large.jpg | PX04 | scored | 35.057 | 0.971 |  |  |
| jpeg-large.jpg | PX05 | scored | 31.078 | 0.894 |  |  |
| jpeg-large.jpg | PX06 | scored | 36.260 | 0.982 |  |  |
| jpeg-large.jpg | image-safe | scored | 34.666 | 0.976 |  |  |
| jpeg-large.jpg | image-aggressive+PX03 | scored | 34.730 | 0.975 |  |  |
| jpeg-large.jpg | image-aggressive+PX04 | scored | 33.972 | 0.965 |  |  |
| jpeg-large.jpg | image-aggressive+PX05 | scored | 30.572 | 0.893 |  |  |
| jpeg-large.jpg | image-aggressive+PX06 | scored | 34.679 | 0.975 |  |  |
| jpeg-large.jpg | image-aggressive | scored | 30.572 | 0.893 |  |  |
| jpeg-large.jpg | image-two-opt-in | scored | 30.051 | 0.878 |  |  |
| webp-small-alpha.webp | image-metadata | identical |  |  |  | decoded stream identical |
| webp-small-alpha.webp | PX01 | held |  |  |  | PX01 on WebP input: no pure Rust lossy WebP encoder exists |
| webp-small-alpha.webp | PX02 | scored | 120.000 | 1.000 |  |  |
| webp-small-alpha.webp | PX03 | scored | 120.000 | 1.000 |  |  |
| webp-small-alpha.webp | PX04 | scored | 41.764 | 0.972 |  |  |
| webp-small-alpha.webp | PX05 | scored | 33.834 | 0.848 |  |  |
| webp-small-alpha.webp | PX06 | scored | 120.000 | 1.000 |  |  |
| webp-small-alpha.webp | image-safe | held |  |  |  | PX01 on WebP input: no pure Rust lossy WebP encoder exists |
| webp-small-alpha.webp | image-aggressive+PX03 | held |  |  |  | PX01 on WebP input: no pure Rust lossy WebP encoder exists |
| webp-small-alpha.webp | image-aggressive+PX04 | held |  |  |  | PX01 on WebP input: no pure Rust lossy WebP encoder exists |
| webp-small-alpha.webp | image-aggressive+PX05 | held |  |  |  | PX01 on WebP input: no pure Rust lossy WebP encoder exists |
| webp-small-alpha.webp | image-aggressive+PX06 | held |  |  |  | PX01 on WebP input: no pure Rust lossy WebP encoder exists |
| webp-small-alpha.webp | image-aggressive | held |  |  |  | PX01 on WebP input: no pure Rust lossy WebP encoder exists |
| webp-small-alpha.webp | image-two-opt-in | held |  |  |  | PX01 on WebP input: no pure Rust lossy WebP encoder exists |
| webp-medium.webp | image-metadata | identical |  |  |  | decoded stream identical |
| webp-medium.webp | PX01 | held |  |  |  | PX01 on WebP input: no pure Rust lossy WebP encoder exists |
| webp-medium.webp | PX02 | scored | 120.000 | 1.000 |  |  |
| webp-medium.webp | PX03 | scored | 120.000 | 1.000 |  |  |
| webp-medium.webp | PX04 | scored | 40.539 | 0.980 |  |  |
| webp-medium.webp | PX05 | scored | 32.582 | 0.889 |  |  |
| webp-medium.webp | PX06 | scored | 120.000 | 1.000 |  |  |
| webp-medium.webp | image-safe | held |  |  |  | PX01 on WebP input: no pure Rust lossy WebP encoder exists |
| webp-medium.webp | image-aggressive+PX03 | held |  |  |  | PX01 on WebP input: no pure Rust lossy WebP encoder exists |
| webp-medium.webp | image-aggressive+PX04 | held |  |  |  | PX01 on WebP input: no pure Rust lossy WebP encoder exists |
| webp-medium.webp | image-aggressive+PX05 | held |  |  |  | PX01 on WebP input: no pure Rust lossy WebP encoder exists |
| webp-medium.webp | image-aggressive+PX06 | held |  |  |  | PX01 on WebP input: no pure Rust lossy WebP encoder exists |
| webp-medium.webp | image-aggressive | held |  |  |  | PX01 on WebP input: no pure Rust lossy WebP encoder exists |
| webp-medium.webp | image-two-opt-in | held |  |  |  | PX01 on WebP input: no pure Rust lossy WebP encoder exists |
| webp-large.webp | image-metadata | identical |  |  |  | decoded stream identical |
| webp-large.webp | PX01 | held |  |  |  | PX01 on WebP input: no pure Rust lossy WebP encoder exists |
| webp-large.webp | PX02 | scored | 120.000 | 1.000 |  |  |
| webp-large.webp | PX03 | scored | 120.000 | 1.000 |  |  |
| webp-large.webp | PX04 | scored | 40.529 | 0.983 |  |  |
| webp-large.webp | PX05 | scored | 32.550 | 0.904 |  |  |
| webp-large.webp | PX06 | scored | 120.000 | 1.000 |  |  |
| webp-large.webp | image-safe | held |  |  |  | PX01 on WebP input: no pure Rust lossy WebP encoder exists |
| webp-large.webp | image-aggressive+PX03 | held |  |  |  | PX01 on WebP input: no pure Rust lossy WebP encoder exists |
| webp-large.webp | image-aggressive+PX04 | held |  |  |  | PX01 on WebP input: no pure Rust lossy WebP encoder exists |
| webp-large.webp | image-aggressive+PX05 | held |  |  |  | PX01 on WebP input: no pure Rust lossy WebP encoder exists |
| webp-large.webp | image-aggressive+PX06 | held |  |  |  | PX01 on WebP input: no pure Rust lossy WebP encoder exists |
| webp-large.webp | image-aggressive | held |  |  |  | PX01 on WebP input: no pure Rust lossy WebP encoder exists |
| webp-large.webp | image-two-opt-in | held |  |  |  | PX01 on WebP input: no pure Rust lossy WebP encoder exists |
| wav-short-stereo.wav | audio-metadata | identical |  |  |  | decoded stream identical |
| wav-short-stereo.wav | AU01 | scored |  |  | 1.130 |  |
| wav-short-stereo.wav | AU02 | scored |  |  | 0.045 |  |
| wav-short-stereo.wav | AU03 | held |  |  |  | AU03 is reserved and not runnable in this version |
| wav-short-stereo.wav | AU04 | scored |  |  | 1.243 |  |
| wav-short-stereo.wav | AU05 | scored |  |  | 5.252 |  |
| wav-short-stereo.wav | audio-safe | scored |  |  | 1.130 |  |
| wav-short-stereo.wav | audio-aggressive+AU04 | scored |  |  | 1.690 |  |
| wav-short-stereo.wav | audio-aggressive+AU05 | scored |  |  | 5.253 |  |
| wav-short-stereo.wav | audio-aggressive | scored |  |  | 5.253 |  |
| wav-short-stereo.wav | audio-two-opt-in | scored |  |  | 5.483 |  |
| wav-medium-24bit.wav | audio-metadata | identical |  |  |  | decoded stream identical |
| wav-medium-24bit.wav | AU01 | scored |  |  | 1.557 |  |
| wav-medium-24bit.wav | AU02 | scored |  |  | 0.056 |  |
| wav-medium-24bit.wav | AU03 | held |  |  |  | AU03 is reserved and not runnable in this version |
| wav-medium-24bit.wav | AU04 | scored |  |  | 1.226 |  |
| wav-medium-24bit.wav | AU05 | scored |  |  | 9.044 |  |
| wav-medium-24bit.wav | audio-safe | scored |  |  | 1.558 |  |
| wav-medium-24bit.wav | audio-aggressive+AU04 | scored |  |  | 2.006 |  |
| wav-medium-24bit.wav | audio-aggressive+AU05 | scored |  |  | 9.047 |  |
| wav-medium-24bit.wav | audio-aggressive | scored |  |  | 9.047 |  |
| wav-medium-24bit.wav | audio-two-opt-in | scored |  |  | 9.106 |  |
| wav-long.wav | audio-metadata | identical |  |  |  | decoded stream identical |
| wav-long.wav | AU01 | scored |  |  | 1.604 |  |
| wav-long.wav | AU02 | scored |  |  | 0.053 |  |
| wav-long.wav | AU03 | held |  |  |  | AU03 is reserved and not runnable in this version |
| wav-long.wav | AU04 | scored |  |  | 1.186 |  |
| wav-long.wav | AU05 | scored |  |  | 4.239 |  |
| wav-long.wav | audio-safe | scored |  |  | 1.605 |  |
| wav-long.wav | audio-aggressive+AU04 | scored |  |  | 2.033 |  |
| wav-long.wav | audio-aggressive+AU05 | scored |  |  | 4.670 |  |
| wav-long.wav | audio-aggressive | scored |  |  | 4.670 |  |
| wav-long.wav | audio-two-opt-in | scored |  |  | 4.888 |  |
| flac-short-stereo.flac | audio-metadata | identical |  |  |  | decoded stream identical |
| flac-short-stereo.flac | AU01 | scored |  |  | 1.913 |  |
| flac-short-stereo.flac | AU02 | scored |  |  | 0.044 |  |
| flac-short-stereo.flac | AU03 | held |  |  |  | AU03 is reserved and not runnable in this version |
| flac-short-stereo.flac | AU04 | scored |  |  | 1.245 |  |
| flac-short-stereo.flac | AU05 | scored |  |  | 4.982 |  |
| flac-short-stereo.flac | audio-safe | scored |  |  | 1.913 |  |
| flac-short-stereo.flac | audio-aggressive+AU04 | scored |  |  | 2.260 |  |
| flac-short-stereo.flac | audio-aggressive+AU05 | scored |  |  | 5.158 |  |
| flac-short-stereo.flac | audio-aggressive | scored |  |  | 5.158 |  |
| flac-short-stereo.flac | audio-two-opt-in | scored |  |  | 5.370 |  |
| flac-medium-24bit.flac | audio-metadata | identical |  |  |  | decoded stream identical |
| flac-medium-24bit.flac | AU01 | scored |  |  | 1.590 |  |
| flac-medium-24bit.flac | AU02 | scored |  |  | 0.056 |  |
| flac-medium-24bit.flac | AU03 | held |  |  |  | AU03 is reserved and not runnable in this version |
| flac-medium-24bit.flac | AU04 | scored |  |  | 1.224 |  |
| flac-medium-24bit.flac | AU05 | scored |  |  | 10.214 |  |
| flac-medium-24bit.flac | audio-safe | scored |  |  | 1.591 |  |
| flac-medium-24bit.flac | audio-aggressive+AU04 | scored |  |  | 2.038 |  |
| flac-medium-24bit.flac | audio-aggressive+AU05 | scored |  |  | 10.216 |  |
| flac-medium-24bit.flac | audio-aggressive | scored |  |  | 10.216 |  |
| flac-medium-24bit.flac | audio-two-opt-in | scored |  |  | 10.279 |  |
| flac-long.flac | audio-metadata | identical |  |  |  | decoded stream identical |
| flac-long.flac | AU01 | scored |  |  | 1.552 |  |
| flac-long.flac | AU02 | scored |  |  | 0.055 |  |
| flac-long.flac | AU03 | held |  |  |  | AU03 is reserved and not runnable in this version |
| flac-long.flac | AU04 | scored |  |  | 1.227 |  |
| flac-long.flac | AU05 | scored |  |  | 3.628 |  |
| flac-long.flac | audio-safe | scored |  |  | 1.553 |  |
| flac-long.flac | audio-aggressive+AU04 | scored |  |  | 2.006 |  |
| flac-long.flac | audio-aggressive+AU05 | scored |  |  | 3.637 |  |
| flac-long.flac | audio-aggressive | scored |  |  | 3.637 |  |
| flac-long.flac | audio-two-opt-in | scored |  |  | 3.715 |  |
