# Sing-Along

**A small desktop tool for finding, editing and converting synced lyrics for
your own music files.** Built for people who keep a local library and want
their lyrics to work everywhere — including on phones, in cars.

Sing-Along scans the music folders you choose, looks up matching lyrics, saves
them next to your audio files or embeds them in the tags, and lets you correct
the timings while you listen. It is a desktop companion, not a full music
player: pick a track, watch the lyrics follow along, fix what is off.

Lyrics are retrieved from [LRCLIB](https://lrclib.net).

## Independent project

**Sing-Along is not tracking upstream.** It began as a fork of
[LRCGET](https://github.com/tranxuanthang/lrcget) and has since gone its own
way — there is no upstream remote, no pull from the original repository, and no
intent to push changes back. Bug reports and features are decided here.

That also means a few things you should not expect:

- **No parity with LRCGET.** Upstream may add features this fork never gets, and
  the other way round.
- **A different scope.** LRCGET is a mass-downloading utility. Sing-Along is a
  small personal tool that also edits and converts — see below.
- **Upstream issues do not apply here.** If you hit something, report it in this
  repository.

The MIT license and the original copyright notice are preserved regardless; see
[License and attribution](#license-and-attribution).

## What it is good at

- **Mass-fetching lyrics** for a large local library from LRCLIB, with
  configurable skip rules so it does not overwrite what you already fixed.
- **Editing timings by ear.** Play a track and correct line-by-line, or
  word-by-word, instead of hand-editing text files.
- **Multilingual text, including CJK.** Word-level segmentation uses
  [Charabia](https://github.com/meilisearch/charabia), so Chinese and Japanese
  lines break at real word boundaries rather than at a fixed character count.
  Chinese titles, artist names and file paths are handled throughout.
- **Exporting to what phones actually read.** One library can be written out as
  enhanced LRC (A2) or TTML, so the same lyrics work on Android players and on
  iOS players that only understand TTML.

## Why word-level lyrics need converting

Word-level lyric files are not portable. There are at least eight dialects and
only one of them (TTML) is an actual standard:

| Format                | Line timing | Word timing | Defined by                                                  |
| --------------------- | ----------- | ----------- | ----------------------------------------------------------- |
| LRC                   | ✅          | ❌          | a 1998 Winamp plug-in, never standardised                   |
| LRC A2 (Enhanced LRC) | ✅          | ✅          | the A2 Media Player author                                  |
| SPL                   | ✅          | ✅          | the Salt Player author                                      |
| YRC / QRC / KRC       | ✅          | ✅          | NetEase / QQ Music / Kugou (the last two are **encrypted**) |
| TTML                  | ✅          | ✅          | W3C (started as DFXP in 2003)                               |

So the same library has to be exported twice if you use two players: enhanced
LRC for players that read the A2 extension (Gramophone, Fluxio), and TTML for
players whose word highlighting comes only from TTML (Steve on iOS).

Background, with sources, on how these formats came to exist, who owns each
one, why two of them are encrypted, and why free offline players with
word-level lyrics are so scarce:

**📖 [从「能免费在车里唱歌」出发，我为什么会撞上三种歌词格式](https://freelamp.com/articles/2026-10-18_lrc-ttml-karaoke-formats/)** (Chinese)

The conversion helpers used for this live in [`tools/`](tools/README.md):
`lrc-a2-word-sync.py` for enhanced LRC, `lrc-to-ttml.py` for TTML.

## License and attribution

Sing-Along is a fork of [LRCGET](https://github.com/tranxuanthang/lrcget),
Copyright (c) 2023 tranxuanthang, used under the MIT License. The original
copyright notice and the MIT permission notice are retained in
[LICENSE](LICENSE); see [NOTICE](NOTICE) for full attribution and a list of
what this fork changed. The About dialog in the app carries the same credit.

## Screenshots

![01.png](screenshots/01.png)

<details>
<summary>Click to expand more screenshots</summary>

![02.png](screenshots/02.png)

![03.png](screenshots/03.png)

![04.png](screenshots/04.png)

</details>

## Troubleshooting

**Audio cannot be played in Linux (Ubuntu and other distros)**

Try to install `pipewire-alsa` package. For example, in Ubuntu or Debian-based distros:

```
sudo apt install pipewire-alsa
```

**App won't open in Windows 10/11**

If you are using Windows 10 LTSC, or have tried running some scripts to debloat Windows 10 (which will uninstall Microsoft Edge and its webview component), you might have issues as Sing-Along depends on WebView2. Reinstalling Microsoft Edge usually fixes it — WebView2 is a Windows component this app cannot ship itself.

**Scrollbar is invisible in Linux (KDE Plasma 5/6)**

The exact cause is still unknown, but it can be fixed by going to System Settings > Appearance > Global Theme > Application Style > Configure GNOME/GTK Application Style... > Change to something other than breeze (Adwaita or Default) > Apply.

## Contact

Email: [albertxu@freelamp.com](mailto:albertxu@freelamp.com)

## Development

Sing-Along is made with [Tauri](https://tauri.app) v2 (Rust backend, Vue 3
frontend).

Install the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for
your OS. In short you need:

- Rust 1.81.0 or newer
- Node.js 18 or newer
- The platform build tools (on Windows: Visual Studio C++ Build Tools and
  WebView2; on Linux: `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libasound2-dev`
  and `build-essential`)

Two architecture notes are worth reading before changing code:
[`src/ARCHITECTURE.md`](src/ARCHITECTURE.md) for the frontend and
[`src-tauri/ARCHITECTURE.md`](src-tauri/ARCHITECTURE.md) for the backend.

Start the development window with:

```shell
npm install
npm run tauri:dev
```

`npm run tauri:dev` passes `--features dev,devtools`. Plain `tauri dev` only
passes `--no-default-features`, so a debug build started that way has neither
the devtools panel nor the WebDriver bridge — this trips people up.

**Build note:** Charabia's Japanese and Korean dictionaries are downloaded at
build time by its build scripts. If that fails (network, TLS), point
`LINDERA_CACHE` at a populated cache directory and the scripts take a fast path
instead of downloading.

## Building

```shell
npm install
npm run tauri build
```

Release builds carry no devtools and no WebDriver server: both are behind
opt-in cargo features that `tauri build` does not enable. Verify with
`cargo tree -i tauri-plugin-webdriver`, which cannot resolve the package at all
in the default feature set.

Your built binaries are located at:

```
./src-tauri/target/release/
```

For more detailed instruction, follow the [building guide](https://tauri.app/v1/guides/building/) to build the application according to your OS platform.
