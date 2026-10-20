#!/usr/bin/env python3
"""Verify a TTML lyric file against its source LRC.

Word-level lyric conversion is lossy in a way that is invisible until it is
displayed: a token split wrongly, or a word tag left in as text, still parses as
valid TTML. The only check that catches it is comparing the visible text.

    python3 tools/verify-ttml.py track.ttml [--against track.lrc]

Prints each paragraph's text and, when a source is given, reports any line that
does not match. Exits non-zero on a mismatch.
"""

import re
import sys
from pathlib import Path

PARAGRAPH_RE = re.compile(r"<p[^>]*>(.*?)</p>", re.S)
TAG_RE = re.compile(r"<[^>]+>")


def ttml_lines(path: Path) -> list[str]:
    text = path.read_text(encoding="utf-8")
    return [TAG_RE.sub("", para) for para in PARAGRAPH_RE.findall(text)]


# Prefixes that mark a credit line ("作词：方文山", "Produced by …"). The word-sync
# converter drops these on purpose: they carry no singing, and putting them in
# karaoke sweeps is wrong. The comparison has to drop the same ones or it reports
# a mismatch on every Chinese lyric file.
CREDIT_PREFIXES = (
    "作词", "作曲", "编曲", "制作", "出品", "监制", "录音", "混音", "母带", "词", "曲",
    "lyrics", "music", "produced", "written", "composed",
)


def is_credit_line(text: str) -> bool:
    head = text.strip().lower()
    for prefix in CREDIT_PREFIXES:
        if head.startswith(prefix):
            rest = head[len(prefix):].lstrip()
            if rest.startswith(":") or rest.startswith("："):
                return True
    return False


def lrc_lines(path: Path, keep_credits: bool = False) -> list[str]:
    """Timed lines of an LRC file, with any A2 word tags stripped."""
    out = []
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = re.sub(r"<[0-9:.]*>", "", raw)
        line = re.sub(r"^\[[0-9:.]*\]", "", line).strip()
        if line and (keep_credits or not is_credit_line(line)):
            out.append(line)
    return out


def main() -> int:
    args = [a for a in sys.argv[1:]]
    if not args:
        print(__doc__)
        return 2

    ttml_path = Path(args[0])
    source = None
    if "--against" in args:
        source = Path(args[args.index("--against") + 1])

    converted = ttml_lines(ttml_path)
    if not converted:
        print(f"no <p> elements found in {ttml_path}", file=sys.stderr)
        return 1

    for line in converted:
        print(line)

    if source is None:
        print(f"\n{len(converted)} paragraphs")
        return 0

    original = lrc_lines(source)
    # A TTML conversion may merge or split lines, so compare the joined text
    # rather than line by line: that catches dropped or duplicated words without
    # failing on legitimate whitespace handling.
    if "".join(converted).replace(" ", "") != "".join(original).replace(" ", ""):
        print("\nMISMATCH: converted text does not match the source", file=sys.stderr)
        return 1

    print(f"\n{len(converted)} paragraphs, text matches {source.name}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
