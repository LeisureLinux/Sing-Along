#!/usr/bin/env python3
"""Build the donation QR image shown on the About page.

Crops the white card out of the WeChat收款码 screenshot (the green surround and
the phone's status bar are not part of the code), puts a title line above it,
and writes a PNG suitable for embedding in the app.

    python3 tools/make-donate-qr.py SOURCE.jpg OUT.png
"""

import sys

import numpy as np
from PIL import Image, ImageDraw, ImageFont

FONT = "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"
FONT_BOLD = "/usr/share/fonts/opentype/noto/NotoSansCJK-Medium.ttc"

GREEN = (88, 190, 107)  # WeChat's card surround, sampled from the source
TITLE = "感谢对 Sing-Along 项目的认可"


def crop_qr(src: Image.Image) -> Image.Image:
    """Return the QR modules with an even quiet zone around them.

    The screenshot's card has uneven padding — 88px above the code, 37px
    below — so cropping the card leaves the code visibly off-centre. The
    modules themselves are found by their dark pixels and the quiet zone is
    rebuilt at a uniform width.
    """
    rgb = src.convert("RGB")
    px = np.asarray(rgb).astype(int)

    # The green surround is not part of anything we want; drop it first so its
    # darkness cannot be mistaken for QR modules.
    surround = np.array(GREEN)
    is_content = np.abs(px - surround).sum(axis=2) > 40
    rows = np.where(is_content.any(axis=1))[0]
    cols = np.where(is_content.any(axis=0))[0]
    card = px[rows.min() : rows.max() + 1, cols.min() : cols.max() + 1]

    # Drop any green tint left from the card's border or the screenshot's
    # antialiased edge: the card is white inside, so a greenish pixel anywhere
    # here is contamination, not content.
    r_ch, g_ch, b_ch = card[:, :, 0], card[:, :, 1], card[:, :, 2]
    greenish = (g_ch > r_ch + 25) & (g_ch > b_ch + 25)
    card[greenish] = 255

    # QR modules are the dark pixels; everything else in the card is white.
    dark = card.sum(axis=2) < 128 * 3
    rows = np.where(dark.any(axis=1))[0]
    cols = np.where(dark.any(axis=0))[0]
    top, bottom = rows.min(), rows.max() + 1
    left, right = cols.min(), cols.max() + 1

    # A QR symbol needs 4 modules of quiet zone to scan reliably. The module
    # size is unknown, but 1/25th of the code's width is 4 modules for the
    # common 21-29 module layouts (version 1-3) and wider ones only gain margin.
    size = max(right - left, bottom - top)
    quiet = max(16, round(size / 25))

    canvas = Image.new("RGB", (size + quiet * 2, size + quiet * 2), (255, 255, 255))
    code = Image.fromarray(card[top:bottom, left:right].astype("uint8"), "RGB")
    canvas.paste(code, (quiet + (size - code.width) // 2, quiet + (size - code.height) // 2))
    return canvas


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__)
        return 2
    src_path, out_path = sys.argv[1], sys.argv[2]

    qr = crop_qr(Image.open(src_path))

    # The title has to fit inside the QR's own width, otherwise the canvas grows
    # sideways and the code ends up floating in whitespace. Rather than wrap the
    # line, step the font size down until it fits.
    margin = 20
    title_gap = 22

    probe = ImageDraw.Draw(Image.new("RGB", (1, 1)))
    max_text_w = qr.width - margin * 2

    title_size = 38
    while title_size > 12:
        font = ImageFont.truetype(FONT_BOLD, title_size)
        if probe.textbbox((0, 0), TITLE, font=font)[2] <= max_text_w:
            break
        title_size -= 1

    font = ImageFont.truetype(FONT_BOLD, title_size)
    # Use the glyph box, not the font metrics: the metrics include leading above
    # and below the glyphs, which would silently eat the gap we just reserved.
    box = probe.textbbox((0, 0), TITLE, font=font)
    text_w, text_h = box[2] - box[0], box[3] - box[1]

    width = qr.width + margin * 2
    height = margin + text_h + title_gap + qr.height + margin

    canvas = Image.new("RGB", (width, height), (255, 255, 255))
    draw = ImageDraw.Draw(canvas)

    draw.text(
        ((width - text_w) // 2, margin - box[1]),
        TITLE,
        font=font,
        fill=(32, 32, 32),
    )

    card_x = (width - qr.width) // 2
    card_y = margin + text_h + title_gap
    canvas.paste(qr, (card_x, card_y))

    canvas.save(out_path, "PNG", optimize=True)
    print(f"{out_path}: {canvas.width}x{canvas.height} (title {title_size}px)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
