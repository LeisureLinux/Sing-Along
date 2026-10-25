//! The commands that fetch a track from YouTube.
//!
//! Three things happen here, in order, and each can fail on its own:
//!
//! 1. The user's cookie export is turned into the format `yt-dlp` reads, and
//!    checked for expiry. This is first because it is the step that fails
//!    most often, and the failure it prevents is the worst one to debug: a
//!    download that dies with "Sign in to confirm you're not a bot" says
//!    nothing about the file on disk being the problem.
//!
//! 2. `yt-dlp` is asked what the track is. This is cheap and it gives the user
//!    the title, the artist and the album before anything is downloaded, so a
//!    wrong URL is caught before a large file is fetched.
//!
//! 3. The download runs, and the file is corrected afterwards: the album name
//!    and the cover come from the release database rather than from YouTube,
//!    and the Chinese text is converted to the script the user asked for.

use crate::persistent_entities::PersistentConfig;
use tauri::{Emitter, Manager};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// What the GUI shows about the downloader.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YtdlpStatus {
    /// Whether a `yt-dlp` was found.
    pub binary_found: bool,
    /// The binary that will be used.
    pub binary: String,
    /// The problem with the binary, if there is one.
    pub binary_error: Option<String>,
    /// The cookie export the user has chosen.
    pub cookies_json: String,
    /// The text file derived from it.
    pub cookies_txt: String,
    /// Whether the cookie file was read.
    pub cookies_found: bool,
    /// How many cookies it holds.
    pub cookie_count: usize,
    /// How many of them have expired.
    pub cookies_expired: usize,
    /// Whether the cookies identify a signed-in session.
    pub cookies_signed_in: bool,
    /// When the session cookies expire, if they have an expiry.
    pub cookies_expires: Option<i64>,
    /// A message the user has to act on, in plain words.
    pub problem: Option<String>,
}

/// The result of a probe: what `yt-dlp` knows about a track.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YoutubeTrackInfo {
    pub id: String,
    pub title: String,
    pub album: Option<String>,
    pub artist: Option<String>,
    pub duration: Option<f64>,
    /// The album the release database has, when it has one.
    pub corrected_album: Option<String>,
    /// The artwork the release database has, when it has one.
    pub corrected_cover: bool,
    /// A track in the library that is the same recording.
    ///
    /// The user is asked what to do about it, because the answer depends on
    /// things only the user knows: whether the file on disk is a better copy
    /// than the one in the library, or whether the library entry is a rip
    /// they would rather keep.
    pub existing_track: Option<ExistingTrack>,
}

/// A track in the library that the download would duplicate.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExistingTrack {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub album: String,
    /// Where the file is.
    pub file_path: String,
    /// How long it is, so the user can tell two recordings apart.
    pub duration: f64,
    /// Whether it has lyrics already.
    pub has_lyrics: bool,
}

/// What the user decided to do about an existing track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateChoice {
    /// Replace the file and the library entry.
    Replace,
    /// Keep the library entry and put the download beside it under a
    /// different name.
    KeepBoth,
    /// Do not download.
    Cancel,
}

/// The result of a download.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YoutubeDownloadResult {
    /// Where the file landed.
    pub path: String,
    /// The title it ended up with.
    pub title: String,
    /// The album it ended up with.
    pub album: Option<String>,
    /// The artist it ended up with.
    pub artist: Option<String>,
    /// Whether the album was corrected.
    pub album_corrected: bool,
    /// Whether the cover was replaced.
    pub cover_corrected: bool,
    /// The library entry the file became, when the library was told about it.
    pub track_id: Option<i64>,
    /// Whether lyrics were found for it.
    pub lyrics_found: bool,
    /// Why no lyrics were found, in words the user can act on.
    pub lyrics_note: Option<String>,
    /// The library entry, with the fields the player needs.
    ///
    /// The dialog offers to play the track, and playing it means telling the
    /// player about a library entry rather than about a file on disk.
    pub track: Option<DownloadedTrackInfo>,
}

/// What the player needs to play a downloaded track.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadedTrackInfo {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration: Option<f64>,
}



