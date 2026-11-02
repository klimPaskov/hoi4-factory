"""Build the animated emoji, stickers, server icon and banner.

Usage: build_assets.py CUT_DIR OUT_DIR
CUT_DIR holds the keyed cutouts (key_cutouts.py). OUT_DIR is docs/design/discord.
Text uses Arial Black from the Windows font folder.
"""
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

sys.path.insert(0, str(Path(__file__).parent))
from build_emoji import EMOJI, make, outline  # noqa: E402

FONT = "C:/Windows/Fonts/ariblk.ttf"


def font(px):
    return ImageFont.truetype(FONT, px)


# ---------- animated emoji ----------

def emoji_frame(cut, src, box):
    return make(cut / f"{src}.png", box)


def transform(img, dy=0, sx=1.0, sy=1.0, rot=0.0, size=128):
    w, h = img.size
    im = img.resize((max(1, round(w * sx)), max(1, round(h * sy))), Image.LANCZOS)
    if rot:
        im = im.rotate(rot, resample=Image.BICUBIC, expand=True)
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    # anchor the bottom so squash and stretch keep contact with the ground
    x = (size - im.width) // 2
    y = size - im.height - 2 + dy
    canvas.paste(im, (x, y), im)
    return canvas


def save_gif(frames, path, duration, colors=255):
    pal = []
    for f in frames:
        a = f.getchannel("A")
        base = Image.new("RGB", f.size, (255, 255, 255))
        base.paste(f.convert("RGB"), mask=a)
        q = base.quantize(colors=colors, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE)
        mask = a.point(lambda v: 255 if v < 128 else 0)
        q.paste(255, mask=mask)  # index 255 is the transparent colour
        pal.append(q)
    pal[0].save(path, save_all=True, append_images=pal[1:], duration=duration, loop=0,
                transparency=255, disposal=2, optimize=False)


def animated(cut, out):
    box = EMOJI["wave"][1]
    wave = emoji_frame(cut, "wave", box)
    rest = emoji_frame(cut, "master", box)
    hop = [0, -8, -12, -8, 0, 0]
    seq = []
    for i, dy in enumerate(hop):
        base = wave if i % 2 == 0 else rest
        seq.append(transform(base, dy=dy))
    seq += [transform(wave, dy=0), transform(rest, dy=0)]
    save_gif(seq, out / "factory_wave_hop.gif", 110)

    cheer = emoji_frame(cut, "cheer", (0.10, 0.0, 1.0, 0.66))
    steps = [(0, 1.0, 1.0), (-4, 1.0, 1.04), (-12, 0.97, 1.08), (-18, 0.97, 1.08),
             (-12, 0.98, 1.05), (-2, 1.0, 1.0), (0, 1.06, 0.92), (0, 1.0, 1.0)]
    save_gif([transform(cheer, dy=d, sx=sx, sy=sy) for d, sx, sy in steps],
             out / "factory_cheer_bounce.gif", 90)

    laugh = emoji_frame(cut, "laugh", EMOJI["laugh"][1])
    angles = [0, 6, 0, -6, 0, 7, 0, -7]
    ys = [0, -3, 0, -3, 0, -3, 0, -3]
    save_gif([transform(laugh, dy=y, rot=a) for a, y in zip(angles, ys)],
             out / "factory_laugh_shake.gif", 70)


# ---------- stickers ----------

STICKER = 320


def fit(img, w, h):
    s = min(w / img.width, h / img.height)
    return img.resize((max(1, round(img.width * s)), max(1, round(img.height * s))), Image.LANCZOS)


