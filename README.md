# Sing-Along

Find and edit synced lyrics for your offline music library.

Sing-Along scans the music files in the folders you choose, looks up matching
lyrics, and saves them alongside your audio files or embeds them in the tags.
It is a desktop player-adjacent tool: you can play a track, watch the lyrics
highlight line by line (or word by word), and correct the timings while you
listen.

Lyrics are retrieved from [LRCLIB](https://lrclib.net). Sing-Along is an
independent project and is **not** an official client of LRCLIB.

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

![01.png](https://raw.githubusercontent.com/tranxuanthang/lrcget/9e0578bd9411fcc024a56d2f1108701751c5ec3a/screenshots/01.png)

<details>
<summary>Click to expand more screenshots</summary>

![02.png](https://raw.githubusercontent.com/tranxuanthang/lrcget/9e0578bd9411fcc024a56d2f1108701751c5ec3a/screenshots/02.png)

![03.png](https://raw.githubusercontent.com/tranxuanthang/lrcget/9e0578bd9411fcc024a56d2f1108701751c5ec3a/screenshots/03.png)

![04.png](https://raw.githubusercontent.com/tranxuanthang/lrcget/9e0578bd9411fcc024a56d2f1108701751c5ec3a/screenshots/04.png)

</details>

## Troubleshooting

**Audio cannot be played in Linux (Ubuntu and other distros)**

Try to install `pipewire-alsa` package. For example, in Ubuntu or Debian-based distros:

```
sudo apt install pipewire-alsa
```

**App won't open in Windows 10/11**

If you are using Windows 10 LTSC, or have tried running some scripts to debloat Windows 10 (which will uninstall Microsoft Edge and its webview component), you might have issues as Sing-Along depends on WebView2. Reinstalling Microsoft Edge might fix the problem (see issue https://github.com/tranxuanthang/lrcget/issues/45).

**Scrollbar is invisible in Linux (KDE Plasma 5/6)**

The exact cause is still unknown, but it can be fixed by going to System Settings > Appearance > Global Theme > Application Style > Configure GNOME/GTK Application Style... > Change to something other than breeze (Awaita or Default) > Apply (see comment https://github.com/tranxuanthang/lrcget/issues/44#issuecomment-1962998268)

## Contact

Email: [albertxu@freelamp.com](mailto:albertxu@freelamp.com)

## Development

Sing-Along is made with [Tauri](https://tauri.app).

To start developing the application, you need to do the [prerequisites](https://tauri.app/v1/guides/getting-started/prerequisites) steps according to your operating system.

For example, you need the following components to start the development in Windows:

- Microsoft Visual Studio C++ Build Tools
- Rust 1.81.0 or higher
- NodeJS v16.18.0 or higher

Start the development window with the following command:

```shell
cd lrcget
npm install
npm run tauri dev
```

## Building

Start the build process with the following command:

```shell
cd lrcget
npm install
npm run tauri build
```

Your built binaries are located at:

```
./src-tauri/target/release/
```

For more detailed instruction, follow the [building guide](https://tauri.app/v1/guides/building/) to build the application according to your OS platform.
