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
}

/// The settings that decide how the downloader is called, read from the
/// configuration.
fn download_options(
    config: &PersistentConfig,
    output_dir: Option<PathBuf>,
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
        script: crate::script_convert::ScriptTarget::parse(&config.lyric_script)
            .unwrap_or(crate::script_convert::ScriptTarget::Off),
        fix_metadata: true,
    }
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
    app_state: tauri::State<'_, crate::state::AppState>,
) -> Result<YoutubeTrackInfo, String> {
    // The lock is taken and released inside a block, so that no guard is held
    // across an await: a guard is not `Send`, and a command that holds one
    // across an await cannot be called from the main thread.
    let config = {
        let conn_guard = app_state.db.lock().unwrap();
        let conn = conn_guard.as_ref().unwrap();
        crate::db::get_config(conn).map_err(|err| err.to_string())?
    };

    let options = download_options(&config, None);
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

    Ok(YoutubeTrackInfo {
        id: info.id,
        title: info.title,
        album: info.album,
        artist: info.artist,
        duration: info.duration,
        corrected_album,
        corrected_cover,
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
    app_state: tauri::State<'_, crate::state::AppState>,
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

    let mut options = download_options(&config, Some(output_dir));

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

        if let Ok(Some(correction)) = crate::youtube::lookup(
            &client,
            &downloaded.info.title,
            downloaded.info.artist.as_deref().unwrap_or_default(),
        )
        .await
        {
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

        // The script conversion comes last, so it also converts the album
        // name that was just written.
        let _ = crate::youtube::convert_file_script(&downloaded.path, options.script);
    }

    Ok(YoutubeDownloadResult {
        path: downloaded.path.display().to_string(),
        title: downloaded.info.title,
        album,
        artist: downloaded.info.artist,
        album_corrected,
        cover_corrected,
    })
}

/// Where a file is, for the GUI to show.
pub fn file_path_for(path: &Path) -> String {
    path.display().to_string()
}
