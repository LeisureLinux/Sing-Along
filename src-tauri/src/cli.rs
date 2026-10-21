//! Command-line interface.
//!
//! The same binary serves both roles: `sing-along` with no arguments starts the
//! GUI, `sing-along <command>` runs headless. This is what `code` and `vim` do,
//! and it means a shell script — or an agent following the skill in `skills/` —
//! drives the same code the window does.
//!
//! Deliberately not a full mirror of the GUI. Three commands cover what needs
//! scripting: fetch lyrics for a directory, export what is already there in a
//! specific player's format, and report what a directory currently holds.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};

use crate::export::{self, ExportFormat, ExportStatus};
use crate::lyricsfile;
use crate::persistent_entities::PersistentTrack;
use crate::scanner::metadata::TrackMetadata;

const USAGE: &str = "\
sing-along — lyric sync for your offline musics

USAGE:
    sing-along                       Open the player
    sing-along fetch <path>...       Download missing lyrics
    sing-along export <path>...      Write lyrics in a player's format
    sing-along status <path>...      Report what lyrics a path already has
    sing-along formats               List supported export targets
    sing-along --version             Print the version
    sing-along --help                This text

FETCH OPTIONS:
    --script <off|simplified|traditional>   Convert Chinese lyrics (default: off)
    --overwrite                             Re-download where lyrics exist
    --proxy <url>                           Proxy for the lyric services

EXPORT OPTIONS:
    --for <player>                  Target player; see `sing-along formats`
    --format <a2|lrc|ttml|txt>      Explicit format instead of --for
    --dry-run                       Show what would be written

EXAMPLES:
    sing-along fetch ~/Music
    sing-along fetch ~/Music --script simplified --overwrite
    sing-along export ~/Music --for gramophone
    sing-along export ~/Music --for steve --dry-run
    sing-along status ~/Music
";

/// Player name → the format that player actually reads.
///
/// Word-level formats are not portable: Steve (iOS) only highlights words from
/// TTML, while Gramophone and Fluxio read the A2 extension of LRC. Sending one to
/// the other shows the timestamps as lyric text. Booming is absent on purpose —
/// no file format avoids its forced space between CJK words.
const PLAYERS: &[(&str, ExportFormat, &str)] = &[
    ("steve", ExportFormat::Ttml, "Steve (iOS/iPadOS)"),
    ("apple", ExportFormat::Ttml, "Steve (iOS/iPadOS)"),
    ("ios", ExportFormat::Ttml, "Steve (iOS/iPadOS)"),
    ("gramophone", ExportFormat::LrcA2, "Gramophone (Android)"),
    ("fluxio", ExportFormat::LrcA2, "Fluxio (Android)"),
    ("android", ExportFormat::LrcA2, "Gramophone / Fluxio (Android)"),
    ("lrc", ExportFormat::Lrc, "Line-level LRC, any LRC reader"),
    ("txt", ExportFormat::Txt, "Plain text"),
];

/// Entry point for a standalone CLI binary.
///
/// Builds its own runtime: the CLI binary has no `#[tokio::main]`, and the GUI
/// (which already runs one) calls `run` from inside it instead.
pub fn run_blocking(args: &[String]) -> Option<i32> {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("could not start the async runtime: {err}");
            return Some(1);
        }
    };
    runtime.block_on(run(args))
}

/// Entry point. Returns `None` when the GUI should start instead.
///
/// Async because the fetch path has to await the lyric service.
pub async fn run(args: &[String]) -> Option<i32> {
    let Some(first) = args.get(1) else {
        return None;
    };

    let code = match first.as_str() {
        "--help" | "-h" | "help" => {
            print!("{USAGE}");
            0
        }
        // Plain `sing-along --version` output, so a script can read it without
        // parsing the usage text. Same shape as `--version` from other tools:
        // the program name, a space, the version.
        "--version" | "-V" | "version" => {
            // Program name from argv[0], so the GUI binary and the standalone
            // CLI each report the name they were actually invoked as.
            let name = args
                .first()
                .and_then(|arg| std::path::Path::new(arg).file_stem())
                .and_then(|stem| stem.to_str())
                .unwrap_or("sing-along");
            println!("{name} {}", env!("CARGO_PKG_VERSION"));
            0
        }
        "formats" => {
            print_formats();
            0
        }
        "fetch" => report(fetch(&args[2..]).await),
        "export" => report(export_command(&args[2..])),
        "status" => report(status(&args[2..])),
        // An unknown first word is more likely a mistyped command than a wish to
        // open a window, so say so instead of launching the GUI.
        other => {
            eprintln!("Unknown command: {other}\n\n{USAGE}");
            2
        }
    };

    Some(code)
}

