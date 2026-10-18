#!/usr/bin/env python3
r"""
Upload a music library to Steve Player's Wi-Fi import endpoint.

Steve serves a small page on http://<iphone-ip>:8080 with a multipart form:

    POST /upload
    X-Upload-ID: <opaque string>
    X-Upload-File-Count: <number of files in this request>
    form field "tracks" repeated per file

    -> {"ok": true, "processing": ..., "message": ...}

GET /status returns {"progress", "importedCount", "fileCount", "message", "phase"}
while the phone copies the files into its library.

Two things this script has to get right:

1. **Batching.** One request per file would mean ~700 round-trips; all 2 GB in
   one request means no progress reporting and an all-or-nothing failure. Files
   are sent in batches with a per-batch status poll in between.

2. **Keeping .lrc beside its audio.** Each file is sent with a bare basename,
   so a lyrics file and its track must land in the same batch (or at least be
   uploaded with distinct names). Steve matches lyrics by filename stem, so the
   stems are preserved verbatim and never sanitised differently for audio and
   sidecar.

Usage:
    python3 steve-upload.py --host 192.168.68.202 --root ~/Music-to-phone
    python3 steve-upload.py --host ... --dry-run
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
import urllib.error
import urllib.request
import uuid

AUDIO_EXT = (".mp3", ".m4a", ".flac", ".wav", ".ogg", ".opus", ".aac")
LYRIC_EXT = (".lrc", ".ttml", ".dfxp", ".txt")


def collect(root: str) -> list[tuple[str, str]]:
    """Return [(absolute_path, upload_name)] preserving Artist/Album structure.

    The upload endpoint flattens to a basename, so names must already be unique.
    Rather than inventing a scheme, keep the file's own name and rely on the
    library's existing layout: sidecar lyrics share the stem, which is exactly
    what the player needs.
    """
    items: list[tuple[str, str]] = []
    seen: dict[str, int] = {}
    for dirpath, _dirnames, filenames in os.walk(root):
        for name in sorted(filenames):
            lower = name.lower()
            if not lower.endswith(AUDIO_EXT + LYRIC_EXT):
                continue
            full = os.path.join(dirpath, name)
            upload_name = name
            # Same basename can exist under two albums (e.g. "Intro.mp3").
            # Disambiguate with the parent album so nothing is overwritten.
            if upload_name in seen:
                seen[upload_name] += 1
                parent = os.path.basename(dirpath)
                stem, ext = os.path.splitext(name)
                upload_name = f"{stem} [{parent}]{ext}"
            else:
                seen[upload_name] = 1
            items.append((full, upload_name))
    return items


def build_multipart(files: list[tuple[str, str]], boundary: str) -> bytes:
    chunks: list[bytes] = []
    for path, name in files:
        chunks.append(f"--{boundary}\r\n".encode())
        chunks.append(
            f'Content-Disposition: form-data; name="tracks"; filename="{name}"\r\n'.encode()
        )
        chunks.append(b"Content-Type: application/octet-stream\r\n\r\n")
        with open(path, "rb") as fh:
            chunks.append(fh.read())
        chunks.append(b"\r\n")
    chunks.append(f"--{boundary}--\r\n".encode())
    return b"".join(chunks)


def post_batch(host: str, files: list[tuple[str, str]], timeout: int) -> dict:
    boundary = f"----SteveBoundary{uuid.uuid4().hex}"
    body = build_multipart(files, boundary)
    req = urllib.request.Request(
        f"http://{host}/upload",
        data=body,
        method="POST",
        headers={
            "Content-Type": f"multipart/form-data; boundary={boundary}",
            "Content-Length": str(len(body)),
            "X-Upload-ID": uuid.uuid4().hex,
            "X-Upload-File-Count": str(len(files)),
        },
    )
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        payload = resp.read().decode("utf-8", "replace")
    try:
        return json.loads(payload)
    except json.JSONDecodeError:
        return {"raw": payload}


def get_status(host: str, timeout: int = 15) -> dict:
    try:
        with urllib.request.urlopen(f"http://{host}/status", timeout=timeout) as resp:
            return json.loads(resp.read().decode("utf-8", "replace"))
    except Exception as exc:  # noqa: BLE001 - status is advisory only
        return {"error": str(exc)}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--host", default="192.168.68.202:8080")
    ap.add_argument("--root", default=os.path.expanduser("~/Music-to-phone"))
    ap.add_argument("--batch-files", type=int, default=20, help="files per request")
    ap.add_argument("--batch-mb", type=float, default=120.0, help="size cap per request")
    ap.add_argument("--timeout", type=int, default=600)
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    items = collect(args.root)
    audio = sum(1 for _p, n in items if n.lower().endswith(AUDIO_EXT))
    lyrics = len(items) - audio
    total_bytes = sum(os.path.getsize(p) for p, _n in items)
    print(f"{audio} audio, {lyrics} lyrics, {total_bytes / 1024 / 1024:.0f} MB")

    # Keep an audio file and its sidecar in the same batch: group by stem first.
    by_stem: dict[str, list[tuple[str, str]]] = {}
    for path, name in items:
        stem = os.path.splitext(name)[0].lower()
        by_stem.setdefault(stem, []).append((path, name))

    units = list(by_stem.values())
    batches: list[list[tuple[str, str]]] = []
    current: list[tuple[str, str]] = []
    current_bytes = 0
    for unit in units:
        unit_bytes = sum(os.path.getsize(p) for p, _n in unit)
        if current and (
            len(current) + len(unit) > args.batch_files
            or current_bytes + unit_bytes > args.batch_mb * 1024 * 1024
        ):
            batches.append(current)
            current, current_bytes = [], 0
        current.extend(unit)
        current_bytes += unit_bytes
    if current:
        batches.append(current)

    print(f"{len(batches)} batches")
    if args.dry_run:
        for i, batch in enumerate(batches[:3], 1):
            size = sum(os.path.getsize(p) for p, _n in batch) / 1024 / 1024
            print(f"  batch {i}: {len(batch)} files, {size:.0f} MB")
            for _p, n in batch[:4]:
                print(f"     {n}")
        return 0

    sent_files = 0
    sent_bytes = 0
    started = time.time()
    for index, batch in enumerate(batches, 1):
        size = sum(os.path.getsize(p) for p, _n in batch)
        print(
            f"[{index}/{len(batches)}] {len(batch)} files, {size / 1024 / 1024:.0f} MB ... ",
            end="",
            flush=True,
        )
        try:
            result = post_batch(args.host, batch, args.timeout)
        except urllib.error.HTTPError as exc:
            print(f"HTTP {exc.code}: {exc.read()[:200]!r}")
            return 1
        except Exception as exc:  # noqa: BLE001
            print(f"failed: {exc}")
            return 1

        sent_files += len(batch)
        sent_bytes += size
        elapsed = time.time() - started
        rate = sent_bytes / 1024 / 1024 / max(elapsed, 1e-6)
        print(f"ok ({rate:.1f} MB/s avg)")

        # Give the phone a moment to ingest before the next request.
        status = get_status(args.host)
        if "error" not in status:
            print(
                "     device: "
                f"phase={status.get('phase')} "
                f"imported={status.get('importedCount')} "
                f"progress={status.get('progress')}"
            )

    print(f"\nsent {sent_files} files, {sent_bytes / 1024 / 1024:.0f} MB in {time.time() - started:.0f}s")
    print("final status:", get_status(args.host))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
