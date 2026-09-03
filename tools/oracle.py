#!/usr/bin/env python3
"""Subprocess oracle over the invisible-watermark package (dwtDct).

unmark never links this package or carries its code. CI installs it so the
efficacy test can compare the in-crate decoder against it; a local run without
it skips that test. The package's rivaGan module imports torch at import time,
which the dwtDct path never uses, so torch and onnxruntime are stubbed when
absent.

    oracle.py encode IN.png OUT.png hex:<payload-hex>:<bits> | text:<utf8>
    oracle.py decode IN.png <bits>            prints the recovered bits as 0/1
    oracle.py version

Exit 0 on success, 3 when the package is not importable.
"""
import sys
import types


def _import():
    for name in ("torch", "onnxruntime"):
        try:
            __import__(name)
        except ImportError:
            sys.modules[name] = types.ModuleType(name)
    try:
        from imwatermark import WatermarkDecoder, WatermarkEncoder  # noqa: E402
    except ImportError as e:
        sys.stderr.write(f"oracle: invisible-watermark not importable: {e}\n")
        sys.exit(3)
    import cv2  # noqa: E402

    return WatermarkEncoder, WatermarkDecoder, cv2


def payload_bits(spec):
    kind, _, rest = spec.partition(":")
    if kind == "hex":
        hexval, _, nbits = rest.partition(":")
        n = int(nbits)
        value = int(hexval, 16)
        return [(value >> (n - 1 - i)) & 1 for i in range(n)]
    if kind == "text":
        bits = []
        for b in rest.encode("utf-8"):
            bits.extend((b >> (7 - i)) & 1 for i in range(8))
        return bits
    raise SystemExit(f"oracle: unknown payload spec {spec!r}")


def main(argv):
    if len(argv) < 2:
        raise SystemExit(__doc__)
    cmd = argv[1]
    if cmd == "version":
        _import()
        import importlib.metadata as md

        print(md.version("invisible-watermark"))
        return
    Enc, Dec, cv2 = _import()
    if cmd == "encode":
        src, dst, spec = argv[2], argv[3], argv[4]
        bits = payload_bits(spec)
        bgr = cv2.imread(src, cv2.IMREAD_COLOR)
        if bgr is None:
            raise SystemExit(f"oracle: cannot read {src}")
        enc = Enc()
        enc.set_watermark("bits", bits)
        out = enc.encode(bgr, "dwtDct")
        if not cv2.imwrite(dst, out):
            raise SystemExit(f"oracle: cannot write {dst}")
        return
    if cmd == "decode":
        src, nbits = argv[2], int(argv[3])
        bgr = cv2.imread(src, cv2.IMREAD_COLOR)
        if bgr is None:
            raise SystemExit(f"oracle: cannot read {src}")
        dec = Dec("bits", nbits)
        bits = dec.decode(bgr, "dwtDct")
        print("".join("1" if b else "0" for b in bits))
        return
    raise SystemExit(__doc__)


if __name__ == "__main__":
    main(sys.argv)