fn report(result: Result<()>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("error: {err:#}");
            1
        }
    }
}

fn print_formats() {
    println!("Supported export targets:\n");
    for (name, _, description) in PLAYERS {
        println!("  {name:<12} {description}");
    }
    println!(
        "\nBooming (Android) is not listed: its parser inserts a space after every\n\
         word, so CJK lyrics render as \"雨 一直 下\". No file format avoids that."
    );
}

/// Split argv into positional paths and `--flag value` pairs.
fn parse_args(args: &[String]) -> (Vec<String>, HashMap<String, String>) {
    const VALUED: [&str; 4] = ["script", "for", "format", "proxy"];

    let mut paths = Vec::new();
    let mut flags = HashMap::new();
    let mut index = 0;

    while index < args.len() {
        let arg = &args[index];
        let Some(name) = arg.strip_prefix("--") else {
            paths.push(arg.clone());
            index += 1;
            continue;
        };

        match name.split_once('=') {
            Some((name, value)) => {
                flags.insert(name.to_owned(), value.to_owned());
            }
            None if VALUED.contains(&name) && index + 1 < args.len() => {
                flags.insert(name.to_owned(), args[index + 1].clone());
                index += 1;
            }
            None => {
                flags.insert(name.to_owned(), String::new());
            }
        }
        index += 1;
    }

    (paths, flags)
}

/// Every audio file under the given paths, in a stable order.
fn collect_tracks(paths: &[String]) -> Result<Vec<TrackMetadata>> {
    if paths.is_empty() {
        bail!("give at least one file or directory");
    }

    let mut files = Vec::new();
    for raw in paths {
        let path = Path::new(raw);
        if path.is_file() {
            files.push(path.to_path_buf());
        } else if path.is_dir() {
            let mut found = Vec::new();
            collect_dir(path, &mut found)?;
            found.sort();
            files.extend(found);
        } else {
            bail!("no such file or directory: {raw}");
        }
    }

    if files.is_empty() {
        bail!("no audio files found");
    }

    let mut tracks = Vec::new();
    for file in files {
        match TrackMetadata::from_path(&file) {
            Ok(metadata) => tracks.push(metadata),
            // One unreadable tag should not abort a library-wide run.
            Err(err) => eprintln!("skipping {}: {err}", file.display()),
        }
    }

    if tracks.is_empty() {
        bail!("no readable audio files found");
    }
    Ok(tracks)
}

fn collect_dir(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry?.path();
        if path.is_dir() {
            collect_dir(&path, out)?;
        } else if is_audio(&path) {
            out.push(path);
        }
    }
    Ok(())
}

fn is_audio(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("mp3" | "flac" | "m4a" | "mp4" | "ogg" | "opus" | "wav" | "aiff" | "wv")
    )
}

/// Existing lyric sidecar beside a track: `.lrc` first, then `.txt`.
fn sidecar_for(track_path: &Path) -> Option<PathBuf> {
    ["lrc", "txt"]
        .iter()
        .map(|ext| track_path.with_extension(ext))
        .find(|candidate| candidate.exists())
}

/// The GUI stores this in its database; the CLI has no database, so it reads the
/// same setting from the environment and lets `--proxy` override it.
fn proxy_for(flags: &HashMap<String, String>) -> String {
    flags
        .get("proxy")
        .cloned()
        .or_else(|| std::env::var("SING_ALONG_PROXY").ok())
        .unwrap_or_default()
}

fn user_agent() -> String {
    format!(
        "Sing-Along v{} (https://github.com/LeisureLinux/Sing-Along)",
        env!("CARGO_PKG_VERSION")
    )
}

