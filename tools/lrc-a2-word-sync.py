#!/usr/bin/env python3
r"""
Add standard enhanced-LRC (A2) word timings to line-level .lrc files.

Target format -- the one Gramophone and Fluxio both parse:

    [00:29.25]<00:29.25>2002<00:30.76>年<00:31.14>的<00:31.52>第一<00:32.27>场<00:32.65>雪

Grammar, from Gramophone's SemanticLyrics.parseLrc:

    timeMarksRegex     = \[(\d+)[.:](\d+)([.:]\d+)?]         line timestamps
    timeWordMarksRegex = <(\d+)[.:](\d+)([.:]\d+)?>          word sync points
    is LyricText -> currentLine.add(...(lastWordSyncPoint, element.text))

`element.text` is inserted verbatim, so a Chinese line keeps its original
characters with no spaces inserted. That is the key difference from Booming,
whose parser appends a space after every non-final word and therefore renders
CJK lyrics as "雨 一直 下".

Segmentation comes from Charabia (the `segcli` helper), not fixed-width
slicing: it yields 是/你/的/甜言/蜜语/改变/季节 rather than 是你的/甜言/蜜语.

Timing is proportional to token length rather than a flat divide, so a
four-character word occupies more of the line than a particle. It is still an
estimate -- there is no audio analysis -- so held notes and fast runs drift.

Usage:
    python3 lrc-a2-word-sync.py FILE.lrc ...
    python3 lrc-a2-word-sync.py --dry-run FILE.lrc
    python3 lrc-a2-word-sync.py --revert FILE.lrc ...
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys

LINE_TAG = re.compile(r"^\[(\d{1,3}):(\d{2})(?:[.:](\d{1,3}))?\]")
ID_TAG = re.compile(r"^\[[a-zA-Z#]+:[^\]]*\]")
WORD_TAG = re.compile(r"<\d{1,3}:\d{2}(?:[.:]\d{1,3})?>")

CREDIT_RE = re.compile(
    r"^\s*(作词|作曲|编曲|制作|出品|监制|录音|混音|母带|词|曲|"
    r"lyrics?|music|produced|written)\s*[:：]",
    re.IGNORECASE,
)

MAX_SPAN_MS = 12_000

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


def stamp(ms: int) -> str:
    ms = max(0, ms)
    return f"{ms // 60_000:02d}:{(ms % 60_000) // 1_000:02d}.{(ms % 1_000) // 10:02d}"


def distribute(tokens: list[str], start_ms: int, end_ms: int) -> list[int]:
    total = end_ms - start_ms
    if total <= 0 or not tokens:
        return [start_ms] * len(tokens)
    weights = [max(1, len(t.strip())) for t in tokens]
    weight_sum = sum(weights)
    starts, elapsed = [], 0.0
    for weight in weights:
        starts.append(start_ms + int(elapsed * total / weight_sum))
        elapsed += weight
    return starts


def clean_tokens(tokens: list[str]) -> list[str]:
    """Merge contractions and fold punctuation so nothing decorative gets a slot."""
    merged: list[str] = []
    for token in tokens:
        # Merge an apostrophe contraction ("it's" -> ["it", "'s"]) only when the
        # apostrophe is glued to the previous token. A leading "'round" or
        # "'cause" is a separate word after a space and must stay detached,
        # otherwise the visible text loses that space ("else 'round" ->
        # "else'round").
        glued = token.startswith(("'", "\u2019"))
        spaced = token.startswith((" ", "\t")) or token[:1].isspace()
        if merged and glued and not spaced:
            merged[-1] += token
        else:
            merged.append(token)

    out: list[str] = []
    for token in merged:
        if not token:
            continue
        if not token.strip():
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


def revert(text: str) -> tuple[str, int]:
    kept, dropped = [], 0
    for line in text.splitlines(keepends=True):
        new = WORD_TAG.sub("", line)
        if new != line:
            dropped += 1
        kept.append(new)
    return "".join(kept), dropped


def convert(text: str) -> tuple[str, int, int]:
    lines = text.splitlines(keepends=True)

    timed: list[tuple[int, int, str]] = []
    for index, raw in enumerate(lines):
        stripped = raw.strip()
        if not stripped or ID_TAG.match(stripped):
            continue
        match = LINE_TAG.match(stripped)
        if not match:
            continue
        start = parse_time(match.group(1), match.group(2), match.group(3))
        # Consume every leading timestamp, not just the first: a repeated chorus
        # is written "[02:37.49][04:39.23]歌词", and leaving the second one in the
        # body made it part of the sung text.
        body_offset = match.end()
        while True:
            extra = LINE_TAG.match(stripped[body_offset:])
            if not extra:
                break
            body_offset += extra.end()
        body = stripped[body_offset:].strip()
        if WORD_TAG.search(body):
            return text, 0, 0  # already converted
        timed.append((index, start, body))

    plan: list[tuple[int, int, int, str]] = []
    skipped = 0
    for position, (index, start, body) in enumerate(timed):
        if CREDIT_RE.search(body) or not body:
            skipped += 1
            continue
        if position + 1 < len(timed):
            end = timed[position + 1][1]
            if end - start > MAX_SPAN_MS:
                skipped += 1
                continue
        else:
            end = start + 3_000
        plan.append((index, start, end, body))

    if not plan:
        return text, 0, skipped

    tokens_per_line = segment_batch([p[3] for p in plan])

    out_lines = list(lines)
    added = 0
    for (index, start, end, body), tokens in zip(plan, tokens_per_line):
        tokens = clean_tokens(tokens)
        if len(tokens) < 2:
            skipped += 1
            continue

        starts = distribute(tokens, start, end)
        pieces = "".join(f"<{stamp(s)}>{t}" for s, t in zip(starts, tokens))

        # Rebuild the line as [line tags]<word tags and text>.
        #
        # A line may carry several timestamps ("[02:37.49][04:39.23]北西南东"),
        # which LRC uses for a repeated chorus. All of them belong in front of the
        # word tags: leaving the extras in `rest` would splice them into the
        # lyric text, which is what put "[00:35.69]" inside the words for a
        # handful of files.
        match = LINE_TAG.match(out_lines[index].strip())
        head = out_lines[index][: out_lines[index].index(match.group(0))]
        line_tags = ""
        rest = out_lines[index][len(head):]
        while True:
            m = LINE_TAG.match(rest)
            if not m:
                break
            line_tags += m.group(0)
            rest = rest[m.end():]
        # Keep any leading space that followed the timestamp: some files write
        # "[00:40.16] 雨一直下" and dropping it changes the original text.
        leading_ws = ""
        while rest.startswith((" ", "\t")):
            leading_ws += rest[0]
            rest = rest[1:]
        newline = "\n" if out_lines[index].endswith("\n") else ""
        out_lines[index] = f"{head}{line_tags}{leading_ws}{pieces}{newline}"
        added += 1

    return "".join(out_lines), added, skipped


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("files", nargs="+")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--revert", action="store_true")
    args = ap.parse_args()

    for path in args.files:
        try:
            text = open(path, encoding="utf-8").read()
        except UnicodeDecodeError:
            print(f"{path}: not UTF-8, skipped", file=sys.stderr)
            continue

        if args.revert:
            new_text, dropped = revert(text)
            if not args.dry_run:
                open(path, "w", encoding="utf-8").write(new_text)
            print(f"{path}: reverted {dropped} lines")
            continue

        new_text, added, skipped = convert(text)
        if args.dry_run:
            print(f"=== {path}: +{added} word lines, {skipped} skipped ===")
            for line in new_text.splitlines()[:8]:
                print("  " + line)
            continue

        open(path, "w", encoding="utf-8").write(new_text)
        print(f"{path}: +{added} lines, {skipped} skipped")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
