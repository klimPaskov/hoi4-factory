"""Key the magenta background out of the generated Discord poses.

Usage: key_cutouts.py SRC_DIR OUT_DIR
Writes trimmed RGBA PNG cutouts, one per magenta-background source file.
"""
import sys
from pathlib import Path

import numpy as np
from PIL import Image


def trim(img):
    box = img.getchannel("A").point(lambda v: 255 if v > 24 else 0).getbbox()
    return img.crop(box)


def key(path):
    raw = Image.open(path)
    if raw.mode == "RGBA" and raw.getchannel("A").getextrema()[0] == 0:
        return trim(raw)  # the model already returned real transparency
    im = raw.convert("RGB")
    a = np.asarray(im).astype(np.float32)
    r, g, b = a[..., 0], a[..., 1], a[..., 2]
    m = np.minimum(r, b) - g  # magenta strength, about 255 on the background
    alpha = 1.0 - np.clip((m - 50.0) / 110.0, 0.0, 1.0)
    # despill: pull red and blue back toward green where magenta bleeds into edges
    spill = np.clip(np.minimum(r, b) - g, 0, None) * (alpha < 0.999)
    r2 = r - spill * 0.85
    b2 = b - spill * 0.85
    out = np.dstack([r2, g, b2, alpha * 255.0]).clip(0, 255).astype(np.uint8)
    return trim(Image.fromarray(out, "RGBA"))


if __name__ == "__main__":
    src, dst = Path(sys.argv[1]), Path(sys.argv[2])
    dst.mkdir(parents=True, exist_ok=True)
    for f in sorted(src.glob("*.png")):
        key(f).save(dst / f.name)
        print(f.name)