/// `sing-along fetch <path>...`
async fn fetch(args: &[String]) -> Result<()> {
    let (paths, flags) = parse_args(args);

    let target = match flags.get("script").map(|s| s.as_str()).unwrap_or("off") {
        "off" | "" => None,
        other => Some(
            crate::script_convert::ScriptTarget::parse(other)
                .ok_or_else(|| anyhow!("unknown --script value: {other}"))?,
        ),
    };

    let overwrite = flags.contains_key("overwrite");
    let proxy = proxy_for(&flags);
    let instance = std::env::var("SING_ALONG_LRCLIB").unwrap_or_else(|_| "https://lrclib.net".to_owned());

    // `main` is already `#[tokio::main]`, so enter the existing runtime rather
    // than building a second one — nesting them panics.
    let tracks = collect_tracks(&paths)?;
    let mut fetched = 0usize;
    let mut skipped = 0usize;
    let mut missing = 0usize;
    let mut transport_failures = 0usize;

    for track in &tracks {
        let path = Path::new(&track.file_path);

        if !overwrite && sidecar_for(path).is_some() {
            skipped += 1;
            continue;
        }

        // Search first, then pick the best hit.
        //
        // `/api/get` is an exact match on title + album + artist + duration, and
        // in a real library any of those can differ from what LRCLIB holds — the
        // album tag is often a folder name. `/api/search` with the title and
        // artist returns candidates, and the closest by duration is the same
        // choice a person would make.
        let hit = match find_lyrics(&track.title, &track.artist, track.duration, &instance, &proxy).await
        {
            Ok(Some(hit)) => hit,
            Ok(None) => {
                missing += 1;
                println!("missing  {}  (no match)", track.file_path);
                continue;
            }
            Err(err) => {
                missing += 1;
                // Distinguish "the service is unreachable" from "no match": a
                // bare count sends people to the wrong fix.
                if err.to_string().contains("request") || err.to_string().contains("connect") {
                    transport_failures += 1;
                }
                println!("missing  {}  ({err})", track.file_path);
                continue;
            }
        };

        let resolved = match crate::cli_support::resolve_lrclib_lyrics_payload(hit) {
            Ok(resolved) => resolved,
            Err(err) => {
                missing += 1;
                println!("missing  {}  ({err})", track.file_path);
                continue;
            }
        };

        // Work from the resolved lyrics directly. Rebuilding a lyricsfile and
        // parsing it again only adds a way for the text to go missing between
        // the two steps.
        let mut synced = resolved.synced_lyrics.clone();
        if synced.trim().is_empty() {
            missing += 1;
            println!("missing  {}  (no timed lyrics)", track.file_path);
            continue;
        }

        if let Some(target) = target {
            if crate::script_convert::has_cjk(&synced) {
                synced = crate::script_convert::convert(&synced, target);
            }
        }

        std::fs::write(path.with_extension("lrc"), &synced)
            .with_context(|| format!("writing lyrics beside {}", path.display()))?;
        fetched += 1;
        println!("fetched  {}", track.file_path);
    }

    println!("\n{fetched} fetched, {skipped} already had lyrics, {missing} not found");
    // Only suggest the proxy when the failures looked like network errors. A
    // run where every miss was "no timed lyrics" is not a connectivity problem,
    // and pointing at --proxy sends the user down the wrong path.
    if fetched == 0 && transport_failures > 0 {
        println!(
            "\nSome lookups could not reach the service. If this machine needs a proxy,\n\
             retry with `sing-along fetch <path> --proxy http://your-proxy:port`."
        );
    }
    Ok(())
}

/// `sing-along export <path>...`
fn export_command(args: &[String]) -> Result<()> {
    let (paths, flags) = parse_args(args);

    let format = match (flags.get("for"), flags.get("format")) {
        (Some(player), _) => PLAYERS
            .iter()
            .find(|(name, _, _)| name.eq_ignore_ascii_case(player))
            .map(|(_, format, _)| *format)
            .with_context(|| {
                format!("unknown player: {player}\nRun `sing-along formats` for the list.")
            })?,
        (None, Some(name)) => match name.as_str() {
            "a2" => ExportFormat::LrcA2,
            "lrc" => ExportFormat::Lrc,
            "ttml" => ExportFormat::Ttml,
            "txt" => ExportFormat::Txt,
            other => bail!("unknown --format: {other}"),
        },
        (None, None) => bail!("give --for <player> or --format <format>"),
    };

    let dry_run = flags.contains_key("dry-run");
    let tracks = collect_tracks(&paths)?;

    let mut written = 0usize;
    let mut no_lyrics = 0usize;
    let mut failed = 0usize;

    for track in &tracks {
        let path = Path::new(&track.file_path);
        let Some(sidecar) = sidecar_for(path) else {
            no_lyrics += 1;
            continue;
        };

        let raw = std::fs::read_to_string(&sidecar)
            .with_context(|| format!("reading {}", sidecar.display()))?;
        let parsed = parse_sidecar(&raw)?;

        let result = if dry_run {
            export::build_sidecar_path(&track.file_path, format_extension(format))
                .map(|target| (target, ExportStatus::Success))
                .map_err(|err| err.to_string())
        } else {
            let persistent = to_persistent(track);
            export::export_track_format(&persistent, &parsed, format)
                .map(|result| {
                    (
                        result.path.unwrap_or_else(|| PathBuf::from(&track.file_path)),
                        result.status,
                    )
                })
                .map_err(|err| err.to_string())
        };

        match result {
            Ok((target, ExportStatus::Success)) => {
                written += 1;
                println!("{}  {}", if dry_run { "would write" } else { "wrote     " }, target.display());
            }
            Ok((_, ExportStatus::Skipped(reason))) => {
                println!("skipped  {}: {reason}", track.file_path);
            }
            Ok((_, ExportStatus::Error(reason))) => {
                failed += 1;
                eprintln!("failed   {}: {reason}", track.file_path);
            }
            Err(err) => {
                failed += 1;
                eprintln!("failed   {}: {err}", track.file_path);
            }
        }
    }

    println!("\n{written} written, {no_lyrics} had no lyrics, {failed} failed");
    if no_lyrics > 0 {
        println!("Run `sing-along fetch <path>` first to download the missing ones.");
    }
    Ok(())
}