/// A suffix that makes the file name free.
///
/// "Keep both" means two files of the same song, side by side. The second
/// one is called "Artist - Title (2).mp3", the third "Artist - Title (3).mp3",
/// and so on: a number a person can read, rather than a timestamp they have
/// to decode.
async fn find_free_suffix(
    config: &PersistentConfig,
    output_dir: &Path,
    url: &str,
    script: crate::script_convert::ScriptTarget,
) -> Result<String, String> {
    let options = download_options(config, Some(output_dir.to_path_buf()), false, "", script);
    let info = crate::ytdlp::probe(&options, url)
        .await
        .map_err(|err| err.to_string())?;

    let base = crate::ytdlp::sanitise(&crate::ytdlp::file_stem_for(&info, script));

    for number in 2..100 {
        let candidate = format!(" ({number})");
        let path = output_dir.join(format!("{base}{candidate}.mp3"));
        if !path.exists() {
            return Ok(candidate);
        }
    }

    Ok(" (2)".to_owned())
}

/// The library track that a download would duplicate.
///
/// The match is on the title, the artist and the duration, which together
/// identify a recording well enough: two different songs rarely share all
/// three, and the same song released twice does — but then the two files are
/// the same recording and the user is the one who knows which to keep.
///
/// The scripts are compared after conversion, because a library that was
/// downloaded in Simplified and a download that arrives in Traditional are
/// the same recording.
fn find_existing_track(
    title: &str,
    artist: &str,
    album: Option<&str>,
    duration: Option<f64>,
    conn: &rusqlite::Connection,
) -> Option<ExistingTrack> {
    // The comparison is made in one script.
    //
    // YouTube publishes in whatever script the uploader chose, and the
    // library holds the title as it was written into the file — which, for a
    // download, is the script the user asked for. So a library entry in
    // Simplified and a download in Traditional are the same recording, and
    // the search has to see that.
    //
    // Both sides are converted to Simplified for the search, because the
    // library's own text is what the search runs against.
    let title = crate::script_convert::convert(title, crate::script_convert::ScriptTarget::Simplified);
    let artist =
        crate::script_convert::convert(artist, crate::script_convert::ScriptTarget::Simplified);
    let album = album.map(|value| {
        crate::script_convert::convert(value, crate::script_convert::ScriptTarget::Simplified)
    });

    let mut matches = crate::db::find_tracks_by_metadata(
        &title,
        Some(&artist),
        album.as_deref(),
        duration,
        conn,
    )
    .ok()?;

    // The search is exact on the title, so a recording whose title differs
    // only in the script is not found by it. The candidates are therefore
    // looked at again, with the script ignored.
    if matches.is_empty() {
        matches = find_tracks_by_title_and_artist(&title, &artist, duration, conn);
    }

    let track = matches.into_iter().next()?;

    let has_lyrics = crate::db::get_track_lyricsfile(track.id, conn)
        .ok()
        .flatten()
        .is_some();

    Some(ExistingTrack {
        id: track.id,
        title: track.title,
        artist: track.artist_name,
        album: track.album_name,
        file_path: track.file_path,
        duration: track.duration,
        has_lyrics,
    })
}


/// The library tracks with a title and an artist, whatever the script.
///
/// The search the database offers is exact on the normalised title, and the
/// normalisation does not know about the Chinese scripts. So the two texts
/// that differ only in the script do not match, and the recording a download
/// is about to duplicate is not found.
fn find_tracks_by_title_and_artist(
    title: &str,
    artist: &str,
    duration: Option<f64>,
    conn: &rusqlite::Connection,
) -> Vec<crate::persistent_entities::PersistentTrack> {
    let Ok(tracks) = crate::db::get_tracks(conn) else {
        return Vec::new();
    };

    let wanted = normalise_for_comparison(title);
    let wanted_artist = normalise_for_comparison(artist);

    tracks
        .into_iter()
        .filter(|track| {
            if normalise_for_comparison(&track.title) != wanted {
                return false;
            }
            if !wanted_artist.is_empty()
                && normalise_for_comparison(&track.artist_name) != wanted_artist
            {
                return false;
            }
            // The duration has to be close, when it is known: two different
            // recordings rarely share a title and an artist exactly.
            match (duration, track.duration) {
                (Some(wanted), actual) if actual > 0.0 => (wanted - actual).abs() <= 3.0,
                _ => true,
            }
        })
        .collect()
}

