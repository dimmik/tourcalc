#!/usr/bin/env python3
"""Renders an icon set from the two SVGs beside it.

    python3 render-icons.py .        # the app's own icons
    python3 render-icons.py beta     # the beta build's, which the server serves instead

`favicon.svg` is the tile with its own rounded corners - browsers, bookmarks, the tab
strip. `icon-maskable.svg` is the same drawing full-bleed, for a launcher that cuts it to
its own shape. Everything else in the directory is rendered from those two and is not
worth editing by hand.

Needs cairosvg and pillow:  pip install cairosvg pillow
"""
import sys, os, io
import cairosvg
from PIL import Image

# what is rendered from which source, and at what size
FROM_TILE = [("icon-180.png", 180), ("icon-192.png", 192), ("icon-512.png", 512),
             ("ms-icon-150x150.png", 150), ("ms-icon-310x310.png", 310)]
FROM_MASK = [("icon-maskable-512.png", 512)]
# the .ico carries the three sizes Windows and old browsers ask for, in one file
ICO_SIZES = [(16, 16), (32, 32), (48, 48)]


def png(src, out, size):
    cairosvg.svg2png(url=src, write_to=out, output_width=size, output_height=size)
    print(f"  {out}  {size}x{size}")


def main(where):
    tile = os.path.join(where, "favicon.svg")
    mask = os.path.join(where, "icon-maskable.svg")
    for f in (tile, mask):
        if not os.path.exists(f):
            sys.exit(f"no {f} - this is not an icon directory")
    print(f"rendering {where}:")
    for name, size in FROM_TILE:
        png(tile, os.path.join(where, name), size)
    for name, size in FROM_MASK:
        # full-bleed, so no transparency to keep
        raw = cairosvg.svg2png(url=mask, output_width=size, output_height=size)
        Image.open(io.BytesIO(raw)).convert("RGB").save(os.path.join(where, name))
        print(f"  {name}  {size}x{size}")
    raw = cairosvg.svg2png(url=tile, output_width=48, output_height=48)
    Image.open(io.BytesIO(raw)).save(os.path.join(where, "favicon.ico"), sizes=ICO_SIZES)
    print("  favicon.ico  16/32/48")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else ".")