/// `sing-along status <path>...`
fn status(args: &[String]) -> Result<()> {
    let (paths, _) = parse_args(args);
    let tracks = collect_tracks(&paths)?;

    let mut synced = 0usize;
    let mut plain = 0usize;
    let mut none = 0usize;

    for track in &tracks {
        let path = Path::new(&track.file_path);
        match sidecar_for(path) {
            Some(sidecar) => {
                let raw = std::fs::read_to_string(&sidecar).unwrap_or_default();
                let has_timed = parse_sidecar(&raw)
                    .ok()
                    .and_then(|parsed| parsed.synced_lyrics)
                    .is_some_and(|text| !text.trim().is_empty());
                if has_timed {
                    synced += 1;
                    println!("synced   {}", track.file_path);
                } else {
                    plain += 1;
                    println!("plain    {}", track.file_path);
                }
            }
            None => {
                none += 1;
                println!("none     {}", track.file_path);
            }
        }
    }

    println!("\n{synced} synced, {plain} plain only, {none} without lyrics");
    Ok(())
}

/// Find the best LRCLIB match for a track.
///
/// Searches on title and artist, then prefers the candidate whose duration is
/// closest — that is what distinguishes a studio take from a live recording when
/// both carry the same title. Returns `None` when nothing comes back.
async fn find_lyrics(
    title: &str,
    artist: &str,
    duration: f64,
    instance: &str,
    proxy: &str,
) -> Result<Option<crate::lrclib::get::RawResponse>> {
    let query = format!("{title} {artist}").trim().to_owned();
    let response = crate::lrclib::search::request("", "", "", &query, instance, proxy).await?;

    let best = response
        .items()
        .iter()
        // LRCLIB increasingly returns the `lyricsfile` field (the same YAML
        // document this app stores) with `syncedLyrics`/`plainLyrics` left empty,
        // so a hit is only usable if any of the three carries text.
        .filter(|item| {
            item.synced_lyrics.as_deref().is_some_and(|s| !s.trim().is_empty())
                || item.plain_lyrics.as_deref().is_some_and(|s| !s.trim().is_empty())
                || item.lyricsfile.as_deref().is_some_and(|s| !s.trim().is_empty())
        })
        .min_by(|a, b| {
            let drift = |item: &crate::lrclib::search::SearchItem| {
                item.duration
                    .map(|d| (d - duration).abs())
                    .unwrap_or(f64::MAX)
            };
            drift(a).total_cmp(&drift(b))
        });

    let Some(item) = best else {
        return Ok(None);
    };

    Ok(Some(crate::lrclib::get::RawResponse::from_search_item(item)))
}

/// Parse a sidecar lyric file.
///
/// Two shapes exist in a real library: the YAML lyricsfile the app writes into
/// its database, and the plain `.lrc`/`.txt` that sits next to an audio file.
/// Both must be accepted — a user pointing the CLI at their music folder has the
/// plain kind.
fn parse_sidecar(raw: &str) -> Result<lyricsfile::ParsedLyricsfile> {
    if let Ok(parsed) = lyricsfile::parse_lyricsfile(raw) {
        return Ok(parsed);
    }

    // Not YAML: treat it as plain LRC, which is what a sidecar usually is.
    //
    // The line may also be enhanced LRC (A2), whose inline `<mm:ss.xx>` word
    // tags the plain LRC parser keeps as lyric text. Strip them first, otherwise
    // the export carries the previous format's timestamps as lyrics.
    let stripped = strip_word_tags(raw);
    let parsed = crate::parser::lrc::parse_lrc(&stripped);

    let synced = if parsed.timed_lines.is_empty() {
        None
    } else {
        let mut out = String::new();
        for line in &parsed.timed_lines {
            out.push_str(&crate::parser::lrc::format_timestamp(line.timestamp_ms));
            out.push_str(&line.text);
            out.push('\n');
        }
        Some(out)
    };

    Ok(lyricsfile::ParsedLyricsfile {
        plain_lyrics: synced
            .as_deref()
            .map(crate::utils::strip_timestamp)
            .filter(|text| !text.trim().is_empty())
            .or_else(|| Some(raw.trim().to_owned()).filter(|t| !t.is_empty())),
        synced_lyrics: synced,
        is_instrumental: false,
    })
}