def sticker_text(canvas, text, y, size, angle=0, fill=(255, 255, 255), stroke=(24, 28, 40)):
    f = font(size)
    d = ImageDraw.Draw(canvas)
    while d.textlength(text, font=f) > STICKER - 28 and size > 16:
        size -= 2
        f = font(size)
    tw = int(d.textlength(text, font=f)) + 40
    layer = Image.new("RGBA", (tw, size + 50), (0, 0, 0, 0))
    ld = ImageDraw.Draw(layer)
    ld.text((tw // 2, (size + 50) // 2), text, font=f, fill=fill, anchor="mm",
            stroke_width=max(4, size // 7), stroke_fill=stroke)
    layer = layer.rotate(angle, resample=Image.BICUBIC, expand=True)
    canvas.alpha_composite(layer, ((STICKER - layer.width) // 2, y - layer.height // 2))


# name, source cutout, text, text size
STICKERS = [
    ("gg", "gg", "", 0),
    ("on_it", "working", "On it!", 58),
    ("ship_it", "shipit", "Ship it", 58),
    ("hello", "wave", "Hello!", 58),
    ("thanks", "love", "Thanks!", 58),
    ("my_bad", "sorry", "My bad", 58),
    ("lets_build", "hammer", "Let's build", 50),
    ("victory", "cheer", "Victory!", 58),
]


def sticker(cut, out, name, src, text, size):
    im = Image.open(cut / f"{src}.png")
    room = STICKER - (62 if text else 12)
    body = outline(fit(im, STICKER - 28, room - 12), 6)
    canvas = Image.new("RGBA", (STICKER, STICKER), (0, 0, 0, 0))
    canvas.alpha_composite(body, ((STICKER - body.width) // 2, 4))
    if text:
        sticker_text(canvas, text, STICKER - 34, size, angle=-3)
    canvas.save(out / f"factory_{name}.png", optimize=True)


# ---------- server icon and banner ----------

def gradient_bg(w, h, inner, outer):
    base = Image.new("RGB", (w, h), outer)
    glow = Image.new("RGB", (w, h), inner)
    mask = Image.new("L", (w, h), 0)
    ImageDraw.Draw(mask).ellipse((-w * 0.2, -h * 0.35, w * 1.2, h * 1.05), fill=255)
    mask = mask.filter(ImageFilter.GaussianBlur(min(w, h) * 0.18))
    return Image.composite(glow, base, mask)


def brick_texture(bg, step, alpha=22):
    layer = Image.new("RGBA", bg.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    bh = step // 2
    for row, y in enumerate(range(0, bg.height, bh)):
        d.line((0, y, bg.width, y), fill=(255, 255, 255, alpha), width=2)
        off = (row % 2) * step // 2
        for x in range(-step, bg.width + step, step):
            d.line((x + off, y, x + off, y + bh), fill=(255, 255, 255, alpha), width=2)
    out = bg.convert("RGBA")
    out.alpha_composite(layer)
    return out


def server_icon(cut, out):
    bg = gradient_bg(1024, 1024, (70, 82, 140), (22, 24, 48))
    bg = brick_texture(bg, 128, 14)
    # the full mascot is scaled up so the body bleeds off the edges and the
    # circle crop does the cutting, with the face and chimney inside the circle
    im = Image.open(cut / "master.png")
    big = im.resize((1160, round(im.height * 1160 / im.width)), Image.LANCZOS)
    bg.alpha_composite(big, (-50, 85))
    bg.convert("RGB").save(out / "server-icon-1024.png", optimize=True)


def banner(cut, out):
    W, H = 960, 540
    bg = gradient_bg(W, H, (70, 82, 140), (18, 20, 40))
    bg = brick_texture(bg, 96, 12)
    glow = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    ImageDraw.Draw(glow).ellipse((470, 40, 940, 520), fill=(240, 120, 50, 70))
    bg.alpha_composite(glow.filter(ImageFilter.GaussianBlur(60)))
    mascot = fit(outline(Image.open(cut / "point-left.png"), 3), 420, 500)
    bg.alpha_composite(mascot, (W - mascot.width - 10, H - mascot.height - 20))
    d = ImageDraw.Draw(bg)
    d.text((64, 150), "HOI4", font=font(124), fill=(255, 255, 255), stroke_width=6, stroke_fill=(18, 20, 40))
    d.text((64, 272), "FACTORY", font=font(90), fill=(240, 130, 60), stroke_width=6, stroke_fill=(18, 20, 40))
    d.text((68, 396), "Build Hearts of Iron IV mods", font=font(30), fill=(222, 228, 245))
    d.text((68, 440), "Chaos Redux  |  Mod Setup  |  Agent Tools", font=font(22), fill=(160, 172, 210))
    bg.convert("RGB").save(out / "server-banner-960x540.png", optimize=True)


if __name__ == "__main__":
    cut, root = Path(sys.argv[1]), Path(sys.argv[2])
    for sub in ("emoji", "stickers", "server"):
        (root / sub).mkdir(parents=True, exist_ok=True)
    animated(cut, root / "emoji")
    for s in STICKERS:
        sticker(cut, root / "stickers", *s)
    server_icon(cut, root / "server")
    banner(cut, root / "server")
    print("done")
