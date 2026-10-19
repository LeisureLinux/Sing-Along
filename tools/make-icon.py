#!/usr/bin/env python3
"""
Generate the Sing-Along app icon.

Mark: a microphone silhouette with a lyric "sweep" bar across it, on a dark
rounded square. The sweep refers to the app's actual job — following a lyric
line as it is sung — and distinguishes it from a generic microphone glyph.

The previous icon belonged to the upstream project; this one is generated here
so the fork ships its own identity rather than borrowed art.

Everything is drawn from primitives so the source stays readable and the output
is reproducible: no binary source image is checked in, only this script and the
PNGs it produces.

Usage:
    python3 tools/make-icon.py            # writes src-tauri/icons/*.png
"""

from __future__ import annotations

import math
import os
import pathlib

from PIL import Image, ImageDraw

SIZE = 1024  # drawn at 1024 and downscaled, so curves stay clean
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ICONS = os.path.join(ROOT, "src-tauri", "icons")

# Palette: graphite base with a warm amber accent, echoing the karaoke highlight
# used in the app and staying readable on both light and dark desktops.
BASE_TOP = (28, 32, 40)
BASE_BOTTOM = (16, 18, 24)
ACCENT = (251, 176, 59)
ACCENT_SOFT = (255, 214, 140)
MIC = (238, 242, 248)

# macOS icons need breathing room; Windows and Linux tiles fill more of the
# canvas. 0.84 keeps the mark clear of the rounded corners on all three.
INSET = 0.84


def rounded_mask(size: int, radius_ratio: float) -> Image.Image:
    mask = Image.new("L", (size, size), 0)
    draw = ImageDraw.Draw(mask)
    pad = int(size * (1 - INSET) / 2)
    radius = int(size * radius_ratio)
    draw.rounded_rectangle([pad, pad, size - pad, size - pad], radius=radius, fill=255)
    return mask


def draw_vertical_gradient(size: int, top: tuple, bottom: tuple) -> Image.Image:
    gradient = Image.new("RGB", (1, size))
    pixels = gradient.load()
    for y in range(size):
        t = y / max(1, size - 1)
        pixels[0, y] = tuple(round(top[i] + (bottom[i] - top[i]) * t) for i in range(3))
    return gradient.resize((size, size), Image.BILINEAR)


def draw_mark(size: int, simple: bool = False) -> Image.Image:
    """The microphone + lyric sweep, drawn on a transparent canvas.

    `simple` keeps the capsule and the sweep but drops the cradle, stem and base.
    Below roughly 48px those three segments are one or two pixels tall each and
    antialias into a smear — at 32px the full mark reads as a blob, while capsule
    plus sweep still reads as a microphone.
    """
    layer = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(layer)

    cx = size / 2

    # --- microphone capsule -------------------------------------------------
    capsule_w = size * 0.175
    capsule_h = size * 0.30
    capsule_top = size * 0.150
    draw.rounded_rectangle(
        [cx - capsule_w / 2, capsule_top, cx + capsule_w / 2, capsule_top + capsule_h],
        radius=capsule_w / 2,
        fill=MIC,
    )

    if simple:
        _draw_sweep(draw, size, cx)
        return layer

    # --- cradle: the arc under the capsule ----------------------------------
    arc_r = size * 0.165
    arc_cy = capsule_top + capsule_h * 0.62
    # Thicker than a literal microphone: at 32px a 1px line antialiases
    # away entirely, and the mark loses its stem.
    width = max(3, int(size * 0.042))
    draw.arc(
        [cx - arc_r, arc_cy - arc_r, cx + arc_r, arc_cy + arc_r],
        start=20,
        end=160,
        fill=MIC,
        width=width,
    )

    # --- stem and base ------------------------------------------------------
    stem_top = arc_cy + arc_r - size * 0.004
    stem_bottom = size * 0.560
    draw.line([cx, stem_top, cx, stem_bottom], fill=MIC, width=width)
    base_half = size * 0.075
    draw.rounded_rectangle(
        [cx - base_half, stem_bottom, cx + base_half, stem_bottom + size * 0.030],
        radius=size * 0.014,
        fill=MIC,
    )

    _draw_sweep(draw, size, cx)
    return layer


