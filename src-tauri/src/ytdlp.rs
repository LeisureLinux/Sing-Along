//! Driving an external `yt-dlp` to fetch a track.
//!
//! The downloader is not reimplemented here. `yt-dlp` is the only thing that
//! knows how to talk to YouTube, and it changes every week; a copy of it in
//! this project would be a copy that stops working. So the application calls
//! the user's own `yt-dlp` and reads the result.
//!
//! ## What the caller has to provide
//!
//! Two things, and both are the user's to choose:
//!
//! * **The binary.** The `yt-dlp` on `PATH`, or a path the user gave. The
//!   Python installation and the compiled binary behave differently here: the
//!   compiled one cannot load the plugins that produce a proof-of-origin
//!   token, and the token is what YouTube asks for when a request comes from
//!   a residential address.
//!
//! * **The cookies.** A signed-out browser cannot download from YouTube
//!   reliably, and the browser's cookie database is encrypted with a key in
//!   the login keyring, so the cookies have to be exported. The export is a
//!   JSON file that a browser extension writes; `crate::cookies` turns it into
//!   the text format `yt-dlp` reads.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

/// What `yt-dlp` reports about a track.
#[derive(Debug, Clone, Deserialize)]
pub struct TrackInfo {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub album_artist: Option<String>,
    #[serde(default)]
    pub thumbnail: Option<String>,
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub webpage_url: Option<String>,
}

/// The result of a download.
#[derive(Debug, Clone)]
pub struct DownloadedTrack {
    /// Where the file landed.
    pub path: PathBuf,
    /// What `yt-dlp` said about it.
    pub info: TrackInfo,
}

/// The settings that decide how `yt-dlp` is called.
#[derive(Debug, Clone)]
pub struct DownloadOptions {
    /// The `yt-dlp` to run. Empty means the one on `PATH`.
    pub binary: String,
    /// The Netscape cookie file. Empty means no cookies.
    pub cookies: PathBuf,
    /// The proxy for the download. Empty means a direct connection.
    pub proxy: String,
    /// Where the file should land.
    pub output_dir: PathBuf,
    /// The script the metadata should end up in.
    pub script: crate::script_convert::ScriptTarget,
    /// Whether the album and the cover should be looked up.
    pub fix_metadata: bool,
    /// Whether an existing file of the same name is replaced.
    ///
    /// `yt-dlp` does not replace one: it says "this has already been
    /// downloaded" and stops, which leaves the caller believing it
    /// succeeded. The flag is passed through, so the decision is the
    /// caller's.
    pub overwrite: bool,
    /// A suffix for the file name, for the case where the download is to sit
    /// beside an existing one rather than replace it.
    pub name_suffix: String,
    /// The proxy for the metadata lookups, which are a different service and
    /// may need a different route.
    pub metadata_proxy: String,
}

/// The name of the file the download produces, before the extension.
///
/// The name is built from the artist and the title, in the script the user
/// asked for, because that is what a listener expects to see in a file
/// browser: "許美靜 - 城裡的月光.mp3" rather than "AbQ7DWlQCe8.mp3".
pub fn file_stem_for(info: &TrackInfo, script: crate::script_convert::ScriptTarget) -> String {
    output_name(info, script)
}

/// The name of the file the download produces, before the extension.
fn output_name(info: &TrackInfo, script: crate::script_convert::ScriptTarget) -> String {
    let artist = info
        .artist
        .as_deref()
        .or(info.album_artist.as_deref())
        .unwrap_or_default()
        .trim();
    let title = info.title.trim();

    let convert = |text: &str| {
        if text.is_empty() {
            String::new()
        } else {
            crate::script_convert::convert(text, script)
        }
    };

    let artist = convert(artist);
    let title = convert(title);

    match (artist.is_empty(), title.is_empty()) {
        (true, true) => "track".to_owned(),
        (true, false) => title,
        (false, true) => artist,
        (false, false) => format!("{artist} - {title}"),
    }
}

/// Remove the characters a file name cannot hold.
///
/// A slash in an artist name would otherwise create a directory that does not
/// exist, and the download would fail with a message about a missing path
/// rather than about the name.
pub fn sanitise(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => ' ',
            c if (c as u32) < 0x20 => ' ',
            c => c,
        })
        .collect();
    let trimmed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if trimmed.is_empty() {
        "track".to_owned()
    } else {
        trimmed
    }
}

