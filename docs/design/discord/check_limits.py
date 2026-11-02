"""Check every Discord asset against Discord's upload limits.

Usage: check_limits.py DISCORD_DIR
Exit code 1 when any file breaks a limit.
"""
import sys
from pathlib import Path

from PIL import Image

root = Path(sys.argv[1])
bad = 0


def check(path, size, max_kb, kinds):
    global bad
    im = Image.open(path)
    kb = path.stat().st_size / 1024
    ok = im.size == size and kb < max_kb and im.format in kinds
    frames = getattr(im, "n_frames", 1)
    print(("ok   " if ok else "FAIL ") + f"{path.relative_to(root)} {im.size} {im.format} {kb:.0f} KB frames={frames}")
    if not ok:
        bad += 1


for p in sorted((root / "emoji").glob("*.png")):
    check(p, (128, 128), 256, {"PNG"})
for p in sorted((root / "emoji").glob("*.gif")):
    check(p, (128, 128), 256, {"GIF"})
for p in sorted((root / "stickers").glob("*.png")):
    check(p, (320, 320), 512, {"PNG"})
check(root / "server" / "server-icon-1024.png", (1024, 1024), 10240, {"PNG"})
check(root / "server" / "server-banner-960x540.png", (960, 540), 10240, {"PNG"})
sys.exit(1 if bad else 0)
