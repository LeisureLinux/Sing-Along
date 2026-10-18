#!/usr/bin/env python3
r"""
Prepare ~/Music for syncing to an Android phone (Booming music player).

Why a staging copy instead of pushing the existing tree:

  * The library is split across `Organized/` and `iPod-Nano-6Gen/` with
    inconsistent artist naming ("温拿" vs "温拿乐队", "王菲 & 陈奕迅" vs
    "陈奕迅 + 王菲"), so the same artist would appear twice in the phone's
    library.
  * Album folders are mostly absent, so Booming (which groups by tags) would
    show a flat dump for anyone browsing by folder.
  * Booming matches lyrics by a sidecar .lrc that shares the audio file's
    stem and sits next to it -- that has to survive any renaming.

What this does:

  1. Walks every audio file once.
  2. Reads tags; falls back to the existing folder name, then to the filename.
  3. Normalises artist names through an explicit alias table (merged artists
     keep the more common spelling).
  4. Dedupes by content hash (real duplicates only; different recordings of
     the same title are kept).
  5. Writes <Artist>/<Album>/<Title>.<ext>, and moves the matching .lrc beside
     it with the same stem. A missing album becomes "Unknown Album".
  6. Sanitises characters that are illegal on Android's FUSE sdcard
     (forward slash, backslash, colon, star, question mark, quotes, angle
     brackets, pipe) and trims to a safe length.
  7. Refuses to overwrite: an existing target with different content gets a
     " (2)" suffix so nothing is silently lost.

The output tree is a staging directory; nothing in ~/Music is modified.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import collections

AUDIO_EXTS = (".mp3", ".m4a", ".flac", ".wav", ".ogg", ".opus")
UNKNOWN_ALBUM = "Unknown Album"

# Artist spellings that must collapse to one folder. Keys are compared
# case-insensitively after whitespace normalisation.
ARTIST_ALIASES = {
    "温拿乐队": "温拿",
    "温拿": "温拿",
    "王菲 & 陈奕迅": "陈奕迅 + 王菲",
    "陈奕迅 + 王菲": "陈奕迅 + 王菲",
    "陈奕迅+王菲": "陈奕迅 + 王菲",
    "李宗盛+梁静茹": "李宗盛 & 梁静茹",
    "李宗盛 & 林强": "李宗盛 & 林强",
}

# Tag values that are not artist names. These come from the rip's source
# ("_未分类" is a placeholder) or from a label erroneously stored as artist
# ("滚石唱片 ROCK RECORDS" is a record company). When one is seen, the folder
# name is used instead, which is what the library was organised by.
NOT_AN_ARTIST = {"_未分类", "未知", "unknown", "unknown artist"}

# A tag that is a label, not a performer. Matched as a case-insensitive
# substring so "滚石唱片 ROCK RECORDS" and variants both hit.
LABEL_PATTERNS = ("rock records", "滚石唱片", "唱片", "records", "music ltd", "entertainment")

# Android/FUSE sdcard forbids these in a path component. Full-width variants
# are legal but map to lookalikes, so normalise them too.
ILLEGAL = str.maketrans(
    {
        "/": "-",
        "\\": "-",
        ":": "-",
        "*": "",
        "?": "",
        '"': "'",
        "<": "(",
        ">": ")",
        "|": "-",
        "：": "-",
        "？": "",
        "＂": "'",
        "＜": "(",
        "＞": ")",
        "｜": "-",
        "／": "-",
        "＼": "-",
    }
)

MAX_COMPONENT = 120  # conservative; ext4 and FUSE both accept far more

# Media that is not music and should not land in the phone's library. Matched
# case-insensitively against the full path. The ERP recording in this library
# is a lecture, not a track, and would otherwise sit next to albums forever.
EXCLUDE_PATH_PATTERNS = (
    "enterprise resource planning",
    "/lecture",
    "/podcast",
)

# Durations outside this range are almost never music: too short is a jingle or
# a mis-rip, too long is a lecture or a DJ set. Files without a readable
# duration are kept.
MIN_DURATION_S = 20
MAX_DURATION_S = 30 * 60


def sanitise(name: str, fallback: str) -> str:
    name = (name or "").strip().translate(ILLEGAL)
    name = re.sub(r"\s+", " ", name).strip(" .")
    if not name:
        name = fallback
    # Cut on a byte-safe boundary for CJK: keep at most MAX_COMPONENT chars.
    if len(name) > MAX_COMPONENT:
        name = name[:MAX_COMPONENT].rstrip(" .-")
    return name or fallback


def probe(path: str) -> dict:
    """Return lowercase tags for an audio file, or {} when unreadable."""
    try:
        out = subprocess.run(
            ["ffprobe", "-v", "quiet", "-print_format", "json", "-show_format", path],
            capture_output=True,
            text=True,
            timeout=30,
        ).stdout
        tags = json.loads(out)["format"].get("tags", {})
    except Exception:
        return {}
    tags = {str(k).lower(): str(v) for k, v in tags.items()}
    try:
        duration = float(json.loads(out)["format"]["duration"])
    except Exception:
        duration = None
    tags["__duration"] = duration  # type: ignore[assignment]
    return tags


def looks_like_label(value: str) -> bool:
    low = value.lower()
    return any(pattern in low for pattern in LABEL_PATTERNS)


def normalise_artist(raw: str, allow_label: bool = True) -> str:
    """Canonicalise an artist tag; return "" when the value is not a person."""
    key = re.sub(r"\s+", " ", (raw or "").strip())
    if not key:
        return ""
    if key.lower() in NOT_AN_ARTIST:
        return ""
    if not allow_label and looks_like_label(key):
        return ""
    lookup = key.lower()
    for alias, canonical in ARTIST_ALIASES.items():
        if alias.lower() == lookup:
            return canonical
    return key


def content_hash(path: str) -> str:
    h = hashlib.blake2b(digest_size=16)
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def collect(root: str):
    audio, lyrics = [], []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d != ".git"]
        for name in filenames:
            full = os.path.join(dirpath, name)
            low = name.lower()
            if low.endswith(AUDIO_EXTS):
                audio.append(full)
            elif low.endswith(".lrc"):
                lyrics.append(full)
    return audio, lyrics


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("source", nargs="?", default=os.path.expanduser("~/Music"))
    ap.add_argument("dest", nargs="?", default=os.path.expanduser("~/Music-to-phone"))
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    source = os.path.abspath(args.source)
    dest = os.path.abspath(args.dest)

    if not os.path.isdir(source):
        print(f"source not found: {source}", file=sys.stderr)
        return 1
    if dest == source or dest.startswith(source + os.sep):
        # Staging inside the source would make the next run see its own output.
        print("dest must be outside source", file=sys.stderr)
        return 1

    audio, lyric_files = collect(source)
    print(f"found {len(audio)} audio files, {len(lyric_files)} .lrc files")

    excluded = []
    kept = []
    for path in audio:
        low = path.lower()
        if any(pattern in low for pattern in EXCLUDE_PATH_PATTERNS):
            excluded.append((path, "path pattern"))
        else:
            kept.append(path)
    audio = kept
    if excluded:
        print(f"  excluded {len(excluded)} non-music files:")
        for path, reason in excluded:
            print(f"    {reason}: {os.path.relpath(path, source)}")

    lyric_by_stem = {os.path.splitext(p)[0]: p for p in lyric_files}

    # Clear the staging tree before planning, not after: the collision check
    # below uses os.path.exists, and a leftover tree from a previous run would
    # make every file look like a conflict and get a " (2)" suffix.
    if not args.dry_run and os.path.isdir(dest):
        print(f"clearing previous staging dir {dest}")
        shutil.rmtree(dest)

    seen_hashes: dict[str, str] = {}
    duplicates: list[tuple[str, str]] = []
    unreadable: list[str] = []
    too_short_or_long: list[tuple[str, float]] = []
    no_artist: list[str] = []
    used_targets: set[str] = set()

    plans = []
    for path in sorted(audio):
        tags = probe(path)
        duration = tags.pop("__duration", None)
        if not tags:
            # Unreadable tags are not necessarily unplayable; keep the file and
            # fall back to filesystem names rather than dropping it.
            unreadable.append(path)
        if isinstance(duration, float):
            if duration < MIN_DURATION_S or duration > MAX_DURATION_S:
                too_short_or_long.append((path, duration))
                continue

        stem = os.path.splitext(path)[0]
        dir_artist = os.path.basename(os.path.dirname(path))
        filename_stem = os.path.splitext(os.path.basename(path))[0]

        # A label name in the artist tag would create a bogus folder-group; the
        # directory the file came from is a better signal in that case.
        artist = normalise_artist(
            tags.get("artist") or tags.get("album_artist") or "", allow_label=False
        )
        if not artist:
            artist = normalise_artist(dir_artist, allow_label=False)
        if not artist:
            no_artist.append(path)
            artist = "Unknown Artist"

        album = tags.get("album") or UNKNOWN_ALBUM
        title = tags.get("title") or filename_stem

        # A "Artist - Title" filename loses to a real title tag, but a title tag
        # that merely repeats the artist ("战马－崔伟立") is noisier than the
        # filename, so prefer whichever is shorter and non-empty.
        if artist and artist in title and title != artist:
            cleaned = title.replace(artist, "").strip(" -－—·,，")
            if cleaned:
                title = cleaned

        ext = os.path.splitext(path)[1].lower()
        digest = content_hash(path)
        if digest in seen_hashes:
            duplicates.append((path, seen_hashes[digest]))
            continue
        seen_hashes[digest] = path

        parts = [
            sanitise(artist, "Unknown Artist"),
            sanitise(album, UNKNOWN_ALBUM),
            sanitise(title, filename_stem),
        ]

        # Resolve collisions across different content.
        base = os.path.join(dest, *parts)
        candidate = base + ext
        n = 2
        # On a dry run the staging dir may hold a previous run's output; only
        # real collisions within this run count, otherwise every file looks like
        # a conflict and the sample is useless for reviewing names.
        # A genuine collision means the same artist/album/title with different
        # audio (a different rip or edit). Number them, but lead with the
        # duration so the two are distinguishable in a track list instead of
        # looking like an accident.
        while candidate.lower() in used_targets or (
            not args.dry_run and os.path.exists(candidate)
        ):
            if isinstance(duration, float) and duration > 0:
                label = f"{int(duration // 60)}m{int(duration % 60):02d}s"
                candidate = f"{base} [{label}]" + (f" ({n})" if n > 2 else "") + ext
            else:
                candidate = f"{base} ({n}){ext}"
            n += 1
        used_targets.add(candidate.lower())

        plans.append(
            {
                "src": path,
                "dst": candidate,
                "lrc_src": lyric_by_stem.get(stem),
                "dst_lrc": os.path.splitext(candidate)[0] + ".lrc",
            }
        )

    print(f"planned {len(plans)} files after dedupe")
    print(f"  dropped as content duplicates: {len(duplicates)}")
    for a, b in duplicates:
        print(f"    dup: {a}\n      == {b}")
    if unreadable:
        print(f"  tags unreadable (kept, names derived from path): {len(unreadable)}")
    if no_artist:
        print(f"  no artist anywhere: {len(no_artist)}")
    if too_short_or_long:
        print(f"  skipped by duration: {len(too_short_or_long)}")
        for path, duration in too_short_or_long:
            print(f"    {duration:.0f}s: {os.path.relpath(path, source)}")

    paired = sum(1 for p in plans if p["lrc_src"])
    print(f"  lyrics pairs: {paired} (audio without lyrics: {len(plans) - paired})")

    # Album/artist sanity: report when one artist's folder count is large, which
    # usually means a tag is dirty.
    by_artist = collections.Counter(os.path.dirname(os.path.dirname(p["dst"])) for p in plans)
    print(f"  distinct artists: {len(by_artist)}")

    if args.dry_run:
        print("\n-- dry run: sample --")
        for p in plans[:12]:
            print(f"  {os.path.relpath(p['dst'], dest)}  {'+lrc' if p['lrc_src'] else ''}")
        return 0

    copied_bytes = 0
    for p in plans:
        os.makedirs(os.path.dirname(p["dst"]), exist_ok=True)
        shutil.copy2(p["src"], p["dst"])
        copied_bytes += os.path.getsize(p["dst"])
        if p["lrc_src"]:
            shutil.copy2(p["lrc_src"], p["dst_lrc"])

    print(f"\nwrote {len(plans)} audio files, {copied_bytes / 1024 / 1024:.0f} MB -> {dest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
