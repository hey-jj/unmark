#!/usr/bin/env python3
"""Render the builder's efficacy fixtures and watermark them through the
oracle. Every base image is procedural (gradients, texture fields, flat
shapes, text-like bars), so nothing here is third-party media.

    render_fixtures.py OUT_DIR

Writes OUT_DIR/<name>-base.png, <name>-sdxl.png (48-bit Diffusers payload),
<name>-compvis.png (136-bit CompVis payload), and for two bases <name>-sdxl.jpg
(the marked image written as a quality-95 JPEG with 4:4:4 chroma, which keeps
the U channel the mark lives in). The 4:2:0 encode of the photo-like base goes
to OUT_DIR/../subsampled-sdxl-420.jpg as the detect-unreliable evidence
fixture. Every base carries a smooth chroma field (blurred noise) because the
mark rides on the largest value in each block of the U channel and a nearly
flat chroma block reads back as noise. Requires the oracle venv as python.
"""
import os
import subprocess
import sys

import cv2
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
ORACLE = os.path.join(HERE, "oracle.py")


def blobs(rng, h, w, sigma, amp):
    """A smooth chroma field: blurred unit noise scaled to amp."""
    n = rng.normal(0, 1, (h, w))
    n = cv2.GaussianBlur(n, (0, 0), sigma)
    n /= (n.std() + 1e-9)
    return n * amp


def gradient_field(rng, h, w, sigma, amp, noise):
    y, x = np.mgrid[0:h, 0:w].astype(np.float64)
    img = np.zeros((h, w, 3))
    img[..., 0] = 110 + 60 * np.sin(x / 90) + 30 * np.cos(y / 70) + blobs(rng, h, w, sigma, amp)
    img[..., 1] = 100 + 70 * (y / h) + 20 * np.sin((x + y) / 40) + blobs(rng, h, w, sigma, amp * 0.5)
    img[..., 2] = 140 - 50 * (x / w) + 25 * np.cos(x / 25) * np.sin(y / 33) + blobs(rng, h, w, sigma, amp)
    img += rng.normal(0, noise, img.shape)
    return img


def bases():
    rng = np.random.default_rng(20260903)
    out = {}
    # A photo-like field: smooth gradients with a soft chroma field.
    out["photo-like"] = gradient_field(rng, 384, 512, 2.0, 22, 1.5)
    # A flat illustration: large flat regions with hard edges.
    h, w = 480, 480
    img = np.full((h, w, 3), 200.0)
    cv2.circle(img, (165, 195), 112, (60, 120, 200), -1)
    cv2.rectangle(img, (248, 90), (450, 315), (210, 90, 70), -1)
    cv2.ellipse(img, (315, 375), (135, 52), 20, 0, 360, (90, 170, 110), -1)
    out["flat-illustration"] = img
    # A dense texture: high-frequency structured noise over a chroma field.
    # Every base keeps its short edge at or above 336 pixels so the default
    # run's 32-pixel crop and 0.95 resize leave an output the oracle still
    # reads (it refuses an edge under 256).
    h, w = 340, 448
    y, x = np.mgrid[0:h, 0:w].astype(np.float64)
    img = np.zeros((h, w, 3))
    base = 128 + 50 * np.sin(x * 0.7) * np.cos(y * 0.5) + rng.normal(0, 8, (h, w))
    img[..., 0] = base + blobs(rng, h, w, 2.0, 45)
    img[..., 1] = base * 0.8 + 20 * np.sin(y * 0.3)
    img[..., 2] = base * 0.6 + 40 * np.cos(x * 0.2) + blobs(rng, h, w, 2.0, 45)
    out["dense-texture"] = img
    # Text-like bars on a mid-tone page. A near-white page saturates the
    # U-channel edit away, so the page is a light gray that holds the mark.
    h, w = 540, 420
    img = np.full((h, w, 3), 205.0)
    for row in range(40, 500, 20):
        width = int(rng.integers(150, 360))
        cv2.rectangle(img, (30, row), (30 + width, row + 8), (40, 40, 40), -1)
    out["text-ui"] = img
    # The smallest image whose default-run output the oracle still reads,
    # with a stronger chroma field because the 136-bit payload repeats fewer
    # times here.
    out["small"] = gradient_field(rng, 336, 352, 2.0, 50, 1.5)
    return {k: np.clip(v, 0, 255).astype(np.uint8) for k, v in out.items()}


def main(out_dir):
    os.makedirs(out_dir, exist_ok=True)
    py = sys.executable
    for name, img in bases().items():
        base = os.path.join(out_dir, f"{name}-base.png")
        cv2.imwrite(base, img)
        sdxl = os.path.join(out_dir, f"{name}-sdxl.png")
        compvis = os.path.join(out_dir, f"{name}-compvis.png")
        subprocess.run([py, ORACLE, "encode", base, sdxl, "hex:B3EC907BB19E:48"], check=True)
        subprocess.run([py, ORACLE, "encode", base, compvis, "text:StableDiffusionV1"], check=True)
        if name in ("photo-like", "small"):
            marked = cv2.imread(sdxl, cv2.IMREAD_COLOR)
            cv2.imwrite(
                os.path.join(out_dir, f"{name}-sdxl.jpg"),
                marked,
                [cv2.IMWRITE_JPEG_QUALITY, 95, cv2.IMWRITE_JPEG_SAMPLING_FACTOR, 0x111111],
            )
        if name == "photo-like":
            marked = cv2.imread(sdxl, cv2.IMREAD_COLOR)
            cv2.imwrite(
                os.path.join(out_dir, os.pardir, "subsampled-sdxl-420.jpg"),
                marked,
                [cv2.IMWRITE_JPEG_QUALITY, 95],
            )
        for f, n in ((sdxl, 48), (compvis, 136)):
            bits = subprocess.run([py, ORACLE, "decode", f, str(n)], check=True, capture_output=True, text=True).stdout.strip()
            print(name, os.path.basename(f), "oracle bits", bits)
    print("wrote", out_dir)


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "fixtures/efficacy")
