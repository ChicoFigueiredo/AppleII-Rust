#!/usr/bin/env python3
"""Regenerates packaging/icons/* from AppleWin's APPLEWIN.ICO and DISK.ICO (GPL, AppleWin project).

Usage: python3 packaging/make_icons.py /path/to/AppleWin/resource
Needs Pillow. The originals only carry 16x16 and 32x32 frames, so larger sizes are the 32x32
frame upscaled with nearest-neighbour (crisp pixel-art look, no blur).
"""
import sys
from pathlib import Path
from PIL import Image

SIZES = [16, 24, 32, 48, 64, 128, 256, 512]
out = Path(__file__).resolve().parent / "icons"
res = Path(sys.argv[1] if len(sys.argv) > 1 else "resource")


def base(path):
    im = Image.open(path)
    return im.ico.getimage(max(im.ico.sizes())).convert("RGBA")


def render(src, size):
    return src.resize((size, size), Image.NEAREST)


out.mkdir(parents=True, exist_ok=True)
for ico, name in (("APPLEWIN.ICO", "apple2-rust"), ("DISK.ICO", "disk")):
    src = base(res / ico)
    for s in SIZES:
        render(src, s).save(out / f"{name}-{s}.png")
    render(src, 256).save(out / f"{name}.ico", sizes=[(s, s) for s in (16, 24, 32, 48, 64, 128, 256)])
    render(src, 512).save(out / f"{name}.icns", format="ICNS")
print("ok", sorted(p.name for p in out.iterdir()))
