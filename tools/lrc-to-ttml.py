#!/usr/bin/env python3
r"""
Convert line-level .lrc files to TTML with word-level timings.

Why TTML: Steve Player's description is explicit that word-by-word highlighting
comes *only* from TTML --

    "Import LRC, TTML, DFXP, or plain-text lyrics ... Line-synced lyrics scroll
     automatically ... Word-by-word, beat-by-beat highlighting **from TTML**"

LRC is therefore line-synced only on that player, and its LRC reader does not
strip inline `<mm:ss.xx>` tags, so A2-enhanced LRC shows the timestamps as
lyric text. Gramophone/Fluxio read A2 LRC; Steve needs TTML. Pick the format to
match the player, not the other way round.

Output shape (Apple Music style, as consumed by AMLL-compatible players):

    <tt xmlns="http://www.w3.org/ns/ttml"
        xmlns:ttm="http://www.w3.org/ns/ttml#metadata"
        xmlns:itunes="http://music.apple.com/lyric-ttml-internal"
        itunes:timing="Word">
      <body><div>
        <p begin="00:29.250" end="00:33.410">
          <span begin="00:29.250" end="00:30.760">2002</span>
          <span begin="00:30.760" end="00:31.140">年</span>
        </p>
      </div></body>
    </tt>

Spaces between Latin words are written *outside* the spans (the "most
compliant" option in the AMLL spec), so the visible text is unchanged while CJK
runs stay glued together.

Segmentation uses Charabia via `segcli`; timing is proportional to token
length. This is an estimate: a held note or a fast run will drift.

Usage:
    python3 lrc-to-ttml.py FILE.lrc ...          # writes FILE.ttml beside it
    python3 lrc-to-ttml.py --dry-run FILE.lrc
    SEGCLI=/path/to/segcli python3 lrc-to-ttml.py ...
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from xml.sax.saxutils import escape

LINE_TAG = re.compile(r"^\[(\d{1,3}):(\d{2})(?:[.:](\d{1,3}))?\]")
ID_TAG = re.compile(r"^\[([a-zA-Z#]+):([^\]]*)\]")
WORD_TAG = re.compile(r"<\d{1,3}:\d{2}(?:[.:]\d{1,3})?>")

CREDIT_RE = re.compile(
    r"^\s*(作词|作曲|编曲|制作|出品|监制|录音|混音|母带|词|曲|"
    r"lyrics?|music|produced|written)\s*[:：]",
    re.IGNORECASE,
)

MAX_SPAN_MS = 12_000
MIN_SPAN_MS = 400

SEGCLI_CANDIDATES = [
    os.environ.get("SEGCLI", ""),
    os.path.join(os.path.dirname(os.path.abspath(__file__)), "segcli", "target", "release", "segcli"),
    "/tmp/segcli/target/release/segcli",
]


def segcli_path() -> str:
    for path in SEGCLI_CANDIDATES:
        if path and os.path.isfile(path) and os.access(path, os.X_OK):
            return path
    sys.exit("segcli not found; build tools/segcli or set SEGCLI=/path/to/segcli")


def segment_batch(texts: list[str]) -> list[list[str]]:
    proc = subprocess.run(
        [segcli_path()],
        input="\n".join(t.replace("\n", " ") for t in texts) + "\n",
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        sys.exit(f"segcli failed: {proc.stderr}")
    out = [json.loads(line) for line in proc.stdout.splitlines()]
    if len(out) != len(texts):
        sys.exit(f"segcli returned {len(out)} lines for {len(texts)} inputs")
    return out


def parse_time(minutes: str, seconds: str, frac: str | None) -> int:
    ms = int(minutes) * 60_000 + int(seconds) * 1_000
    if frac:
        ms += int(frac) * (100 if len(frac) == 1 else 10 if len(frac) == 2 else 1)
    return ms


def timestamp(ms: int) -> str:
    ms = max(0, ms)
    return f"{ms // 3_600_000:02d}:{(ms % 3_600_000) // 60_000:02d}:{(ms % 60_000) / 1000:06.3f}"


def distribute(tokens: list[str], start_ms: int, end_ms: int) -> list[int]:
    total = end_ms - start_ms
    if total <= 0 or not tokens:
        return [start_ms] * len(tokens)
    weights = [max(1, len(t.strip())) for t in tokens]
    total_weight = sum(weights)
    starts, elapsed = [], 0.0
    for weight in weights:
        starts.append(start_ms + int(elapsed * total / total_weight))
        elapsed += weight
    return starts


def clean_tokens(tokens: list[str]) -> list[str]:
    """Merge contractions and fold punctuation; keep Latin inter-word spaces.

    A token that ends with a space keeps it: the space is written between spans
    in the TTML output, which is how the visible text stays identical to the
    source line.
    """
    merged: list[str] = []
    for token in tokens:
        glued = token.startswith(("'", "\u2019"))
        spaced = token[:1].isspace()
        if merged and glued and not spaced:
            merged[-1] += token
        else:
            merged.append(token)

    out: list[str] = []
    for token in merged:
        if not token:
            continue
        if not token.strip():
            # whitespace-only: attach to the previous token so the line's spacing
            # survives without becoming its own timed span
            if out:
                out[-1] += token
            continue
        if all(not ch.isalnum() for ch in token.strip()):
            if out:
                out[-1] += token
            else:
                out.append(token)
            continue

        leading = ""
        for ch in token:
            if ch.isalnum() or ch.isspace():
                break
            leading += ch
        if leading and out:
            out[-1] += leading
            token = token[len(leading):]
            if not token:
                continue
        out.append(token)
    return out


def parse_lrc(text: str) -> tuple[dict[str, str], list[tuple[int, str]]]:
    """Return (id tags, [(start_ms, body)])."""
    meta: dict[str, str] = {}
    entries: list[tuple[int, str]] = []
    for raw in text.splitlines():
        line = raw.strip()
        if not line:
            continue
        idm = ID_TAG.match(line)
        if idm:
            meta[idm.group(1).lower()] = idm.group(2).strip()
            continue
        m = LINE_TAG.match(line)
        if not m:
            continue
        start = parse_time(m.group(1), m.group(2), m.group(3))
        offset = m.end()
        while True:
            extra = LINE_TAG.match(line[offset:])
            if not extra:
                break
            offset += extra.end()
        body = WORD_TAG.sub("", line[offset:]).strip()
        if body:
            entries.append((start, body))
    entries.sort(key=lambda e: e[0])
    return meta, entries


def build_ttml(meta: dict[str, str], entries: list[tuple[int, str]], title: str) -> str:
    plan: list[tuple[int, int, str]] = []
    for index, (start, body) in enumerate(entries):
        if CREDIT_RE.search(body):
            continue
        end = entries[index + 1][0] if index + 1 < len(entries) else start + 3_000
        if end - start > MAX_SPAN_MS:
            end = start + 3_000  # intro/instrumental gap: do not stretch words
        if end - start < MIN_SPAN_MS:
            end = start + MIN_SPAN_MS
        plan.append((start, end, body))

    tokens_per_line = segment_batch([p[2] for p in plan]) if plan else []

    parts = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        '<tt xmlns="http://www.w3.org/ns/ttml"',
        '    xmlns:ttm="http://www.w3.org/ns/ttml#metadata"',
        '    xmlns:itunes="http://music.apple.com/lyric-ttml-internal"',
        '    itunes:timing="Word">',
        "  <head>",
        "    <metadata>",
        f"      <ttm:title>{escape(title)}</ttm:title>",
    ]
    if meta.get("ar"):
        parts.append(f"      <ttm:artist>{escape(meta['ar'])}</ttm:artist>")
    if meta.get("al"):
        parts.append(f"      <ttm:album>{escape(meta['al'])}</ttm:album>")
    parts += ["    </metadata>", "  </head>", "  <body>", "    <div>"]

    for (start, end, body), tokens in zip(plan, tokens_per_line):
        tokens = clean_tokens(tokens)
        if len(tokens) < 2:
            parts.append(f'      <p begin="{timestamp(start)}" end="{timestamp(end)}">{escape(body)}</p>')
            continue

        starts = distribute([t.rstrip() for t in tokens], start, end)
        pieces = []
        for index, (token, token_start) in enumerate(zip(tokens, starts)):
            token_end = starts[index + 1] if index + 1 < len(starts) else end
            stripped = token.rstrip()
            trailing_space = " " if token != token.rstrip() else ""
            pieces.append(
                f'<span begin="{timestamp(token_start)}" end="{timestamp(token_end)}">'
                f"{escape(stripped)}</span>{trailing_space}"
            )
        parts.append(f'      <p begin="{timestamp(start)}" end="{timestamp(end)}">{"".join(pieces)}</p>')

    parts += ["    </div>", "  </body>", "</tt>", ""]
    return "\n".join(parts)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("files", nargs="+")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--keep-lrc", action="store_true", help="keep the source .lrc")
    args = ap.parse_args()

    for path in args.files:
        try:
            text = open(path, encoding="utf-8").read()
        except UnicodeDecodeError:
            print(f"{path}: not UTF-8, skipped", file=sys.stderr)
            continue

        meta, entries = parse_lrc(text)
        if not entries:
            print(f"{path}: no timed lines, skipped")
            continue

        title = meta.get("ti") or os.path.splitext(os.path.basename(path))[0]
        ttml = build_ttml(meta, entries, title)
        out_path = os.path.splitext(path)[0] + ".ttml"

        if args.dry_run:
            print(f"=== {path} -> {os.path.basename(out_path)} ({len(entries)} lines) ===")
            print("\n".join(ttml.splitlines()[:18]))
            continue

        open(out_path, "w", encoding="utf-8").write(ttml)
        if not args.keep_lrc:
            os.remove(path)
        print(f"{path}: wrote {os.path.basename(out_path)} ({len(entries)} lines)")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
