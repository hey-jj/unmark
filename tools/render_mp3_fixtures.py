#!/usr/bin/env python3
"""Render the MPEG audio fixtures from a synthetic tone. The tone is a
one-second 440 Hz sine written as a WAV here; a local LAME binary encodes
it (fixture creation only, never a crate dependency). The script then adds
the tags the detector and rewriter are tested against: LAME's own ID3v1,
ID3v2, and Info frame, an APEv2 tag appended here, and a synthetic file
whose information frame precedes a frame that draws from the bit reservoir.

    render_mp3_fixtures.py OUT_DIR
"""
import math
import os
import struct
import subprocess
import sys
import wave


def tone(path, seconds=1.0, rate=44100):
    frames = int(seconds * rate)
    data = b"".join(
        struct.pack("<h", int(12000 * math.sin(2 * math.pi * 440 * i / rate) + 3000 * math.sin(2 * math.pi * 1760 * i / rate)))
        for i in range(frames)
    )
    with wave.open(path, "wb") as w:
        w.setparams((1, 2, rate, frames, "NONE", "not compressed"))
        w.writeframes(data)


def ape_tag(items):
    body = b""
    for key, value in items:
        v = value.encode()
        body += struct.pack("<II", len(v), 0) + key.encode() + b"\0" + v
    size = len(body) + 32
    def block(flags):
        return b"APETAGEX" + struct.pack("<IIII", 2000, size, len(items), flags) + b"\0" * 8
    return block(0xA0000000) + body + block(0x80000000)


def parse_header(b):
    if len(b) < 4 or b[0] != 0xFF or (b[1] & 0xE0) != 0xE0:
        return None
    version = (b[1] >> 3) & 3
    if version == 1 or (b[1] >> 1) & 3 != 1:
        return None
    crc = b[1] & 1 == 0
    bi = b[2] >> 4
    sr = (b[2] >> 2) & 3
    pad = (b[2] >> 1) & 1
    mono = (b[3] >> 6) == 3
    v1 = version == 3
    br = [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0][bi] if v1 else [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 0][bi]
    rates = {3: [44100, 48000, 32000], 2: [22050, 24000, 16000], 0: [11025, 12000, 8000]}[version]
    if br == 0 or sr == 3:
        return None
    length = (144000 if v1 else 72000) * br // rates[sr] + pad
    side = (17 if mono else 32) if v1 else (9 if mono else 17)
    return length, side, crc, v1


def frames_of(data):
    out = []
    p = 0
    while p + 4 <= len(data):
        h = parse_header(data[p:p + 4])
        if h is None or p + h[0] > len(data):
            break
        out.append((p, h))
        p += h[0]
    return out


def main_data_begin(frame, h):
    at = 4 + (2 if h[2] else 0)
    word = struct.unpack(">H", frame[at:at + 2])[0]
    return word >> 7 if h[3] else word >> 8


def main(out_dir):
    os.makedirs(out_dir, exist_ok=True)
    wav = os.path.join(out_dir, "tone.wav")
    tone(wav)
    base = ["lame", "--quiet", "-m", "m", "-b", "128"]
    tagged = os.path.join(out_dir, "tagged.mp3")
    subprocess.run(base + ["--add-id3v2", "--tt", "Fixture tone", "--ta", "unmark builder", "--tc", "generator fixture", wav, tagged], check=True)
    # The APE tag goes before the trailing ID3v1 tag, the order the APE
    # specification recommends.
    data = open(tagged, "rb").read()
    assert data[-128:-125] == b"TAG"
    with open(tagged, "wb") as f:
        f.write(data[:-128] + ape_tag([("Tool", "fixture renderer"), ("Comment", "ape fixture")]) + data[-128:])
    subprocess.run(["lame", "--quiet", "-m", "m", "-V", "6", wav, os.path.join(out_dir, "vbr.mp3")], check=True)
    plain = os.path.join(out_dir, "plain.mp3")
    subprocess.run(base + ["-t", wav, plain], check=True)
    # A file whose information frame precedes a frame that draws from the
    # reservoir: an Info frame built from the first header, then the stream
    # from the first frame whose main_data_begin is nonzero.
    data = open(plain, "rb").read()
    frames = frames_of(data)
    assert frames, "no frames parsed"
    pick = None
    for p, h in frames[2:]:
        if main_data_begin(data[p:p + h[0]], h) != 0:
            pick = p
            break
    assert pick is not None, "no frame draws from the reservoir"
    p0, h0 = frames[0]
    header = data[p0:p0 + 4]
    info = header + b"\0" * h0[1] + b"Info" + struct.pack(">I", 0)
    info = info + b"\0" * (h0[0] - len(info))
    with open(os.path.join(out_dir, "reservoir.mp3"), "wb") as f:
        f.write(info + data[pick:])
    os.remove(wav)
    for name in ["tagged.mp3", "vbr.mp3", "plain.mp3", "reservoir.mp3"]:
        print(name, os.path.getsize(os.path.join(out_dir, name)), "bytes")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "fixtures/mp3")
