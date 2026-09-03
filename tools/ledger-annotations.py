#!/usr/bin/env python3
"""Export harness annotations from the corpus ledger, read-only.

Writes two CSVs the calibration harness merges by doc_id:

  ladder.csv  doc_id, ladder_step        for round-4 rows, from spec_json
  marks.csv   doc_id, documented_marks   for the keyed-generator rows

The ledger is opened immutable and nothing under the corpus is written.
No document text leaves the ledger: only structural enum-like values.

    python3 tools/ledger-annotations.py CORPUS_ROOT KEYED.csv OUT_DIR
"""
import csv
import json
import os
import sqlite3
import sys


def ladder_step(spec):
    stratum = spec.get("stratum") or "unlabeled"
    def val(k):
        v = spec.get(k)
        return None if v in (None, "", "none") else v
    detail = None
    if stratum == "ocr-jpeg-quality":
        detail = f"q{val('jpeg_quality')}"
    elif stratum == "ocr-downscale":
        detail = f"x{val('downscale_factor')}"
    elif stratum == "ocr-noise":
        detail = f"{val('image_noise_type')}:{val('image_noise_value')}"
    elif stratum == "ocr-rotation":
        detail = f"{val('rotation_deg')}deg"
    elif stratum == "ocr-skew":
        detail = f"{val('skew_deg')}deg"
    elif stratum == "ocr-low-contrast":
        detail = f"ratio{val('contrast_ratio')}"
    elif stratum == "ocr-perspective":
        detail = f"{val('perspective_direction')}:{val('perspective_pct')}pct"
    elif stratum == "ocr-position":
        detail = str(val("position_anchor"))
    elif stratum == "ocr-small-font":
        detail = f"{val('font_px')}px"
    elif stratum.startswith("audio-noise"):
        detail = f"{val('noise_type')}:{val('snr_db_target')}db"
    elif stratum == "audio-say-voice-rate":
        detail = f"{val('rate_wpm')}wpm"
    elif stratum == "audio-acoustic-supplement":
        detail = f"{val('acoustic_kind')}:{val('acoustic_value')}"
    return f"{stratum}/{detail}" if detail else stratum


def main(root, keyed_csv, out_dir):
    os.makedirs(out_dir, exist_ok=True)
    uri = f"file:{os.path.join(root, 'ledger.sqlite')}?mode=ro&immutable=1"
    con = sqlite3.connect(uri, uri=True)
    rows = con.execute(
        "select doc_id, spec_json from documents where round = 4"
    ).fetchall()
    with open(os.path.join(out_dir, "ladder.csv"), "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["doc_id", "ladder_step"])
        for doc_id, spec_json in rows:
            try:
                spec = json.loads(spec_json or "{}")
            except json.JSONDecodeError:
                spec = {}
            w.writerow([doc_id, ladder_step(spec if isinstance(spec, dict) else {})])
    with open(keyed_csv, newline="") as f:
        keyed = list(csv.DictReader(f))
    with open(os.path.join(out_dir, "marks.csv"), "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["doc_id", "documented_marks"])
        for r in keyed:
            marks = ["synthid:vendor-stated"]
            if "overlay" in (r.get("generator_model") or ""):
                marks.append("transformed")
            w.writerow([r["doc_id"], ";".join(marks)])
    print(f"wrote {len(rows)} ladder rows and {len(keyed)} marks rows to {out_dir}")


if __name__ == "__main__":
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    main(*sys.argv[1:])
