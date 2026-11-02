"""Build dark and light contact sheets of the Discord assets.

Usage: build_contact_sheet.py DISCORD_DIR
"""
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

FONT = "C:/Windows/Fonts/arialbd.ttf"


def sheet(root, bg, fg, name):
    emoji = sorted((root / "emoji").glob("*.png")) + sorted((root / "emoji").glob("*.gif"))
    stickers = sorted((root / "stickers").glob("*.png"))
    f = ImageFont.truetype(FONT, 16)
    big = ImageFont.truetype(FONT, 22)
    W = 1500
    cols = 8
    cell = 180
    rows_e = (len(emoji) + cols - 1) // cols
    s_cols = 4
    s_cell = 340
    rows_s = (len(stickers) + s_cols - 1) // s_cols
    H = 60 + rows_e * cell + 70 + rows_s * s_cell + 60 + 560
    img = Image.new("RGB", (W, H), bg)
    d = ImageDraw.Draw(img)
    y = 16
    d.text((24, y), "Emoji (128 px, with 32 px and 22 px size below)", font=big, fill=fg)
    y += 40
    for i, p in enumerate(emoji):
        im = Image.open(p).convert("RGBA")
        x = 20 + (i % cols) * (W - 40) // cols
        yy = y + (i // cols) * cell
        img.paste(im, (x, yy), im)
        for sz, dx in ((32, 136), (22, 136)):
            s = im.resize((sz, sz), Image.LANCZOS)
            img.paste(s, (x + dx, yy + (0 if sz == 32 else 40)), s)
        d.text((x, yy + 130), p.stem.replace("factory_", ""), font=f, fill=fg)
    y += rows_e * cell + 20
    d.text((24, y), "Stickers (320 px)", font=big, fill=fg)
    y += 40
    for i, p in enumerate(stickers):
        im = Image.open(p).convert("RGBA")
        x = 20 + (i % s_cols) * (W - 40) // s_cols
        img.paste(im, (x, y + (i // s_cols) * s_cell), im)
    y += rows_s * s_cell + 20
    d.text((24, y), "Server icon (circle crop) and banner", font=big, fill=fg)
    y += 40
    ic = Image.open(root / "server" / "server-icon-1024.png").convert("RGBA")
    for k, sz in enumerate((256, 96, 48)):
        s = ic.resize((sz, sz), Image.LANCZOS)
        m = Image.new("L", (sz * 4, sz * 4), 0)
        ImageDraw.Draw(m).ellipse((0, 0, sz * 4 - 1, sz * 4 - 1), fill=255)
        m = m.resize((sz, sz), Image.LANCZOS)
        xs = [24, 300, 420][k]
        img.paste(s, (xs, y), m)
    b = Image.open(root / "server" / "server-banner-960x540.png").resize((720, 405))
    img.paste(b, (560, y))
    img.crop((0, 0, W, y + 420)).save(root / name, optimize=True)


if __name__ == "__main__":
    root = Path(sys.argv[1])
    sheet(root, (49, 51, 56), (230, 232, 236), "contact-sheet-dark.png")
    sheet(root, (255, 255, 255), (30, 32, 36), "contact-sheet-light.png")