/// A text reduced to a comparable form, with the Chinese in one script.
fn normalise_for_comparison(text: &str) -> String {
    crate::script_convert::convert(text, crate::script_convert::ScriptTarget::Simplified)
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// The settings that decide how the downloader is called, read from the
/// configuration.
fn download_options(
    config: &PersistentConfig,
    output_dir: Option<PathBuf>,
    overwrite: bool,
    name_suffix: &str,
    script: crate::script_convert::ScriptTarget,
) -> crate::ytdlp::DownloadOptions {
    let cookies = if config.ytdlp_cookies_json.trim().is_empty() {
        PathBuf::new()
    } else {
        let json = PathBuf::from(config.ytdlp_cookies_json.trim());
        crate::cookies::netscape_path_for(&json)
    };

    // The download itself goes through the proxy the user set for the lyric
    // services: the same machine, the same network path.
    let proxy = if config.proxy_url.trim().is_empty() {
        String::new()
    } else {
        config.proxy_url.trim().to_owned()
    };

    crate::ytdlp::DownloadOptions {
        binary: config.ytdlp_binary.trim().to_owned(),
        cookies,
        proxy: proxy.clone(),
        metadata_proxy: proxy,
        output_dir: output_dir.unwrap_or_default(),
        script,
        fix_metadata: true,
        overwrite,
        name_suffix: name_suffix.to_owned(),
        progress: None,
    }
}

/// The Chinese script the metadata should end up in.
///
/// The caller decides, because only the caller knows the interface language.
/// The rule is the one the lyric download uses: the answer the user gave to
/// the download prompt wins, and the interface language is the fallback.
///
/// It matters here because YouTube publishes in whatever script the uploader
/// chose. Without it a Simplified-Chinese reader would get a file whose title
/// is in Traditional, and would not find it again by typing the name they
/// see in their player.
pub fn script_target(requested: Option<&str>, locale: &str) -> crate::script_convert::ScriptTarget {
    let target = match requested {
        // "ask" means the user has not answered the prompt yet, so the
        // interface language decides.
        Some("ask") | None | Some("") => match locale {
            "zh-Hant" => "traditional",
            "zh-Hans" => "simplified",
            _ => "off",
        },
        Some(other) => other,
    };

    crate::script_convert::ScriptTarget::parse(target)
        .unwrap_or(crate::script_convert::ScriptTarget::Off)
}

/// Report what the downloader can and cannot do right now.
///
/// The GUI calls this when the settings dialog opens, so that a user who has
/// not set anything up is told what is missing rather than finding out when
/// a download fails.
#[tauri::command]
pub async fn get_ytdlp_status(app_state: tauri::State<'_, crate::state::AppState>) -> Result<YtdlpStatus, String> {
    let conn_guard = app_state.db.lock().unwrap();
    let conn = conn_guard.as_ref().unwrap();
    let config = crate::db::get_config(conn).map_err(|err| err.to_string())?;
    drop(conn_guard);

    let mut status = YtdlpStatus {
        binary_found: false,
        binary: config.ytdlp_binary.trim().to_owned(),
        binary_error: None,
        cookies_json: config.ytdlp_cookies_json.trim().to_owned(),
        cookies_txt: String::new(),
        cookies_found: false,
        cookie_count: 0,
        cookies_expired: 0,
        cookies_signed_in: false,
        cookies_expires: None,
        problem: None,
    };

    // The binary.
    let binary = if config.ytdlp_binary.trim().is_empty() {
        "yt-dlp".to_owned()
    } else {
        config.ytdlp_binary.trim().to_owned()
    };
    match std::path::Path::new(&binary).exists() {
        true => {
            status.binary_found = true;
            status.binary = binary;
        }
        false => {
            // Empty means "the one on PATH", which the runner looks up.
            if config.ytdlp_binary.trim().is_empty() {
                status.binary_error =
                    Some("yt-dlp was not found on the PATH".to_owned());
            } else {
                status.binary_error =
                    Some(format!("there is no yt-dlp at {binary}"));
            }
        }
    }

    // The cookies.
    if !config.ytdlp_cookies_json.trim().is_empty() {
        let json = PathBuf::from(config.ytdlp_cookies_json.trim());
        let txt = crate::cookies::netscape_path_for(&json);
        status.cookies_txt = txt.display().to_string();

        if txt.exists() {
            match crate::cookies::inspect_netscape(&txt) {
                Ok(inspected) => {
                    status.cookies_found = true;
                    status.cookie_count = inspected.count;
                    status.cookies_expired = inspected.expired;
                    status.cookies_signed_in = inspected.signed_in;
                    status.cookies_expires = inspected.session_expires;
                }
                Err(err) => {
                    status.binary_error = Some(format!("the cookie file is unreadable: {err}"));
                }
            }
        } else {
            status.problem = Some(
                "the cookie export has not been converted yet".to_owned(),
            );
        }
    } else {
        status.problem = Some("no cookie export has been chosen".to_owned());
    }

    // The most important message wins: an expired cookie is what makes a
    // download fail with a message that blames the user for something they
    // did not do.
    if status.cookies_found && status.cookies_expired > 0 {
        status.problem = Some(format!(
            "{} of the {} cookies have expired. Export them again from the browser.",
            status.cookies_expired, status.cookie_count
        ));
    } else if status.cookies_found && !status.cookies_signed_in {
        status.problem = Some(
            "the cookies do not carry a signed-in session. Downloads of most videos will fail."
                .to_owned(),
        );
    }

    Ok(status)
}

/// Turn the user's cookie export into the file `yt-dlp` reads.
///
/// Called when the user picks the JSON file, so the conversion happens once
/// and the text file is there afterwards.
#[tauri::command]
pub async fn convert_cookie_export(
    json_path: String,
    app_state: tauri::State<'_, crate::state::AppState>,
) -> Result<crate::cookies::CookieStatus, String> {
    let path = PathBuf::from(json_path.trim());
    if path.as_os_str().is_empty() {
        return Err("no cookie export was given".to_owned());
    }

    let status = crate::cookies::convert_json_file(&path).map_err(|err| err.to_string())?;

    // Remember where it is, so the user does not have to pick it again.
    {
        let conn_guard = app_state.db.lock().unwrap();
        let conn = conn_guard.as_ref().unwrap();
        let mut config = crate::db::get_config(conn).map_err(|err| err.to_string())?;
        config.ytdlp_cookies_json = path.display().to_string();
        crate::db::set_ytdlp_settings(
            &config.ytdlp_binary,
            &config.ytdlp_cookies_json,
            &config.ytdlp_output_dir,
            conn,
        )
        .map_err(|err| err.to_string())?;
    }

    Ok(status)
}

/// Ask `yt-dlp` what a track is, without downloading it.
#[tauri::command]
pub async fn probe_youtube_track(
    url: String,
    // The Chinese script the metadata should end up in, and the interface
    // language it falls back to. Only the caller knows the language.
    lyric_script: Option<String>,
    locale: String,
    app_state: tauri::State<'_, crate::state::AppState>,
    app_handle: tauri::AppHandle,
) -> Result<YoutubeTrackInfo, String> {
    // The lock is taken and released inside a block, so that no guard is held
    // across an await: a guard is not `Send`, and a command that holds one
    // across an await cannot be called from the main thread.
    let config = {
        let conn_guard = app_state.db.lock().unwrap();
        let conn = conn_guard.as_ref().unwrap();
        crate::db::get_config(conn).map_err(|err| err.to_string())?
    };

    let script = script_target(lyric_script.as_deref(), &locale);
    let options = download_options(&config, None, false, "", script);
    let info = crate::ytdlp::probe(&options, url.trim())
        .await
        .map_err(|err| err.to_string())?;

    // The release database is asked at the same time, so the user sees the
    // album they will actually get and not the one YouTube reports.
    let (corrected_album, corrected_cover) = if config.ytdlp_output_dir.is_empty() {
        (None, false)
    } else {
        match crate::youtube::lookup(
            &crate::http::client(&config.proxy_url, 15)
                .map_err(|err| err.to_string())?,
            &info.title,
            info.artist.as_deref().unwrap_or_default(),
        )
        .await
        {
            Ok(Some(correction)) => (correction.album, correction.cover_url.is_some()),
            Ok(None) => (None, false),
            // The database being unreachable is not a reason to refuse the
            // download: the file is still worth having, just with the album
            // name YouTube gave.
            Err(_) => (None, false),
        }
    };

    // The library is asked whether it already holds this recording. The
    // answer is what lets the user choose between replacing it and keeping
    // both, instead of silently overwriting a file they may care about.
    let existing_track = {
        let conn_guard = app_state.db.lock().unwrap();
        let conn = conn_guard.as_ref().unwrap();
        find_existing_track(
            &info.title,
            info.artist.as_deref().unwrap_or_default(),
            corrected_album.as_deref().or(info.album.as_deref()),
            info.duration,
            conn,
        )
    };

    Ok(YoutubeTrackInfo {
        id: info.id,
        title: info.title,
        album: info.album,
        artist: info.artist,
        duration: info.duration,
        corrected_album,
        corrected_cover,
        existing_track,
    })
}

/// Download a track from YouTube.
///
/// The file is corrected after the download: the album and the cover come
/// from the release database, and the text is converted to the script the
/// user asked for. Then the library is told to look at the directory again, so
/// the new file shows up without the user doing anything.
#[tauri::command]
pub async fn download_youtube_track(
    url: String,
    // What to do about a track the library already holds: "replace"
    // overwrites the file, "keepBoth" puts the download beside it under a
    // different name, and "cancel" does nothing. A string rather than an
    // enum because it crosses the IPC boundary.
    duplicate_choice: Option<String>,
    // Whether the track should start playing once it is downloaded.
    play_now: Option<bool>,
    // Whether the progress of the download is reported.
    //
    // A download of a large file takes minutes, and a window that says
    // nothing for three minutes looks broken. The report is opt-in because
    // it is a lot of text, and a user who does not need it would rather not
    // read it.
    show_progress: Option<bool>,
    // The Chinese script the metadata should end up in, and the interface
    // language it falls back to. Only the caller knows the language.
    lyric_script: Option<String>,
    locale: String,
    app_state: tauri::State<'_, crate::state::AppState>,
    app_handle: tauri::AppHandle,
) -> Result<YoutubeDownloadResult, String> {
    let (config, directories) = {
        let conn_guard = app_state.db.lock().unwrap();
        let conn = conn_guard.as_ref().unwrap();
        let config = crate::db::get_config(conn).map_err(|err| err.to_string())?;
        let directories = crate::db::get_directories(conn).unwrap_or_default();
        (config, directories)
    };

    // Where the file goes: the directory the user chose, or the first one the
    // library watches, which is where the rest of their music already is.
    let output_dir = if config.ytdlp_output_dir.trim().is_empty() {
        directories
            .into_iter()
            .next()
            .map(PathBuf::from)
            .ok_or_else(|| {
                "there is no library directory to download into. Add one first."
                    .to_owned()
            })?
    } else {
        PathBuf::from(config.ytdlp_output_dir.trim())
    };

    let script = script_target(lyric_script.as_deref(), &locale);

    let choice = match duplicate_choice.as_deref() {
        Some("replace") => DuplicateChoice::Replace,
        Some("keepBoth") => DuplicateChoice::KeepBoth,
        Some("cancel") => DuplicateChoice::Cancel,
        // No answer means no decision was needed: there was no duplicate.
        _ => DuplicateChoice::Replace,
    };

    if choice == DuplicateChoice::Cancel {
        return Err("the download was cancelled".to_owned());
    }

    // The file name is worked out before the download, because "keep both"
    // needs a name that is not taken. The name `yt-dlp` would choose is the
    // artist and the title, so that is what is checked.
    let name_suffix = if choice == DuplicateChoice::KeepBoth {
        find_free_suffix(&config, &output_dir, url.trim(), script).await?
    } else {
        String::new()
    };

    let mut options = download_options(
        &config,
        Some(output_dir),
        choice == DuplicateChoice::Replace,
        &name_suffix,
        script,
    );

    // The progress is reported as an event, so that the dialog can show it
    // without polling.
    if show_progress.unwrap_or(false) {
        let app_handle = app_handle.clone();
        options.progress = Some(std::sync::Arc::new(move |update: crate::ytdlp::DownloadProgress| {
            let _ = app_handle.emit("youtube-download-progress", &update);
        }));
    }

    // The cookie export is converted here rather than earlier, so that a user
    // who picked a file but never saved the settings still gets a working
    // download.
    if !config.ytdlp_cookies_json.trim().is_empty() {
        let json = PathBuf::from(config.ytdlp_cookies_json.trim());
        let txt = crate::cookies::netscape_path_for(&json);
        if !txt.exists() {
            let _ = crate::cookies::convert_json_file(&json);
        }
        options.cookies = txt;
    }

    let downloaded = crate::ytdlp::download(&options, url.trim())
        .await
        .map_err(|err| err.to_string())?;

    // Correct the metadata. A failure here does not fail the download: the
    // file is already there, and a wrong album name is a smaller problem than
    // a lost download.
    let mut album_corrected = false;
    let mut cover_corrected = false;
    let mut album = downloaded.info.album.clone();

    if options.fix_metadata {
        let client = crate::http::client(&options.metadata_proxy, 15)
            .map_err(|err| err.to_string())?;

        match crate::youtube::lookup(
            &client,
            &downloaded.info.title,
            downloaded.info.artist.as_deref().unwrap_or_default(),
        )
        .await
        {
            Ok(Some(correction)) => {
            let cover_bytes = match &correction.cover_url {
                Some(url) => crate::youtube::download_cover(&client, url).await.ok(),
                None => None,
            };

            let _ = crate::youtube::write_metadata(
                &downloaded.path,
                correction.album.as_deref(),
                correction.year.as_deref(),
                cover_bytes.as_deref(),
            );

            if correction.album.is_some() {
                album = correction.album.clone();
                album_corrected = true;
            }
            cover_corrected = cover_bytes.is_some();
            }
            // The lookup failing is not a failure of the download: the file
            // is already there, with the album name YouTube gave. It is
            // logged, because a release database that is unreachable and one
            // that has no entry look the same from the outside.
            Ok(None) => {}
            Err(error) => {
                eprintln!("The release database could not be reached: {error:#}");
            }
        }

        // The script conversion comes last, so it also converts the album
        // name that was just written.
        let _ = crate::youtube::convert_file_script(&downloaded.path, options.script);
    }

    // The download directory has to be watched, or the file is in a folder
    // the library does not look at: the track would be added by hand here
    // and then dropped by the next scan, which deletes the entries whose
    // files it did not see.
    {
        let conn_guard = app_state.db.lock().map_err(|err| err.to_string())?;
        let conn = conn_guard.as_ref().unwrap();
        let _ = crate::db::add_directory(&options.output_dir.display().to_string(), conn);
    }

    // The library is told about the new file, so that the track exists and
    // can have lyrics like any other. The scan is of the download directory
    // only, because a full scan of a large library would take minutes for
    // one new song.
    let track_id = register_in_library(&app_state, &app_handle, &downloaded.path).await;

    // And the lyrics are fetched, the same way they are for any other track.
    // A song without lyrics is the usual case on YouTube: the platform has
    // no lyrics of its own, and the sources this project uses may not hold
    // this recording either. So a failure is reported rather than raised.
    let (lyrics_found, lyrics_note) = match track_id {
        Some(track_id) => fetch_lyrics_for(track_id, &app_handle, script).await,
        None => (false, Some("the file was not added to the library".to_owned())),
    };

    // The track as the player needs it, so that the dialog can offer to play
    // it and the player can be told about a library entry rather than a file.
    let track = match track_id {
        Some(track_id) => {
            let conn_guard = app_state.db.lock().ok();
            let conn = conn_guard.as_ref().and_then(|guard| guard.as_ref());
            conn.and_then(|conn| {
                crate::db::get_track_by_id(track_id, conn)
                    .ok()
                    .map(|track| DownloadedTrackInfo {
                        id: track.id,
                        title: track.title,
                        artist: track.artist_name,
                        album: Some(track.album_name),
                        duration: Some(track.duration),
                    })
            })
        }
        None => None,
    };

    // The track starts playing when the user asked for it. The queue is the
    // library, so that "next" moves on to the song after this one rather
    // than to nothing.
    if play_now.unwrap_or(false) {
        if let Some(track) = &track {
            let queue: Vec<i64> = {
                let conn_guard = app_state.db.lock().ok();
                let conn = conn_guard.as_ref().and_then(|guard| guard.as_ref());
                conn.and_then(|conn| {
                    crate::db::get_track_ids(true, true, true, true, conn).ok()
                })
                    .unwrap_or_default()
            };
            let index = queue.iter().position(|id| *id == track.id);

            let _ = crate::gui::play_track_command(
                Some(track.id),
                None,
                None,
                None,
                None,
                None,
                track.duration,
                if queue.is_empty() { None } else { Some(queue) },
                index,
                &app_handle,
            )
            .await;
        }
    }

    Ok(YoutubeDownloadResult {
        path: downloaded.path.display().to_string(),
        title: downloaded.info.title,
        album,
        artist: downloaded.info.artist,
        album_corrected,
        cover_corrected,
        track_id,
        lyrics_found,
        lyrics_note,
        track,
    })
}


/// Add a downloaded file to the library.
///
/// The file is added on its own rather than by a scan. A scan cannot be
/// aimed at one directory: it marks every track as pending and then deletes
/// the ones it did not see, so scanning the download folder would empty the
/// rest of the library. This adds one track and touches nothing else.
async fn register_in_library(
    app_state: &tauri::State<'_, crate::state::AppState>,
    app_handle: &tauri::AppHandle,
    path: &Path,
) -> Option<i64> {
    let cover_cache_dir = app_handle
        .path()
        .app_data_dir()
        .map(|dir| dir.join("covers"))
        .unwrap_or_else(|_| std::env::temp_dir().join("sing-along-covers"));

    let mut conn_guard = app_state.db.lock().ok()?;
    let conn = conn_guard.as_mut()?;

    // The insert reads the file and hashes it, so it runs on a thread that
    // can afford to wait.
    tokio::task::block_in_place(|| crate::db::add_single_track(path, conn, &cover_cache_dir)).ok()
}

/// Fetch the lyrics for a track, the same way the lyric download does.
///
/// Returns whether lyrics were found, and a note when they were not. A note
/// rather than an error: a missing lyric is an ordinary outcome, not a
/// failure of the download.
async fn fetch_lyrics_for(
    track_id: i64,
    app_handle: &tauri::AppHandle,
    script: crate::script_convert::ScriptTarget,
) -> (bool, Option<String>) {
    let script = match script {
        crate::script_convert::ScriptTarget::Off => None,
        crate::script_convert::ScriptTarget::Simplified => Some("simplified".to_owned()),
        crate::script_convert::ScriptTarget::Traditional => Some("traditional".to_owned()),
    };

    match crate::gui::download_lyrics_for_track(track_id, None, script, app_handle).await {
        Ok(_) => (true, None),
        Err(error) => {
            let note = if error.contains("no timed lyrics") {
                "no timed lyrics were found for this song".to_owned()
            } else if error.contains("not found") {
                "no lyrics were found for this song".to_owned()
            } else {
                error
            };
            (false, Some(note))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The script the metadata ends up in.
    ///
    /// The answer the user gave wins, and the interface language is the
    /// fallback. The fallback matters: YouTube publishes in whatever script
    /// the uploader chose, so a Simplified-Chinese reader would otherwise
    /// get a file whose title they cannot type back into a search.
    #[test]
    fn the_interface_language_decides_when_the_user_has_not_answered() {
        assert_eq!(
            script_target(Some("ask"), "zh-Hans"),
            crate::script_convert::ScriptTarget::Simplified
        );
        assert_eq!(
            script_target(Some("ask"), "zh-Hant"),
            crate::script_convert::ScriptTarget::Traditional
        );
        assert_eq!(
            script_target(None, "zh-Hans"),
            crate::script_convert::ScriptTarget::Simplified
        );
    }

    #[test]
    fn a_language_without_chinese_leaves_the_text_alone() {
        assert_eq!(
            script_target(Some("ask"), "en"),
            crate::script_convert::ScriptTarget::Off
        );
        assert_eq!(
            script_target(Some("ask"), "ja"),
            crate::script_convert::ScriptTarget::Off
        );
    }

    #[test]
    fn the_users_answer_beats_the_interface_language() {
        // A Traditional-Chinese reader who asks for Simplified lyrics gets
        // Simplified lyrics, even though the interface is in Traditional.
        assert_eq!(
            script_target(Some("simplified"), "zh-Hant"),
            crate::script_convert::ScriptTarget::Simplified
        );
        assert_eq!(
            script_target(Some("traditional"), "zh-Hans"),
            crate::script_convert::ScriptTarget::Traditional
        );
        assert_eq!(
            script_target(Some("off"), "zh-Hans"),
            crate::script_convert::ScriptTarget::Off
        );
    }
}

/// The library entries whose file is not on the disk, and the pairs that look
/// like the same recording.
///
/// The report is not a verdict. The duplicates are grouped by how sure the
/// library is, because the certainty is what decides whether a pair is worth
/// removing: two files with the same content are the same audio, while two
/// recordings of the same song are not the same thing and a library that
/// threw one away would lose a track.
#[tauri::command]
pub async fn get_library_problems(
    app_state: tauri::State<'_, crate::state::AppState>,
) -> Result<LibraryProblems, String> {
    let conn_guard = app_state.db.lock().map_err(|err| err.to_string())?;
    let conn = conn_guard.as_ref().unwrap();

    let missing = crate::db::tracks_with_missing_files(conn).map_err(|err| err.to_string())?;
    let duplicates = crate::db::find_duplicate_tracks(conn).map_err(|err| err.to_string())?;

    Ok(LibraryProblems {
        missing: missing
            .into_iter()
            .map(|track| MissingTrack {
                id: track.id,
                title: track.title,
                artist: track.artist_name,
                album: track.album_name,
                file_path: track.file_path,
                has_lyrics: crate::db::get_track_lyricsfile(track.id, conn)
                    .ok()
                    .flatten()
                    .is_some(),
            })
            .collect(),
        duplicates,
    })
}

/// What is wrong with the library.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryProblems {
    /// The entries whose file is not on the disk.
    pub missing: Vec<MissingTrack>,
    /// The pairs that look like the same recording.
    pub duplicates: Vec<crate::db::DuplicatePair>,
}

/// A library entry whose file is not on the disk.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingTrack {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub file_path: String,
    pub has_lyrics: bool,
}

/// Remove the library entries the user picked.
#[tauri::command]
pub async fn remove_library_entries(
    track_ids: Vec<i64>,
    app_state: tauri::State<'_, crate::state::AppState>,
) -> Result<usize, String> {
    let mut conn_guard = app_state.db.lock().map_err(|err| err.to_string())?;
    let conn = conn_guard.as_mut().unwrap();

    let tx = conn.transaction().map_err(|err| err.to_string())?;
    let mut removed = 0;

    for id in &track_ids {
        // The lyrics of a track that is gone are gone too, unless they are
        // also kept as a standalone lyricsfile.
        tx.execute("DELETE FROM lyricsfiles WHERE track_id = ?", [id])
            .map_err(|err| err.to_string())?;
        let deleted = tx
            .execute("DELETE FROM tracks WHERE id = ?", [id])
            .map_err(|err| err.to_string())?;
        if deleted > 0 {
            removed += 1;
        }
    }

    tx.execute(
        "DELETE FROM albums WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks)",
        [],
    )
    .map_err(|err| err.to_string())?;

    tx.commit().map_err(|err| err.to_string())?;
    Ok(removed)
}