/// Remove inline `<mm:ss.xx>` word tags from enhanced LRC.
fn strip_word_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;

    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        // Only strip something that looks like a timestamp, so a lyric that
        // genuinely contains angle brackets survives.
        let looks_like_tag = after
            .find('>')
            .map(|close| after[..close].contains(':'))
            .unwrap_or(false);
        match (looks_like_tag, after.find('>')) {
            (true, Some(close)) => rest = &after[close + 1..],
            _ => {
                out.push('<');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Build the minimal `PersistentTrack` the export helpers expect.
///
/// Those helpers only read the path and the identity fields; the database-only
/// fields are filled with zeros because nothing here has a library.
fn to_persistent(track: &TrackMetadata) -> PersistentTrack {
    PersistentTrack {
        id: 0,
        file_path: track.file_path.clone(),
        file_name: track.file_name.clone(),
        title: track.title.clone(),
        album_name: track.album.clone(),
        album_artist_name: Some(track.album_artist.clone()),
        album_id: 0,
        artist_name: track.artist.clone(),
        artist_id: 0,
        image_path: None,
        track_number: track.track_number.map(|n| n as i64),
        txt_lyrics: None,
        lrc_lyrics: None,
        lyricsfile: None,
        lyricsfile_id: None,
        duration: track.duration,
        instrumental: false,
    }
}

fn format_extension(format: ExportFormat) -> &'static str {
    match format {
        ExportFormat::Txt => "txt",
        ExportFormat::Lrc => "lrc",
        ExportFormat::LrcA2 => "alrc",
        ExportFormat::Ttml => "ttml",
        ExportFormat::Embedded => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_lookup_is_case_insensitive() {
        let found = PLAYERS
            .iter()
            .find(|(name, _, _)| name.eq_ignore_ascii_case("Gramophone"));
        assert!(found.is_some());
        assert_eq!(found.unwrap().1, ExportFormat::LrcA2);
    }

    #[test]
    fn audio_extensions_are_recognised() {
        assert!(is_audio(Path::new("a.mp3")));
        assert!(is_audio(Path::new("a.FLAC")));
        assert!(!is_audio(Path::new("a.lrc")));
        assert!(!is_audio(Path::new("a.txt")));
    }

    #[test]
    fn flags_parse_both_forms() {
        let args: Vec<String> = ["--for", "steve", "--dry-run", "/music"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let (paths, flags) = parse_args(&args);
        assert_eq!(paths, vec!["/music".to_string()]);
        assert_eq!(flags.get("for").map(|s| s.as_str()), Some("steve"));
        assert!(flags.contains_key("dry-run"));

        let inline: Vec<String> = ["--format=ttml", "/music"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let (_, flags) = parse_args(&inline);
        assert_eq!(flags.get("format").map(|s| s.as_str()), Some("ttml"));
    }

    #[test]
    fn word_tags_are_stripped_before_parsing() {
        let a2 = "[00:12.00]<00:12.00>青<00:13.40>花<00:14.10>瓷\n";
        let parsed = parse_sidecar(a2).unwrap();
        let synced = parsed.synced_lyrics.unwrap();
        assert!(synced.contains("青花瓷"), "got {synced:?}");
        assert!(!synced.contains('<'), "word tags leaked: {synced:?}");
    }

    #[test]
    fn angle_brackets_in_real_lyrics_survive() {
        // A lyric can contain "<3"; only timestamp-shaped spans are removed.
        let raw = "[00:10.00]I <3 you\n";
        let parsed = parse_sidecar(raw).unwrap();
        assert!(parsed.plain_lyrics.unwrap().contains("<3"));
    }

    #[test]
    fn plain_lrc_is_parsed_as_a_sidecar() {
        let raw = "[00:12.00]第一行\n[00:15.50]第二行\n";
        let parsed = parse_sidecar(raw).unwrap();
        assert!(parsed.synced_lyrics.is_some());
        assert!(parsed.plain_lyrics.unwrap().contains("第一行"));
    }
}
