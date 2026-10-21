---
name: sing-along
description: Manage synced lyrics for a local music library with the Sing-Along CLI — fetch missing lyrics, convert Chinese script, and export in the format a specific phone app can read (TTML for Steve on iOS, enhanced LRC/A2 for Gramophone or Fluxio on Android). Use when asked to add lyrics to music files, fix or complete a lyric library, or prepare lyrics so they display word-by-word on a particular player.
---

# Sing-Along

One binary, two modes: `sing-along` opens the GUI, `sing-along <command>` runs
headless. Everything below is the headless mode.

## When to use this

- "补充/下载歌词" for a folder of music
- "导出适合 XX 播放器的歌词格式"
- Converting a library between lyric formats
- Checking what a library is missing before deciding what to do

## Check the tool is there

```bash
sing-along --help        # prints usage; exits non-zero if the CLI is missing
sing-along formats       # lists export targets
```

If `sing-along` launches a window instead of printing help, the installed
version predates the CLI. Install 2.9.0 or newer.

## The three questions to settle first

**1. Which directory?** Ask if not given. Libraries are usually
`~/Music`, `~/Music/<artist>`, or an external drive.

**2. Which script?** Only for Chinese lyrics. `--script simplified` or
`--script traditional`.

Decide by inspecting the files, not by guessing: read one `.lrc` and look for
Traditional-only characters (們, 這, 說, 麼) or Simplified-only ones (们, 这, 说,
么). A library that is already in the user's script needs no flag.

Never convert a non-Chinese library — the flag is off by default and should stay
off. Conversion is lossy (发 → 發 or 髮 depending on context), so it is only
worth doing when the lyrics are actually in the other script.

**3. Which player?** This decides the export format and it is not optional —
word-level lyric formats are not portable.

| Player | `--for` | Format | Notes |
| ------ | ------- | ------ | ----- |
| [Steve](https://apps.apple.com/app/id6751122850) (iOS/iPadOS) | `steve` | TTML | Word highlighting only from TTML. Its LRC reader is line-level and does **not** strip tags — sending A2 shows the timestamps as lyric text. |
| [Gramophone](https://github.com/FoedusProgramme/Gramophone) (Android) | `gramophone` | Enhanced LRC / A2 | Reads the A2 `<mm:ss.xx>` extension. |
| Fluxio (Android) | `fluxio` | Enhanced LRC / A2 | Same as Gramophone. |
| Any line-level player | `lrc` | plain `.lrc` | No word timing. |
| Plain text | `txt` | `.txt` | |

**Booming (Android) has no supported format.** Its parser
(`LrcLyricsParser.kt:116`) appends a space after every non-final word regardless
of script, so Chinese renders as `雨 一直 下`. Tell the user; do not pretend an
export will work.

If the player is not in the table, ask, or export both A2 and TTML — a player
ignores a lyric file whose format it does not understand, and Steve prefers TTML
when both exist for the same track.

## Workflow: fill in a library

```bash
sing-along status /path/to/music
```

Reports each file as one of:

| Label | Meaning |
| ----- | ------- |
| `synced` | Has timed lyrics — nothing to do |
| `plain` | Has lyric text but no timestamps |
| `none` | No lyric file at all |

Use this first so you can tell the user what is actually missing before touching
anything.

```bash
sing-along fetch /path/to/music
```

Downloads and writes `<track>.lrc` beside each audio file whose lyrics are
missing. Options:

- `--overwrite` — re-download even where a `.lrc` already exists
- `--script simplified|traditional` — convert Chinese script
- `--proxy http://host:port` — if the service is unreachable

Files that are already correct are counted as `already had lyrics`, not
re-downloaded. Failures print a reason:

| Output | Meaning | What to do |
| ------ | ------- | ---------- |
| `missing …  (no timed lyrics)` | LRCLIB has the song but only unsynchronised text | Nothing to fix — no synced version exists anywhere. Report it as such; do not retry. |
| `missing …  (NotFound…)` | The service has no record of this track | Suggest checking the title/artist tags, or searching in the GUI where a different match can be picked. |
| `missing …  (error sending request…)` | The service is unreachable | Retry with `--proxy`. The summary line will say so. |

The summary only suggests `--proxy` when lookups actually failed to reach the
service. If it does not mention the proxy, retrying will not help.

Re-run `status` afterwards and report the before/after counts.

## Workflow: export for a phone

```bash
sing-along export /path/to/music --for steve
```

Writes `<track>.ttml` (or `.alrc` / `.lrc` / `.txt`) next to each track that has
lyrics. Use `--dry-run` first when the user wants to see the plan before anything
is written.

If it reports `had no lyrics`, run `fetch` first and export again.

**Verify the output.** A wrongly split token or a word tag left in as text still
produces valid TTML, so the file has to be checked against its source:

```bash
python3 tools/verify-ttml.py /path/to/track.ttml --against /path/to/track.lrc
```

It prints every paragraph's visible text and exits non-zero if it does not match
the source. If it reports a mismatch, stop and report it — do not hand the file
to the user.

One expected difference: the converter **drops credit lines** (`作词：方文山`,
`Produced by …`) because they carry no singing. The script knows this and filters
them from the source side; if you compare by hand, filter them too.

## Workflow: switch formats

```bash
sing-along export /path/to/music --for gramophone   # A2 for Android
sing-along export /path/to/music --for steve        # TTML for iOS
```

Both can coexist: a player ignores the format it does not read, and Steve prefers
TTML when both files exist.

## Rules

- **Never translate, rewrite or "improve" lyric text.** It is someone else's
  work; the only permitted transformation is Chinese script conversion, and only
  when asked.
- **Never delete existing `.lrc` files** without asking. `--overwrite` replaces
  them.
- **Report the numbers** — how many fetched, how many had none, how many failed —
  rather than saying "done".
- **Do not claim a track can be synced when LRCLIB only has plain text.** That is
  the common false expectation here; say plainly that no synced version exists.
- **The GUI is better for ambiguous matches.** `fetch` takes the best hit by
  duration; a person can pick a different result per track in the app's Search
  tab.
