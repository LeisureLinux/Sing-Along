# Tools

Helpers for getting a lyric library onto a phone. These are not part of the
application build; run them by hand.

## `sync-to-phone.py`

Builds a staging tree from a messy music folder and copies it to an Android
device in a layout the player and its folder browser can use.

    python3 tools/sync-to-phone.py                    # -> ~/Music-to-phone
    python3 tools/sync-to-phone.py --dry-run          # preview only
    adb push ~/Music-to-phone/. /sdcard/Music/Sing-Along/

What it does:

- Reads tags, falls back to the existing folder name, then the filename.
- Normalises artist spellings through `ARTIST_ALIASES` and rejects label names
  (`滚石唱片`, `ROCK RECORDS`) that were mistakenly stored as artist.
- Dedupes by content hash. Different recordings of the same title are kept and
  disambiguated with `[3m48s]` rather than ` (2)`.
- Writes `Artist/Album/Title.ext` and moves the sidecar `.lrc` beside it with the
  same stem, which is how players match lyrics.
- Skips non-music: path patterns (lectures) and durations outside 20s–30min.

Re-running is safe: the staging directory is cleared before planning, so the
collision check never sees its own previous output.

## `lrc-a2-word-sync.py`

Upgrades line-level `.lrc` files to enhanced LRC (A2) with word timings:

    [00:29.25]<00:29.25>2002<00:30.76>年<00:31.14>的<00:31.51>第一<00:32.27>场<00:32.65>雪

    python3 tools/lrc-a2-word-sync.py FILE.lrc ...
    python3 tools/lrc-a2-word-sync.py --dry-run FILE.lrc
    python3 tools/lrc-a2-word-sync.py --revert FILE.lrc   # drop the word tags

**Pick the player first.** Word-sync syntax is not portable:

| Player     | Syntax                                 | CJK spacing                              |
| ---------- | -------------------------------------- | ---------------------------------------- |
| Gramophone | `<mm:ss.xx>word` inline (A2)           | preserved exactly                        |
| Fluxio     | same as Gramophone                     | preserved exactly                        |
| Booming    | `<word:ss.ss:ss.ss …>` on its own line | **a space is inserted after every word** |

Booming's parser (`LrcLyricsParser.kt:116`) appends a space to every non-final
word regardless of script, so Chinese lyrics render as `雨 一直 下` instead of
`雨一直下`. Use A2 for Gramophone/Fluxio; there is no format that avoids the
space in Booming.

Segmentation comes from Charabia through `segcli`, not fixed-width slicing:
`是/你/的/甜言/蜜语/改变/季节` instead of `是你的/甜言/蜜语`.

Timing is proportional to token length and is still an estimate — there is no
audio analysis, so held notes and fast runs drift. This is a limitation of the
approach, not a bug to be fixed with different spacing.

### Verifying a conversion

Word tags must reproduce the original text exactly when stripped. Check against
a backup of the pre-conversion files:

    python3 tools/lrc-a2-word-sync.py --revert /tmp/backup/x.lrc   # compare

Watch for two regressions that bit this script:

- Apostrophe contractions vs. detached words: `it's` must join (`["it","'s"]`),
  but `'round` / `'cause` after a space must stay separate, or the visible text
  loses that space.
- Multiple timestamps per line (`[02:37.49][04:39.23]歌词`): every leading tag has
  to be consumed, otherwise the second one ends up inside the lyric text.

## `segcli/`

Small stdin→stdout segmenter used by the script above.

    cd tools/segcli && LINDERA_CACHE=/path/to/cache cargo build --release
    printf '是你的甜言蜜语\n' | ./target/release/segcli
    ["是","你","的","甜言","蜜语"]

Charabia's lindera dictionaries download at build time; `LINDERA_CACHE` pointing
at a populated cache makes the build work offline (see
`src-tauri/ARCHITECTURE.md`).