/// Look up what `yt-dlp` knows about a track, without downloading it.
///
/// The metadata comes from the same place the download would get it, so what
/// the user is shown before the download is what the file will contain.
pub async fn probe(
    options: &DownloadOptions,
    url: &str,
) -> Result<TrackInfo> {
    let binary = resolve_binary(&options.binary)?;

    let mut command = Command::new(&binary);
    command
        .arg("--dump-single-json")
        .arg("--no-warnings")
        .arg("--skip-download")
        .arg("--no-playlist")
        .arg(url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    apply_common_arguments(&mut command, options);

    let output = command
        .output()
        .await
        .with_context(|| format!("cannot run {}", binary.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("yt-dlp failed: {}", first_meaningful_line(&stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // `--dump-single-json` prints one object, but a stray line can precede it.
    let json = stdout
        .lines()
        .find(|line| line.trim_start().starts_with('{'))
        .ok_or_else(|| anyhow::anyhow!("yt-dlp did not report anything about this track"))?;

    serde_json::from_str(json).context("yt-dlp reported something unreadable")
}

/// Download a track as an MP3 into the output directory.
pub async fn download(options: &DownloadOptions, url: &str) -> Result<DownloadedTrack> {
    let binary = resolve_binary(&options.binary)?;

    std::fs::create_dir_all(&options.output_dir).with_context(|| {
        format!(
            "cannot create the download directory {}",
            options.output_dir.display()
        )
    })?;

    // The name is asked for first, so that the file is named after the track
    // rather than after its identifier.
    let info = probe(options, url).await?;
    let name = sanitise(&format!(
        "{}{}",
        output_name(&info, options.script),
        options.name_suffix
    ));

    let mut command = Command::new(&binary);
    command
        .arg("-f")
        .arg("bestaudio/best")
        .arg("-x")
        .arg("--audio-format")
        .arg("mp3")
        .arg("--audio-quality")
        .arg("0")
        .arg("--add-metadata")
        .arg("--embed-thumbnail")
        .arg("-o")
        .arg(options.output_dir.join(format!("{name}.%(ext)s")))
        .arg("--no-playlist")
        .arg(url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if options.overwrite {
        // Without this, `yt-dlp` refuses to replace a file that is already
        // there: it reports "has already been downloaded" and exits
        // successfully, so the caller sees a success and an unchanged file.
        command.arg("--force-overwrites");
    }

    apply_common_arguments(&mut command, options);

    let output = command
        .output()
        .await
        .with_context(|| format!("cannot run {}", binary.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("yt-dlp failed: {}", first_meaningful_line(&stderr));
    }

    // The file is found by name rather than by parsing the output, because the
    // output says "Destination:" for the intermediate file too, and that one
    // is deleted when the conversion finishes.
    let expected = options.output_dir.join(format!("{name}.mp3"));
    let path = if expected.exists() {
        expected
    } else {
        find_newest_mp3(&options.output_dir, &name).ok_or_else(|| {
            anyhow::anyhow!("the download finished but no file was left behind")
        })?
    };

    Ok(DownloadedTrack { path, info })
}

/// The arguments every call shares.
fn apply_common_arguments(command: &mut Command, options: &DownloadOptions) {
    if !options.cookies.as_os_str().is_empty() && options.cookies.exists() {
        command.arg("--cookies").arg(&options.cookies);
    }

    if !options.proxy.trim().is_empty() {
        command.arg("--proxy").arg(options.proxy.trim());
    }

    // The metadata yt-dlp writes is the metadata YouTube has, which is not
    // what the user wants. It is corrected afterwards, and the tags yt-dlp
    // writes are kept because they carry the release date and the composer.
    command.arg("--js-runtimes").arg("quickjs");
    command.arg("--remote-components").arg("ejs:github");
}

/// The `yt-dlp` to run.
///
/// A configured path wins over the one on `PATH`, so a user with several
/// installations can say which one to use. The compiled binary is a separate
/// file with no `.exe` suffix on Unix, so both are tried.
fn resolve_binary(configured: &str) -> Result<PathBuf> {
    let configured = configured.trim();
    if !configured.is_empty() {
        let path = PathBuf::from(configured);
        if path.exists() {
            return Ok(path);
        }
        anyhow::bail!("the yt-dlp at {} does not exist", path.display());
    }

    if let Ok(path) = which("yt-dlp") {
        return Ok(path);
    }

    anyhow::bail!(
        "yt-dlp was not found. Install it, or set the path to it in the settings."
    )
}

/// Find a program on the `PATH`.
fn which(program: &str) -> Result<PathBuf> {
    let path = std::env::var_os("PATH").ok_or_else(|| anyhow::anyhow!("no PATH"))?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(program);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(anyhow::anyhow!("{program} not found"))
}

/// The most recently written MP3 that starts with a name.
fn find_newest_mp3(dir: &Path, name: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("mp3") {
            continue;
        }
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
        if !stem.starts_with(name) {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        if best.as_ref().is_none_or(|(best_time, _)| modified > *best_time) {
            best = Some((modified, path));
        }
    }
    best.map(|(_, path)| path)
}

/// The first line of an error message that says something.
///
/// `yt-dlp` prints a banner, then the progress, then the error. Taking the
/// first line gives a banner; taking the last line gives the reason. The
/// error lines are the ones that start with "ERROR", so those are preferred.
fn first_meaningful_line(stderr: &str) -> String {
    let lines: Vec<&str> = stderr
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect();

    if let Some(error) = lines.iter().find(|line| line.starts_with("ERROR")) {
        return error
            .trim_start_matches("ERROR:")
            .trim()
            .to_owned();
    }

    lines.last().copied().unwrap_or("no message").to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script_convert::ScriptTarget;

    fn info() -> TrackInfo {
        TrackInfo {
            id: "AbQ7DWlQCe8".to_owned(),
            title: "城裡的月光".to_owned(),
            album: Some("2021熱歌合集".to_owned()),
            artist: Some("許美靜".to_owned()),
            album_artist: None,
            thumbnail: None,
            duration: Some(316.0),
            webpage_url: None,
        }
    }

    #[test]
    fn the_name_holds_the_artist_and_the_title() {
        assert_eq!(
            output_name(&info(), ScriptTarget::Off),
            "許美靜 - 城裡的月光"
        );
    }

    #[test]
    fn the_name_is_converted_to_the_requested_script() {
        assert_eq!(
            output_name(&info(), ScriptTarget::Simplified),
            "许美静 - 城里的月光"
        );
    }

    #[test]
    fn a_track_without_an_artist_is_named_after_its_title() {
        let mut info = info();
        info.artist = None;
        assert_eq!(output_name(&info, ScriptTarget::Off), "城裡的月光");
    }

    #[test]
    fn a_slash_in_a_name_does_not_create_a_directory() {
        assert_eq!(sanitise("AC/DC - Back"), "AC DC - Back");
    }

    #[test]
    fn a_name_that_is_only_punctuation_still_produces_a_file() {
        assert_eq!(sanitise("///"), "track");
    }

    #[test]
    fn the_error_line_is_preferred_over_the_banner() {
        let stderr = "[youtube] Extracting URL: ...\nERROR: Sign in to confirm you're not a bot\n";
        assert_eq!(
            first_meaningful_line(stderr),
            "Sign in to confirm you're not a bot"
        );
    }

    #[test]
    fn a_failure_without_an_error_line_still_says_something() {
        assert_eq!(first_meaningful_line("something went wrong\n"), "something went wrong");
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::script_convert::ScriptTarget;

    /// The end-to-end path, exercised against the real `yt-dlp` when there is
    /// one on the machine.
    ///
    /// Skipped when there is no `yt-dlp`, because a test that needs a program
    /// the machine may not have is a test that fails for the wrong reason.
    #[tokio::test]
    async fn a_probe_reports_the_track() {
        let Ok(binary) = which("yt-dlp") else {
            eprintln!("skipped: no yt-dlp on the PATH");
            return;
        };
        if binary.to_string_lossy().contains("miniforge") {
            // The one in the development environment is a wrapper that needs
            // an interpreter the test does not set up.
            eprintln!("skipped: the yt-dlp on the PATH is a wrapper script");
            return;
        }

        let options = DownloadOptions {
            binary: String::new(),
            cookies: PathBuf::new(),
            proxy: String::new(),
            metadata_proxy: String::new(),
            output_dir: PathBuf::new(),
            script: ScriptTarget::Off,
            fix_metadata: false,
            overwrite: false,
            name_suffix: String::new(),
        };

        // A video that is public and needs no account, chosen because the
        // test must not depend on anything the machine might not have.
        let info = probe(&options, "https://www.youtube.com/watch?v=dQw4w9WgXcQ")
            .await
            .expect("the probe should succeed");

        assert!(!info.id.is_empty());
        assert!(!info.title.is_empty());
    }
}