def _draw_sweep(draw: ImageDraw.ImageDraw, size: int, cx: float) -> None:
    """Three bars of differing length, the middle one solid.

    A lyric line with the current word highlighted, which is what the app does.
    Drawn below the microphone rather than across it: an earlier version put them
    over the capsule, where the opaque bars covered the cradle arc entirely and
    the mark read as a blob.
    """
    bar_h = size * 0.040
    gap = size * 0.046
    sweep_cy = size * 0.680
    lengths = (0.26, 0.38, 0.20)
    colors = (ACCENT_SOFT, ACCENT, ACCENT_SOFT)
    for index, (length, color) in enumerate(zip(lengths, colors)):
        y = sweep_cy + (index - 1) * gap
        half = size * length / 2
        draw.rounded_rectangle(
            [cx - half, y - bar_h / 2, cx + half, y + bar_h / 2],
            radius=bar_h / 2,
            fill=color,
        )


def compose(size: int, simple: bool = False) -> Image.Image:
    base = draw_vertical_gradient(size, BASE_TOP, BASE_BOTTOM).convert("RGBA")

    # A soft highlight in the upper-left keeps a flat gradient from looking dead.
    glow = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    glow_draw = ImageDraw.Draw(glow)
    glow_draw.ellipse(
        [-size * 0.25, -size * 0.35, size * 0.75, size * 0.45],
        fill=(255, 255, 255, 20),
    )
    base = Image.alpha_composite(base, glow)

    base = Image.alpha_composite(base, draw_mark(size, simple=simple))
    base.putalpha(rounded_mask(size, 0.225))
    return base


def main() -> None:
    master = compose(SIZE)

    os.makedirs(ICONS, exist_ok=True)

    # Desktop PNGs required by tauri.conf.json plus the Windows store tiles.
    targets = {
        "32x32.png": 32,
        "128x128.png": 128,
        "128x128@2x.png": 256,
        "icon.png": 512,
        "Square30x30Logo.png": 30,
        "Square44x44Logo.png": 44,
        "Square71x71Logo.png": 71,
        "Square89x89Logo.png": 89,
        "Square107x107Logo.png": 107,
        "Square142x142Logo.png": 142,
        "Square150x150Logo.png": 150,
        "Square284x284Logo.png": 284,
        "Square310x310Logo.png": 310,
        "StoreLogo.png": 50,
    }

    # Small sizes are drawn with the simple mark rather than downscaled from
    # the full one: the microphone's stem is about two pixels tall at 32px and
    # antialiases into nothing, leaving a gap where the mark should be. Drawing
    # the reduced mark at target size keeps every element at least a pixel wide.
    small = compose(SIZE, simple=True)

    for name, px in targets.items():
        source = small if px <= 48 else master
        resized = source.resize((px, px), Image.LANCZOS)
        resized.save(os.path.join(ICONS, name))
        print(f"  {name:28s} {px}px{'  (simple mark)' if px <= 48 else ''}")

    # macOS .icns wants its own set inside icons/macos.
    macos = os.path.join(ICONS, "macos")
    os.makedirs(macos, exist_ok=True)
    for name, px in {"32x32.png": 32, "128x128.png": 128, "128x128@2x.png": 256, "icon.png": 512}.items():
        master.resize((px, px), Image.LANCZOS).save(os.path.join(macos, name))
        print(f"  macos/{name:22s} {px}px")

    # .ico for Windows: several sizes in one file.
    master.save(
        os.path.join(ICONS, "icon.ico"),
        sizes=[(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )
    print("  icon.ico                     multi-size")

    # macOS .icns: a container of PNGs. Written as ic07/ic08/ic09 (128/256/512),
    # the PNG-based types modern macOS reads.
    import io
    import struct

    def _icns(images: dict) -> bytes:
        body = b""
        for type4, data in images.items():
            body += type4.encode("ascii") + struct.pack(">I", len(data)) + data
        return b"icns" + struct.pack(">I", len(body) + 8) + body

    icns_images = {}
    for type4, px in {"ic07": 128, "ic08": 256, "ic09": 512}.items():
        buf = io.BytesIO()
        master.resize((px, px), Image.LANCZOS).save(buf, "PNG")
        icns_images[type4] = buf.getvalue()

    icns_bytes = _icns(icns_images)
    for out in ("icon.icns", os.path.join("macos", "icon.icns")):
        with open(os.path.join(ICONS, out), "wb") as fh:
            fh.write(icns_bytes)
        print(f"  {out:28s} icns")

    print(f"\nwrote icons into {ICONS}")


if __name__ == "__main__":
    main()
