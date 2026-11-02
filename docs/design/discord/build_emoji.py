"""Build the 128x128 Discord emoji from the keyed cutouts.

Usage: build_emoji.py CUT_DIR OUT_DIR
Each emoji is a tight crop of the face and upper body with a white sticker
outline so it reads at Discord's 22 to 32 px sizes.
"""
import sys
from pathlib import Path

from PIL import Image, ImageFilter

# name -> (source cutout, crop box as fractions x0, y0, x1, y1 of the cutout)
HEAD = (0.30, 0.04, 1.00, 0.70)
EMOJI = {
    "wave": ("wave", (0.26, 0.0, 1.0, 0.64)),
    "cheer": ("cheer", (0.10, 0.0, 1.0, 0.66)),
    "thumbsup": ("thumbs-up", (0.28, 0.0, 1.0, 0.66)),
    "think": ("think", (0.26, 0.0, 1.0, 0.68)),
    "surprised": ("surprised", (0.24, 0.0, 1.0, 0.68)),
    "sorry": ("sorry", (0.24, 0.0, 1.0, 0.68)),
    "eyes": ("eyes", (0.24, 0.0, 1.0, 0.66)),
    "laugh": ("laugh", (0.26, 0.0, 1.0, 0.64)),
    "love": ("love", (0.26, 0.0, 1.0, 0.66)),
    "salute": ("salute", (0.26, 0.0, 1.0, 0.66)),
    "facepalm": ("facepalm", (0.26, 0.0, 1.0, 0.66)),
    "cry": ("cry", (0.24, 0.0, 1.0, 0.66)),
    "angry": ("angry", (0.24, 0.0, 1.0, 0.66)),
    "sleepy": ("sleepy", (0.24, 0.0, 1.0, 0.68)),
    "party": ("party", (0.16, 0.0, 1.0, 0.66)),
    "gg": ("gg", (0.14, 0.0, 1.0, 0.76)),
    "please": ("please", (0.24, 0.0, 1.0, 0.68)),
    "ok": ("ok", (0.24, 0.0, 1.0, 0.66)),
    "fire": ("fire", (0.10, 0.0, 1.0, 0.70)),
    "popcorn": ("popcorn", (0.14, 0.0, 1.0, 0.76)),
    "hammer": ("hammer", (0.22, 0.0, 1.0, 0.66)),
    "medal": ("medal", (0.26, 0.0, 1.0, 0.70)),
    "map": ("map", (0.10, 0.0, 1.0, 0.76)),
    "focus": ("focus", (0.12, 0.0, 1.0, 0.76)),
}

SIZE = 128
STROKE = 4


def outline(img, px, color=(255, 255, 255)):
    """Add a solid outline around the alpha shape."""
    pad = px + 2
    big = Image.new("RGBA", (img.width + 2 * pad, img.height + 2 * pad), (0, 0, 0, 0))
    big.paste(img, (pad, pad))
    a = big.getchannel("A").point(lambda v: 255 if v > 40 else 0)
    grown = a.filter(ImageFilter.MaxFilter(2 * px + 1))
    base = Image.new("RGBA", big.size, color + (0,))
    base.putalpha(grown)
    base.alpha_composite(big)
    return base


def make(src, box, size=SIZE, stroke=STROKE):
    im = Image.open(src)
    w, h = im.size
    crop = im.crop((int(box[0] * w), int(box[1] * h), int(box[2] * w), int(box[3] * h)))
    inner = size - 2 * stroke - 2
    crop.thumbnail((inner * 4, inner * 4), Image.LANCZOS)
    # fit into inner square, keep aspect
    scale = inner / max(crop.size)
    crop = crop.resize((max(1, round(crop.width * scale)), max(1, round(crop.height * scale))), Image.LANCZOS)
    framed = outline(crop, stroke)
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    framed.thumbnail((size, size), Image.LANCZOS)
    canvas.paste(framed, ((size - framed.width) // 2, (size - framed.height) // 2), framed)
    return canvas


if __name__ == "__main__":
    cut, out = Path(sys.argv[1]), Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    for name, (src, box) in EMOJI.items():
        img = make(cut / f"{src}.png", box)
        img.save(out / f"factory_{name}.png", optimize=True)
        print(f"factory_{name}.png")
