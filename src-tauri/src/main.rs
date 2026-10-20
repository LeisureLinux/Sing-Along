#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

pub mod db;
pub mod export;
pub mod library;
pub mod lrclib;
pub mod netease;
pub mod lyricsfile;
pub mod parser;
pub mod persistent_entities;
pub mod playlist;
pub mod player;
pub mod scanner;
pub mod script_convert;
pub mod cover;
pub mod cover_cache;
pub mod state;
pub mod utils;
pub mod word_segmentation;
pub mod word_sync;

use persistent_entities::{PersistentAlbum, PersistentArtist, PersistentConfig, PersistentTrack, PlayableTrack};
use player::Player;
use playlist::{PlaybackDecision, RepeatMode};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use state::{AppState, Notify, NotifyType, ServiceAccess};
use tauri::{AppHandle, Emitter, Manager, State};

struct ResolvedLyricsPayload {
    plain_lyrics: String,
    synced_lyrics: String,
    is_instrumental: bool,
    provided_lyricsfile: Option<String>,
}

const LRCLIB_TRACK_NOT_FOUND: &str = "This track does not exist in LRCLIB database";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PublishLyricsProgress {
    request_challenge: String,
    solve_challenge: String,
    publish_lyrics: String,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ExportLyricsFormat {
    Txt,
    Lrc,
    #[serde(rename = "lrcA2")]
    LrcA2,
    Ttml,
    Embedded,
}

/// Explicit format selection captured when a Download batch is submitted.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AutoExportOptions {
    plain_text: bool,
    synced_lrc: bool,
    #[serde(default)]
    embed_into_track: bool,
}

impl AutoExportOptions {
    fn formats(self) -> Result<Vec<export::ExportFormat>, String> {
        let mut formats = Vec::new();
        if self.plain_text {
            formats.push(export::ExportFormat::Txt);
        }
        if self.synced_lrc {
            formats.push(export::ExportFormat::Lrc);
        }
        if self.embed_into_track {
            formats.push(export::ExportFormat::Embedded);
        }
        if formats.is_empty() {
            return Err("Select at least one export format".to_owned());
        }
        Ok(formats)
    }
}

/// Match quality for track matching results
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
enum MatchQuality {
    Strong,
    Partial,
}

/// Track matching result with quality information
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MatchingTrack {
    #[serde(flatten)]
    track: PersistentTrack,
    match_quality: MatchQuality,
}

/// Audio metadata extracted from a file (for file picker)
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AudioMetadataResponse {
    pub file_path: String,
    pub file_name: String,
    pub title: String,
    pub album: String,
    pub artist: String,
    pub album_artist: String,
    pub duration: f64,
    pub track_number: Option<u32>,
}

impl From<ExportLyricsFormat> for export::ExportFormat {
    fn from(value: ExportLyricsFormat) -> Self {
        match value {
            ExportLyricsFormat::Txt => export::ExportFormat::Txt,
            ExportLyricsFormat::Lrc => export::ExportFormat::Lrc,
            ExportLyricsFormat::LrcA2 => export::ExportFormat::LrcA2,
            ExportLyricsFormat::Ttml => export::ExportFormat::Ttml,
            ExportLyricsFormat::Embedded => export::ExportFormat::Embedded,
        }
    }
}

fn resolve_lrclib_lyrics_payload(
    lrclib_response: lrclib::get::RawResponse,
) -> Result<ResolvedLyricsPayload, String> {
    let provided_lyricsfile = lrclib_response
        .lyricsfile
        .clone()
        .filter(|content| !content.trim().is_empty());

    if let Some(lyricsfile_content) = provided_lyricsfile {
        let parsed =
            lyricsfile::parse_lyricsfile(&lyricsfile_content).map_err(|err| err.to_string())?;
        let plain_lyrics = parsed.plain_lyrics.unwrap_or_default();
        let synced_lyrics = parsed.synced_lyrics.unwrap_or_default();
        let is_instrumental = parsed.is_instrumental;

        if !is_instrumental && plain_lyrics.trim().is_empty() && synced_lyrics.trim().is_empty() {
            return Err(LRCLIB_TRACK_NOT_FOUND.to_owned());
        }

        return Ok(ResolvedLyricsPayload {
            plain_lyrics,
            synced_lyrics,
            is_instrumental,
            provided_lyricsfile: Some(lyricsfile_content),
        });
    }

    match lrclib::get::Response::from_raw_response(lrclib_response) {
        lrclib::get::Response::SyncedLyrics(synced_lyrics, plain_lyrics) => {
            Ok(ResolvedLyricsPayload {
                plain_lyrics,
                synced_lyrics,
                is_instrumental: false,
                provided_lyricsfile: None,
            })
        }
        lrclib::get::Response::UnsyncedLyrics(plain_lyrics) => Ok(ResolvedLyricsPayload {
            plain_lyrics,
            synced_lyrics: String::new(),
            is_instrumental: false,
            provided_lyricsfile: None,
        }),
        lrclib::get::Response::IsInstrumental => Ok(ResolvedLyricsPayload {
            plain_lyrics: String::new(),
            synced_lyrics: lyricsfile::INSTRUMENTAL_LRC.to_owned(),
            is_instrumental: true,
            provided_lyricsfile: None,
        }),
        lrclib::get::Response::None => Err(LRCLIB_TRACK_NOT_FOUND.to_owned()),
    }
}

#[tauri::command]
async fn get_directories(app_state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let conn_guard = app_state.db.lock().unwrap();
    let conn = conn_guard.as_ref().unwrap();
    let directories = db::get_directories(conn);
    match directories {
        Ok(directories) => Ok(directories),
        Err(error) => Err(format!(
            "Cannot get existing directories from database. Error: {}",
            error
        )),
    }
}

#[tauri::command]
async fn set_directories(
    directories: Vec<String>,
    app_state: State<'_, AppState>,
) -> Result<(), String> {
    let conn_guard = app_state.db.lock().unwrap();
    let conn = conn_guard.as_ref().unwrap();
    db::set_directories(directories, conn).map_err(|err| err.to_string())?;

    Ok(())
}

#[tauri::command]
async fn get_init(app_state: State<'_, AppState>) -> Result<bool, String> {
    let conn_guard = app_state.db.lock().unwrap();
    let conn = conn_guard.as_ref().unwrap();
    let init = library::get_init(conn).map_err(|err| err.to_string())?;

    Ok(init)
}

#[tauri::command]
async fn get_config(app_state: State<'_, AppState>) -> Result<PersistentConfig, String> {
    let conn_guard = app_state.db.lock().unwrap();
    let conn = conn_guard.as_ref().unwrap();
    let config = db::get_config(conn).map_err(|err| err.to_string())?;

    Ok(config)
}

#[tauri::command]
async fn set_config(
    skip_tracks_with_synced_lyrics: bool,
    skip_tracks_with_plain_lyrics: bool,
    show_line_count: bool,
    try_embed_lyrics: bool,
    theme_mode: &str,
    lrclib_instance: &str,
    volume: f64,
    sync_cover: bool,
    app_state: State<'_, AppState>,
) -> Result<(), String> {
    let conn_guard = app_state.db.lock().unwrap();
    let conn = conn_guard.as_ref().unwrap();
    db::set_config(
        skip_tracks_with_synced_lyrics,
        skip_tracks_with_plain_lyrics,
        show_line_count,
        try_embed_lyrics,
        theme_mode,
        lrclib_instance,
        volume,
        sync_cover,
        conn,
    )
    .map_err(|err| err.to_string())?;

    Ok(())
}

/// Store whether downloaded lyrics should be converted between Chinese scripts.
#[tauri::command]
fn set_lyric_script(script: String, app_state: State<'_, AppState>) -> Result<(), String> {
    let allowed = ["ask", "off", "simplified", "traditional"];
    if !allowed.contains(&script.as_str()) {
        return Err(format!("unsupported lyric script setting: {script}"));
    }

    let conn_guard = app_state.db.lock().map_err(|e| e.to_string())?;
    let conn = conn_guard.as_ref().ok_or("database not initialised")?;
    db::set_lyric_script(&script, conn).map_err(|err| err.to_string())
}

#[tauri::command]
async fn set_export_preferences(
    auto_export_enabled: Option<bool>,
    export_lrc: Option<bool>,
    export_txt: Option<bool>,
    export_embedded: Option<bool>,
    skip_tracks_with_synced_lyrics: Option<bool>,
    skip_tracks_with_plain_lyrics: Option<bool>,
    app_state: State<'_, AppState>,
) -> Result<PersistentConfig, String> {
    let conn_guard = app_state.db.lock().unwrap();
    let conn = conn_guard.as_ref().unwrap();
    db::set_export_preferences(
        auto_export_enabled,
        export_lrc,
        export_txt,
        export_embedded,
        skip_tracks_with_synced_lyrics,
        skip_tracks_with_plain_lyrics,
        conn,
    )
    .map_err(|err| err.to_string())?;
    db::get_config(conn).map_err(|err| err.to_string())
}

#[tauri::command]
async fn uninitialize_library(app_state: State<'_, AppState>) -> Result<(), String> {
    let conn_guard = app_state.db.lock().unwrap();
    let conn = conn_guard.as_ref().unwrap();

    library::uninitialize_library(conn).map_err(|err| err.to_string())?;

    Ok(())
}

/// Full wipe and rescan of the library.
/// Clears tracks, albums, artists tables, resets init flag, and performs a full rescan.
/// Associated lyricsfiles are preserved (track_id set to NULL) for potential reattachment.
#[tauri::command]
async fn full_scan_library(
    app_state: State<'_, AppState>,
    app_handle: AppHandle,
    use_hash_detection: Option<bool>,
) -> Result<scanner::models::ScanResult, String> {
    // Step 1: Full wipe - clear all library data and reset init flag
    {
        let conn_guard = app_state.db.lock().unwrap();
        let conn = conn_guard.as_ref().unwrap();
        library::full_wipe_library(conn).map_err(|err| err.to_string())?;
    }

    // Step 2: Get directories
    let directories = {
        let conn_guard = app_state.db.lock().unwrap();
        let conn = conn_guard.as_ref().unwrap();
        db::get_directories(conn).map_err(|err| err.to_string())?
    };

    // Determine detection method (default to Hash for reliability)
    let detection_method = if use_hash_detection.unwrap_or(true) {
        scanner::scan::DetectionMethod::Hash
    } else {
        scanner::scan::DetectionMethod::Metadata
    };

    // Clone app_handle for use in the closure
    let app_handle_clone = app_handle.clone();

    // Step 3: Run full scan
    let scan_result = tokio::task::block_in_place(|| {
        let mut conn_guard = app_state.db.lock().unwrap();
        let conn = conn_guard.as_mut().unwrap();

        scanner::scan_library(
            &directories,
            conn,
            &|progress| {
                // Emit progress directly (synchronous)
                let _ = app_handle_clone.emit("scan-progress", progress);
            },
            detection_method,
            &scanner_cover_dir(&app_handle),
        )
    })
    .map_err(|err| err.to_string())?;

    // Emit completion event
    let _ = app_handle.emit("scan-complete", &scan_result);

    Ok(scan_result)
}

#[tauri::command]
async fn scan_library(
    app_state: State<'_, AppState>,
    app_handle: AppHandle,
    use_hash_detection: Option<bool>,
) -> Result<scanner::models::ScanResult, String> {
    // Get directories first (requires immutable access)
    let directories = {
        let conn_guard = app_state.db.lock().unwrap();
        let conn = conn_guard.as_ref().unwrap();
        db::get_directories(conn).map_err(|err| err.to_string())?
    };

    // Determine detection method (default to Hash for reliability)
    let detection_method = if use_hash_detection.unwrap_or(true) {
        scanner::scan::DetectionMethod::Hash
    } else {
        scanner::scan::DetectionMethod::Metadata
    };

    // Clone app_handle for use in the closure
    let app_handle_clone = app_handle.clone();

    // Run scan synchronously but use block_in_place to not block the runtime
    let scan_result = tokio::task::block_in_place(|| {
        let mut conn_guard = app_state.db.lock().unwrap();
        let conn = conn_guard.as_mut().unwrap();

        scanner::scan_library(
            &directories,
            conn,
            &|progress| {
                // Emit progress directly (synchronous)
                let _ = app_handle_clone.emit("scan-progress", progress);
            },
            detection_method,
            &scanner_cover_dir(&app_handle),
        )
    })
    .map_err(|err| err.to_string())?;

    // Emit completion event
    let _ = app_handle.emit("scan-complete", &scan_result);

    Ok(scan_result)
}

#[tauri::command]
async fn get_tracks(app_state: State<'_, AppState>) -> Result<Vec<PersistentTrack>, String> {
    let conn_guard = app_state.db.lock().unwrap();
    let conn = conn_guard.as_ref().unwrap();
    let tracks = library::get_tracks(conn).map_err(|err| err.to_string())?;

    Ok(tracks)
}

#[tauri::command]
async fn get_track_ids(
    search_query: Option<String>,
    synced_lyrics_tracks: Option<bool>,
    plain_lyrics_tracks: Option<bool>,
    instrumental_tracks: Option<bool>,
    no_lyrics_tracks: Option<bool>,
    app_state: State<'_, AppState>,
) -> Result<Vec<i64>, String> {
    let conn_guard = app_state.db.lock().unwrap();
    let conn = conn_guard.as_ref().unwrap();
    let search_query = search_query.filter(|s| !s.is_empty());
    let track_ids = library::get_track_ids(
        search_query,
        synced_lyrics_tracks.unwrap_or(true),
        plain_lyrics_tracks.unwrap_or(true),
        instrumental_tracks.unwrap_or(true),
        no_lyrics_tracks.unwrap_or(true),
        conn,
    )
    .map_err(|err| err.to_string())?;

    Ok(track_ids)
}

#[tauri::command]
async fn get_track(
    track_id: i64,
    app_state: State<'_, AppState>,
) -> Result<PersistentTrack, String> {
    let conn_guard = app_state.db.lock().unwrap();
    let conn = conn_guard.as_ref().unwrap();
    let track = library::get_track(track_id, conn).map_err(|err| err.to_string())?;

    Ok(track)
}

#[tauri::command]
async fn find_matching_tracks(
    title: String,
    album_name: String,
    artist_name: String,
    duration: Option<f64>,
    app_state: State<'_, AppState>,
) -> Result<Vec<MatchingTrack>, String> {
    let conn_guard = app_state.db.lock().unwrap();
    let conn = conn_guard.as_ref().unwrap();

    // First, try to find tracks with all criteria (strong match)
    let strong_matches = db::find_tracks_by_metadata(
        &title,
        Some(&artist_name),
        Some(&album_name),
        duration,
        conn,
    )
    .map_err(|err| err.to_string())?;

    // If we have strong matches, return them
    if !strong_matches.is_empty() {
        return Ok(strong_matches
            .into_iter()
            .map(|track| MatchingTrack {
                track,
                match_quality: MatchQuality::Strong,
            })
            .collect());
    }

    // Otherwise, search for partial matches (title only match with duration if provided)
    let partial_matches =
        db::find_tracks_by_metadata(&title, None, None, duration, conn).map_err(|err| err.to_string())?;

    // Filter out tracks where artist or album don't match at all (still partial but relevant)
    let normalized_artist = utils::prepare_input(&artist_name);
    let normalized_album = utils::prepare_input(&album_name);

    let partial_results: Vec<MatchingTrack> = partial_matches
        .into_iter()
        .map(|track| {
            let track_artist_normalized = utils::prepare_input(&track.artist_name);
            let track_album_normalized = utils::prepare_input(&track.album_name);

            // Check if artist or album matches (case-insensitive via normalization)
            let _artist_matches = track_artist_normalized == normalized_artist;
            let _album_matches = track_album_normalized == normalized_album;

            // It's a partial match if title matches and at least one of artist/album matches
            // or if we have no duration filter and just title matches
            MatchingTrack {
                track,
                match_quality: MatchQuality::Partial,
            }
        })
        .collect();

    Ok(partial_results)
}

#[tauri::command]
async fn get_audio_metadata(file_path: String) -> Result<AudioMetadataResponse, String> {
    let path = std::path::Path::new(&file_path);

    let metadata = scanner::metadata::TrackMetadata::from_path(path)
        .map_err(|err| err.to_string())?;

    Ok(AudioMetadataResponse {
        file_path: metadata.file_path,
        file_name: metadata.file_name,
        title: metadata.title,
        album: metadata.album,
        artist: metadata.artist,
        album_artist: metadata.album_artist,
        duration: metadata.duration,
        track_number: metadata.track_number,
    })
}

#[tauri::command]
async fn prepare_search_query(title: String) -> Result<String, String> {
    Ok(utils::prepare_search_input(&title))
}

#[tauri::command]
async fn get_albums(app_state: State<'_, AppState>) -> Result<Vec<PersistentAlbum>, String> {
    let conn_guard = app_state.db.lock().unwrap();
    let conn = conn_guard.as_ref().unwrap();
    let albums = library::get_albums(conn).map_err(|err| err.to_string())?;

    Ok(albums)
}

#[tauri::command]
async fn get_album_ids(
    search_query: Option<String>,
    app_state: State<'_, AppState>,
) -> Result<Vec<i64>, String> {
    let conn_guard = app_state.db.lock().unwrap();
    let conn = conn_guard.as_ref().unwrap();
    let search_query = search_query.filter(|s| !s.is_empty());
    let album_ids = library::get_album_ids(search_query, conn).map_err(|err| err.to_string())?;

    Ok(album_ids)
}

/// Extract covers for albums that do not have one yet.
///
/// Separate from `scan_library` because a rescan deliberately skips unchanged
/// tracks, so a library built before covers were extracted would never gain any.
/// Called on demand and safe to repeat: albums that already have a cover are
/// not touched.
#[tauri::command]
async fn backfill_album_covers(
    app_handle: AppHandle,
    app_state: State<'_, AppState>,
) -> Result<usize, String> {
    let albums = {
        let conn_guard = app_state.db.lock().unwrap();
        let conn = conn_guard.as_ref().unwrap();
        db::albums_without_cover(conn).map_err(|err| err.to_string())?
    };

    let cache_dir = scanner_cover_dir(&app_handle);
    let mut filled = 0usize;

    for (album_id, track_path) in albums {
        let Some(bytes) = crate::cover_cache::cover_for_track(std::path::Path::new(&track_path))
        else {
            continue;
        };
        let key = format!("album-{album_id}");
        let Some(file_name) = crate::cover_cache::store_cover(&cache_dir, &key, &bytes) else {
            continue;
        };

        let written = {
            let conn_guard = app_state.db.lock().unwrap();
            let conn = conn_guard.as_ref().unwrap();
            db::set_album_image(album_id, &file_name, conn).map_err(|err| err.to_string())?;
            true
        };

        if written {
            filled += 1;
        }
    }

    Ok(filled)
}

/// Absolute path of an album's cached cover, or `None` when it has none.
///
/// `PersistentAlbum.image_path` holds only the cached file name, so the UI needs
/// this to build a loadable URL. Kept separate from `get_album` because album
/// rows are fetched in bulk for the list; resolving a path for every row would
/// be wasted work when only the detail view shows the large image.
#[tauri::command]
async fn get_album_cover(
    album_id: i64,
    app_handle: AppHandle,
    app_state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let image_name = {
        let conn_guard = app_state.db.lock().unwrap();
        let conn = conn_guard.as_ref().unwrap();
        let album = library::get_album(album_id, conn).map_err(|err| err.to_string())?;
        album.image_path.filter(|name| !name.trim().is_empty())
    };

    let Some(name) = image_name else {
        return Ok(None);
    };

    crate::cover_cache::cached_cover_data_url(&scanner_cover_dir(&app_handle), &name)
        .map(Some)
        .map_err(|err| err.to_string())
}

#[tauri::command]
async fn get_album(
    album_id: i64,
    app_state: State<'_, AppState>,
) -> Result<PersistentAlbum, String> {
    let conn_guard = app_state.db.lock().unwrap();
    let conn = conn_guard.as_ref().unwrap();
    let album = library::get_album(album_id, conn).map_err(|err| err.to_string())?;

    Ok(album)
}

#[tauri::command]
async fn get_artists(app_state: State<'_, AppState>) -> Result<Vec<PersistentArtist>, String> {
    let conn_guard = app_state.db.lock().unwrap();
    let conn = conn_guard.as_ref().unwrap();
    let artists = library::get_artists(conn).map_err(|err| err.to_string())?;

    Ok(artists)
}

#[tauri::command]
async fn get_artist_ids(
    search_query: Option<String>,
    app_state: State<'_, AppState>,
) -> Result<Vec<i64>, String> {
    let conn_guard = app_state.db.lock().unwrap();
    let conn = conn_guard.as_ref().unwrap();
    let search_query = search_query.filter(|s| !s.is_empty());
    let artist_ids = library::get_artist_ids(search_query, conn).map_err(|err| err.to_string())?;

    Ok(artist_ids)
}

#[tauri::command]
async fn get_artist(
    artist_id: i64,
    app_state: State<'_, AppState>,
) -> Result<PersistentArtist, String> {
    let conn_guard = app_state.db.lock().unwrap();
    let conn = conn_guard.as_ref().unwrap();
    let artist = library::get_artist(artist_id, conn).map_err(|err| err.to_string())?;

    Ok(artist)
}

#[tauri::command]
async fn get_album_tracks(
    album_id: i64,
    app_state: State<'_, AppState>,
) -> Result<Vec<PersistentTrack>, String> {
    let conn_guard = app_state.db.lock().unwrap();
    let conn = conn_guard.as_ref().unwrap();
    let tracks = library::get_album_tracks(album_id, conn).map_err(|err| err.to_string())?;

    Ok(tracks)
}

#[tauri::command]
async fn get_artist_tracks(
    artist_id: i64,
    app_state: State<'_, AppState>,
) -> Result<Vec<PersistentTrack>, String> {
    let conn_guard = app_state.db.lock().unwrap();
    let conn = conn_guard.as_ref().unwrap();
    let tracks = library::get_artist_tracks(artist_id, conn).map_err(|err| err.to_string())?;

    Ok(tracks)
}

#[tauri::command]
async fn get_album_track_ids(
    album_id: i64,
    without_plain_lyrics: Option<bool>,
    without_synced_lyrics: Option<bool>,
    app_state: State<'_, AppState>,
) -> Result<Vec<i64>, String> {
    let conn_guard = app_state.db.lock().unwrap();
    let conn = conn_guard.as_ref().unwrap();
    let track_ids = library::get_album_track_ids(
        album_id,
        without_plain_lyrics.unwrap_or(false),
        without_synced_lyrics.unwrap_or(false),
        conn,
    )
    .map_err(|err| err.to_string())?;

    Ok(track_ids)
}

#[tauri::command]
async fn get_artist_track_ids(
    artist_id: i64,
    without_plain_lyrics: Option<bool>,
    without_synced_lyrics: Option<bool>,
    app_state: State<'_, AppState>,
) -> Result<Vec<i64>, String> {
    let conn_guard = app_state.db.lock().unwrap();
    let conn = conn_guard.as_ref().unwrap();
    let track_ids = library::get_artist_track_ids(
        artist_id,
        without_plain_lyrics.unwrap_or(false),
        without_synced_lyrics.unwrap_or(false),
        conn,
    )
    .map_err(|err| err.to_string())?;

    Ok(track_ids)
}

#[tauri::command]
async fn download_lyrics(
    track_id: i64,
    auto_export: Option<AutoExportOptions>,
    lyric_script: Option<String>,
    app_handle: AppHandle,
) -> Result<String, String> {
    let export_formats = auto_export.map(AutoExportOptions::formats).transpose()?;
    let track = app_handle
        .db(|db| db::get_track_by_id(track_id, db))
        .map_err(|err| err.to_string())?;
    let config = app_handle
        .db(|db| db::get_config(db))
        .map_err(|err| err.to_string())?;
    // LRCLIB first. If it has no lyrics for this track and the NetEase fallback
    // is enabled, try NetEase before giving up. A NetEase failure of any kind is
    // logged and swallowed: a broken fallback must not turn an LRCLIB outage into
    // an error the user cannot act on.
    let lrclib_result = lrclib::get::request_raw(
        &track.title,
        &track.album_name,
        &track.artist_name,
        track.duration,
        &config.lrclib_instance,
    )
    .await;

    let (lrclib_response, netease_note) = match lrclib_result {
        Ok(response) => (response, String::new()),
        Err(lrclib_error) => {
            if !config.netease_fallback_enabled {
                return Err(lrclib_error.to_string());
            }

            match netease::fetch_lyrics(
                &netease_client(),
                &track.title,
                &track.artist_name,
                Some(track.duration),
            )
            .await
            {
                Ok(Some(netease)) => (
                    lrclib::get::RawResponse::from_synced_lyrics(
                        netease.synced_lyrics,
                        &track.title,
                        &track.album_name,
                        &track.artist_name,
                        track.duration,
                    ),
                    format!(" (via NetEase #{})", netease.song_id),
                ),
                Ok(None) => {
                    // NetEase searched and found nothing: report the original
                    // LRCLIB failure, which is the truthful one.
                    return Err(lrclib_error.to_string());
                }
                Err(netease_error) => {
                    eprintln!("NetEase fallback failed: {netease_error}");
                    return Err(lrclib_error.to_string());
                }
            }
        }
    };
    let resolved = resolve_lrclib_lyrics_payload(lrclib_response)?;

    let message = if resolved.is_instrumental {
        "Marked track as instrumental"
    } else if !resolved.synced_lyrics.is_empty() {
        "Synced lyrics downloaded"
    } else if !resolved.plain_lyrics.is_empty() {
        "Plain lyrics downloaded"
    } else {
        return Err(LRCLIB_TRACK_NOT_FOUND.to_owned());
    };
    let message = format!("{message}{netease_note}");

    // Build lyricsfile content from the resolved response
    let lyricsfile_content = if let Some(ref provided) = resolved.provided_lyricsfile {
        provided.clone()
    } else {
        // Build a lyricsfile from the plain/synced lyrics
        crate::lyricsfile::build_lyricsfile(
            &crate::lyricsfile::LyricsfileTrackMetadata::new(
                &track.title,
                &track.album_name,
                &track.artist_name,
                track.duration,
            ),
            Some(&resolved.plain_lyrics),
            Some(&resolved.synced_lyrics),
        )
        .ok_or_else(|| "Failed to build lyricsfile")?
    };

    // The caller may ask for the lyrics in a particular Chinese script. This is
    // applied here, immediately before the write, so that what reaches the
    // database and any auto-exported sidecar is already converted — converting
    // afterwards would leave the two out of step.
    let lyricsfile_content = match lyric_script.as_deref() {
        Some(target) if target != "off" && crate::script_convert::has_cjk(&lyricsfile_content) => {
            let script = crate::script_convert::ScriptTarget::parse(target)
                .ok_or_else(|| format!("unsupported lyric script: {target}"))?;
            crate::script_convert::convert(&lyricsfile_content, script)
        }
        _ => lyricsfile_content,
    };

    // Upsert the lyricsfile record (handles presence fields automatically)
    app_handle
        .db(|db: &Connection| {
            db::upsert_lyricsfile_for_track(
                track.id,
                &track.title,
                &track.album_name,
                &track.artist_name,
                track.duration,
                &lyricsfile_content,
                db,
            )
        })
        .map_err(|err| err.to_string())?;

    let _ = app_handle.emit("reload-track-id", track_id);

    // Internal save has succeeded: export errors remain part of the success log string.
    Ok(match export_formats {
        Some(formats) => download_export_message(&message, &track, &lyricsfile_content, &formats, || {
            embedding_allowed(&app_handle)
        }),
        None => message.to_owned(),
    })
}

fn download_export_message(
    message: &str,
    track: &PersistentTrack,
    content: &str,
    formats: &[export::ExportFormat],
    embed_allowed: impl FnMut() -> bool,
) -> String {
    let parsed = match lyricsfile::parse_lyricsfile(content) {
        Ok(parsed) => parsed,
        Err(err) => return format!("{message}; export failed: {err}"),
    };
    let mut message = message.to_owned();
    for result in export::export_track_with_embed_gate(track, &parsed, formats, embed_allowed) {
        let format = match result.format {
            export::ExportFormat::Txt => "TXT",
            export::ExportFormat::Lrc => "LRC",
            export::ExportFormat::LrcA2 => "A2",
            export::ExportFormat::Ttml => "TTML",
            export::ExportFormat::Embedded => "Embedded",
        };
        let outcome = match result.status {
            export::ExportStatus::Success => "export completed".to_owned(),
            export::ExportStatus::Skipped(reason) => format!("export skipped: {reason}"),
            export::ExportStatus::Error(reason) => format!("export failed: {reason}"),
        };
        message.push_str(&format!("; {format} {outcome}"));
    }
    message
}

// Recheck immediately before each embedded write, including after a network download.
fn embedding_allowed(app_handle: &AppHandle) -> bool {
    app_handle
        .db(db::get_config)
        .map(|config| config.try_embed_lyrics)
        .unwrap_or(false)
}

#[tauri::command]
async fn apply_lyrics(
    track_id: i64,
    lrclib_response: lrclib::get::RawResponse,
    app_handle: AppHandle,
) -> Result<String, String> {
    let track = app_handle
        .db(|db| db::get_track_by_id(track_id, db))
        .map_err(|err| err.to_string())?;

    let resolved = resolve_lrclib_lyrics_payload(lrclib_response)?;

    // Build lyricsfile content from the resolved response
    let lyricsfile_content = if let Some(ref provided) = resolved.provided_lyricsfile {
        provided.clone()
    } else {
        // Build a lyricsfile from the plain/synced lyrics
        crate::lyricsfile::build_lyricsfile(
            &crate::lyricsfile::LyricsfileTrackMetadata::new(
                &track.title,
                &track.album_name,
                &track.artist_name,
                track.duration,
            ),
            Some(&resolved.plain_lyrics),
            Some(&resolved.synced_lyrics),
        )
        .ok_or_else(|| "Failed to build lyricsfile")?
    };

    // Apply the configured Chinese script conversion. Picking a result in the
    // search dialog is the same user intent as downloading it, so it goes
    // through the same conversion step; without this, a track set to Simplified
    // would silently get Traditional lyrics from a manual search.
    let lyricsfile_content = {
        let configured = app_handle
            .db(|db: &Connection| db::get_config(db).map(|config| config.lyric_script))
            .unwrap_or_else(|_| "off".to_owned());
        match configured.as_str() {
            script if script != "off" && crate::script_convert::has_cjk(&lyricsfile_content) => {
                match crate::script_convert::ScriptTarget::parse(script) {
                    Some(target) => crate::script_convert::convert(&lyricsfile_content, target),
                    None => lyricsfile_content,
                }
            }
            _ => lyricsfile_content,
        }
    };

    // Upsert the lyricsfile record (handles presence fields automatically)
    app_handle
        .db(|db: &Connection| {
            db::upsert_lyricsfile_for_track(
                track.id,
                &track.title,
                &track.album_name,
                &track.artist_name,
                track.duration,
                &lyricsfile_content,
                db,
            )
        })
        .map_err(|err| err.to_string())?;

    std::thread::spawn(move || {
        app_handle.emit("reload-track-id", track_id).unwrap();
    });

    if resolved.is_instrumental {
        Ok("Marked track as instrumental".to_owned())
    } else if !resolved.synced_lyrics.is_empty() {
        Ok("Synced lyrics downloaded".to_owned())
    } else if !resolved.plain_lyrics.is_empty() {
        Ok("Plain lyrics downloaded".to_owned())
    } else {
        Err(LRCLIB_TRACK_NOT_FOUND.to_owned())
    }
}

#[tauri::command]
async fn retrieve_lyrics(
    title: String,
    album_name: String,
    artist_name: String,
    duration: f64,
    app_handle: AppHandle,
) -> Result<lrclib::get::RawResponse, String> {
    let config = app_handle
        .db(|db: &Connection| db::get_config(db))
        .map_err(|err| err.to_string())?;

    let response = lrclib::get::request_raw(
        &title,
        &album_name,
        &artist_name,
        duration,
        &config.lrclib_instance,
    )
    .await
    .map_err(|err| err.to_string())?;

    Ok(response)
}

#[tauri::command]
async fn retrieve_lyrics_by_id(
    id: i64,
    app_handle: AppHandle,
) -> Result<lrclib::get_by_id::RawResponse, String> {
    let config = app_handle
        .db(|db: &Connection| db::get_config(db))
        .map_err(|err| err.to_string())?;

    let response = lrclib::get_by_id::request_raw(id, &config.lrclib_instance)
        .await
        .map_err(|err| err.to_string())?;

    Ok(response)
}

#[tauri::command]
async fn search_lyrics(
    title: String,
    album_name: String,
    artist_name: String,
    q: String,
    app_handle: AppHandle,
) -> Result<lrclib::search::Response, String> {
    let config = app_handle
        .db(|db: &Connection| db::get_config(db))
        .map_err(|err| err.to_string())?;
    let response = lrclib::search::request(
        &title,
        &album_name,
        &artist_name,
        &q,
        &config.lrclib_instance,
    )
    .await
    .map_err(|err| err.to_string())?;

    Ok(response)
}

/// One row in the manual search dialog.
///
/// Both sources are flattened into this shape so the UI can render a single
/// list. The `id` is meaningful only together with `source` — LRCLIB and
/// NetEase number their tracks independently.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchHit {
    source: String,
    id: i64,
    name: String,
    artist_name: String,
    album_name: Option<String>,
    duration: Option<f64>,
    instrumental: bool,
    plain_lyrics: Option<String>,
    synced_lyrics: Option<String>,
}

/// Search lyrics across the sources the user selected.
///
/// A failing source never fails the whole search: LRCLIB being down should not
/// hide NetEase results. The per-source error is reported alongside whatever
/// did come back so the dialog can say which part failed.
#[tauri::command]
async fn search_lyrics_multi(
    keyword: String,
    use_lrclib: bool,
    use_netease: bool,
    app_handle: AppHandle,
) -> Result<SearchOutcome, String> {
    let config = app_handle
        .db(|db: &Connection| db::get_config(db))
        .map_err(|err| err.to_string())?;

    let mut hits: Vec<SearchHit> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    if use_lrclib {
        match lrclib::search::request("", "", "", &keyword, &config.lrclib_instance).await {
            Ok(response) => {
                let items = serde_json::to_value(&response)
                    .ok()
                    .and_then(|v| v.as_array().cloned())
                    .unwrap_or_default();
                for item in items {
                    let get_str = |key: &str| {
                        item.get(key)
                            .and_then(|v| v.as_str())
                            .map(|v| v.to_owned())
                    };
                    let Some(id) = item.get("id").and_then(|v| v.as_i64()) else {
                        continue;
                    };
                    hits.push(SearchHit {
                        source: "lrclib".to_owned(),
                        id,
                        name: get_str("name").unwrap_or_default(),
                        artist_name: get_str("artistName").unwrap_or_default(),
                        album_name: get_str("albumName"),
                        duration: item.get("duration").and_then(|v| v.as_f64()),
                        instrumental: item
                            .get("instrumental")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                        plain_lyrics: get_str("plainLyrics"),
                        synced_lyrics: get_str("syncedLyrics"),
                    });
                }
            }
            Err(error) => errors.push(format!("LRCLIB: {error}")),
        }
    }

    if use_netease {
        match crate::netease::search_songs(&netease_client(), &keyword).await {
            Ok(songs) => {
                // NetEase search returns metadata only; the lyric text is a
                // second request per song. Fetching one at a time keeps the
                // dialog responsive enough and avoids hammering the endpoint.
                for song in songs {
                    let synced = crate::netease::lyrics_for_song(&netease_client(), song.id)
                        .await
                        .ok()
                        .flatten();
                    hits.push(SearchHit {
                        source: "netease".to_owned(),
                        id: song.id,
                        name: song.name,
                        artist_name: song.artists,
                        album_name: None,
                        duration: song.duration_secs,
                        instrumental: false,
                        plain_lyrics: None,
                        synced_lyrics: synced,
                    });
                }
            }
            Err(error) => errors.push(format!("NetEase: {error}")),
        }
    }

    Ok(SearchOutcome { hits, errors })
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchOutcome {
    hits: Vec<SearchHit>,
    errors: Vec<String>,
}

/// Build a lyricsfile from a search hit and return it plus its database id.
///
/// The manual search dialog works with hits that may come from either source.
/// Rather than teach every downstream modal about NetEase, the hit is turned
/// into the same `PrepareLyricsfileResult` shape the LRCLIB path already
/// produces, so preview, editing and association stay source-agnostic.
#[tauri::command]
async fn prepare_search_hit_lyricsfile(
    source: String,
    id: i64,
    name: String,
    artist_name: String,
    album_name: Option<String>,
    duration: Option<f64>,
    app_handle: AppHandle,
) -> Result<PrepareLyricsfileResult, String> {
    let config = app_handle
        .db(|db: &Connection| db::get_config(db))
        .map_err(|err| err.to_string())?;

    // LRCLIB ids are namespaced by instance URL, so delegate to the existing
    // path that knows about the cache lookup and the database key.
    if source == "lrclib" {
        return prepare_lrclib_lyricsfile(id, app_handle).await;
    }

    let synced = crate::netease::lyrics_for_song(&netease_client(), id)
        .await
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "NETEASE_NO_LYRICS".to_owned())?;

    let metadata = lyricsfile::LyricsfileTrackMetadata::new(
        &name,
        album_name.as_deref().unwrap_or_default(),
        &artist_name,
        duration.unwrap_or(0.0),
    );
    let lyricsfile_content = lyricsfile::build_lyricsfile(&metadata, None, Some(&synced))
        .ok_or("Failed to build lyricsfile from NetEase lyrics")?;

    let parsed = lyricsfile::parse_lyricsfile(&lyricsfile_content).map_err(|e| e.to_string())?;

    // NetEase ids are written as negative numbers so they cannot collide with
    // LRCLIB's positive ids in the same cache table.
    let stored = app_handle
        .db(|db: &Connection| {
            db::upsert_lyricsfile_for_lrclib(
                &config.lrclib_instance,
                -id,
                &name,
                album_name.as_deref().unwrap_or_default(),
                &artist_name,
                duration.unwrap_or(0.0),
                &lyricsfile_content,
                db,
            )
        })
        .map_err(|err| err.to_string())?;

    Ok(PrepareLyricsfileResult {
        lyricsfile_id: stored,
        lyricsfile: lyricsfile_content,
        plain_lyrics: parsed.plain_lyrics.unwrap_or_default(),
        synced_lyrics: parsed.synced_lyrics.unwrap_or_default(),
        is_instrumental: parsed.is_instrumental,
        exists_in_db: false,
    })
}

/// Result of preparing a lyricsfile from LRCLIB
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PrepareLyricsfileResult {
    pub lyricsfile_id: i64,
    pub lyricsfile: String,
    pub plain_lyrics: String,
    pub synced_lyrics: String,
    pub is_instrumental: bool,
    pub exists_in_db: bool,
}

#[tauri::command]
async fn prepare_lrclib_lyricsfile(
    lrclib_id: i64,
    app_handle: AppHandle,
) -> Result<PrepareLyricsfileResult, String> {
    // Get config for LRCLIB instance URL
    let config = app_handle
        .db(|db: &Connection| db::get_config(db))
        .map_err(|err| err.to_string())?;

    let lrclib_instance = config.lrclib_instance;

    // Check if we already have this lyricsfile in the database
    let existing = app_handle
        .db(|db: &Connection| {
            db::get_lyricsfile_by_lrclib(&lrclib_instance, lrclib_id, db)
        })
        .map_err(|err| err.to_string())?;

    if let Some((lyricsfile_id, lyricsfile)) = existing {
        // Already exists, parse and return
        let parsed = lyricsfile::parse_lyricsfile(&lyricsfile).map_err(|e| e.to_string())?;
        return Ok(PrepareLyricsfileResult {
            lyricsfile_id,
            lyricsfile,
            plain_lyrics: parsed.plain_lyrics.unwrap_or_default(),
            synced_lyrics: parsed.synced_lyrics.unwrap_or_default(),
            is_instrumental: parsed.is_instrumental,
            exists_in_db: true,
        });
    }

    // Fetch from LRCLIB API
    let lrclib_response = lrclib::get_by_id::request_raw(lrclib_id, &lrclib_instance)
        .await
        .map_err(|err| err.to_string())?;

    // Extract metadata from LRCLIB response
    let title = lrclib_response.name.unwrap_or_default();
    let album_name = lrclib_response.album_name.unwrap_or_default();
    let artist_name = lrclib_response.artist_name.unwrap_or_default();
    let duration = lrclib_response.duration.unwrap_or(0.0);

    // Build or use existing lyricsfile content
    let lyricsfile_content = if let Some(lyricsfile) = lrclib_response.lyricsfile {
        // LRCLIB provided a lyricsfile, use it directly
        lyricsfile
    } else {
        // Need to build lyricsfile from plain/synced lyrics
        let metadata =
            lyricsfile::LyricsfileTrackMetadata::new(&title, &album_name, &artist_name, duration);

        let plain = lrclib_response.plain_lyrics.as_deref();
        let synced = lrclib_response.synced_lyrics.as_deref();

        lyricsfile::build_lyricsfile(&metadata, plain, synced)
            .ok_or("Failed to build lyricsfile from LRCLIB response")?
    };

    // Parse for return values
    let parsed = lyricsfile::parse_lyricsfile(&lyricsfile_content).map_err(|e| e.to_string())?;

    // Save to database (track_id will be NULL)
    let lyricsfile_id = app_handle
        .db(|db: &Connection| {
            db::upsert_lyricsfile_for_lrclib(
                &lrclib_instance,
                lrclib_id,
                &title,
                &album_name,
                &artist_name,
                duration,
                &lyricsfile_content,
                db,
            )
        })
        .map_err(|err| err.to_string())?;

    Ok(PrepareLyricsfileResult {
        lyricsfile_id,
        lyricsfile: lyricsfile_content,
        plain_lyrics: parsed.plain_lyrics.unwrap_or_default(),
        synced_lyrics: parsed.synced_lyrics.unwrap_or_default(),
        is_instrumental: parsed.is_instrumental,
        exists_in_db: false,
    })
}

#[tauri::command]
async fn refresh_lrclib_lyricsfile(
    lrclib_id: i64,
    app_handle: AppHandle,
) -> Result<PrepareLyricsfileResult, String> {
    // Get config for LRCLIB instance URL
    let config = app_handle
        .db(|db: &Connection| db::get_config(db))
        .map_err(|err| err.to_string())?;

    let lrclib_instance = config.lrclib_instance;

    // Fetch fresh data from LRCLIB API (always re-download)
    let lrclib_response = lrclib::get_by_id::request_raw(lrclib_id, &lrclib_instance)
        .await
        .map_err(|err| err.to_string())?;

    // Extract metadata from LRCLIB response
    let title = lrclib_response.name.unwrap_or_default();
    let album_name = lrclib_response.album_name.unwrap_or_default();
    let artist_name = lrclib_response.artist_name.unwrap_or_default();
    let duration = lrclib_response.duration.unwrap_or(0.0);

    // Build or use existing lyricsfile content
    let lyricsfile_content = if let Some(lyricsfile) = lrclib_response.lyricsfile {
        // LRCLIB provided a lyricsfile, use it directly
        lyricsfile
    } else {
        // Need to build lyricsfile from plain/synced lyrics
        let metadata =
            lyricsfile::LyricsfileTrackMetadata::new(&title, &album_name, &artist_name, duration);

        let plain = lrclib_response.plain_lyrics.as_deref();
        let synced = lrclib_response.synced_lyrics.as_deref();

        lyricsfile::build_lyricsfile(&metadata, plain, synced)
            .ok_or("Failed to build lyricsfile from LRCLIB response")?
    };

    // Parse for return values
    let parsed = lyricsfile::parse_lyricsfile(&lyricsfile_content).map_err(|e| e.to_string())?;

    // Save to database (will update existing if lrclib_instance + lrclib_id match)
    let lyricsfile_id = app_handle
        .db(|db: &Connection| {
            db::upsert_lyricsfile_for_lrclib(
                &lrclib_instance,
                lrclib_id,
                &title,
                &album_name,
                &artist_name,
                duration,
                &lyricsfile_content,
                db,
            )
        })
        .map_err(|err| err.to_string())?;

    Ok(PrepareLyricsfileResult {
        lyricsfile_id,
        lyricsfile: lyricsfile_content,
        plain_lyrics: parsed.plain_lyrics.unwrap_or_default(),
        synced_lyrics: parsed.synced_lyrics.unwrap_or_default(),
        is_instrumental: parsed.is_instrumental,
        exists_in_db: true, // After refresh, it definitely exists
    })
}

#[tauri::command]
async fn save_lyrics(
    track_id: Option<i64>,
    lyricsfile_id: Option<i64>,
    lyricsfile: String,
    app_handle: AppHandle,
) -> Result<String, String> {
    let lyricsfile = lyricsfile.trim();

    // Parse the lyricsfile content to validate it
    let _parsed = lyricsfile::parse_lyricsfile(lyricsfile).map_err(|err| err.to_string())?;

    // If we have a track_id, this is a library track - update the lyricsfile record
    if let Some(id) = track_id {
        let track = app_handle
            .db(|db| db::get_track_by_id(id, db))
            .map_err(|err| err.to_string())?;

        // Update or create the lyricsfile record (presence fields are set automatically)
        app_handle
            .db(|db: &Connection| {
                db::upsert_lyricsfile_for_track(
                    track.id,
                    &track.title,
                    &track.album_name,
                    &track.artist_name,
                    track.duration,
                    lyricsfile,
                    db,
                )
            })
            .map_err(|err| err.to_string())?;

        app_handle.emit("reload-track-id", id).unwrap();
    } else if let Some(id) = lyricsfile_id {
        // Standalone lyricsfile update (LRCLIB flow)
        app_handle
            .db(|db: &Connection| db::update_lyricsfile_by_id(id, lyricsfile, db))
            .map_err(|err| err.to_string())?;
    } else {
        return Err("Either track_id or lyricsfile_id must be provided".to_string());
    }

    Ok("Lyrics saved successfully".to_owned())
}

/// Upload a lyricsfile to the LRCLIB community database.
///
/// DISABLED. Not registered in the invoke handler (see the comment there), kept
/// so the feature can be restored without rebuilding it from history. Adding
/// `#[tauri::command]` back and uncommenting the registration is enough.
///
/// The frontend no longer offers it: the editor's split save button became a
/// plain Save.
#[allow(dead_code)]
async fn publish_lyrics(
    title: String,
    album_name: String,
    artist_name: String,
    duration: f64,
    plain_lyrics: Option<String>,
    synced_lyrics: Option<String>,
    lyricsfile: Option<String>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let plain_lyrics = plain_lyrics.and_then(|lyrics| {
        let trimmed = lyrics.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_owned())
        }
    });
    let synced_lyrics = synced_lyrics.and_then(|lyrics| {
        let trimmed = lyrics.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_owned())
        }
    });
    let lyricsfile = lyricsfile.and_then(|content| {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(content)
        }
    });

    if plain_lyrics.is_none() && synced_lyrics.is_none() && lyricsfile.is_none() {
        return Err("No lyrics payload provided for publishing".to_owned());
    }

    let config = app_handle
        .db(|db: &Connection| db::get_config(db))
        .map_err(|err| err.to_string())?;

    let mut progress = PublishLyricsProgress {
        request_challenge: "Pending".to_owned(),
        solve_challenge: "Pending".to_owned(),
        publish_lyrics: "Pending".to_owned(),
    };
    progress.request_challenge = "In Progress".to_owned();
    app_handle
        .emit("publish-lyrics-progress", &progress)
        .unwrap();
    let challenge_response = lrclib::request_challenge::request(&config.lrclib_instance)
        .await
        .map_err(|err| err.to_string())?;
    progress.request_challenge = "Done".to_owned();
    progress.solve_challenge = "In Progress".to_owned();
    app_handle
        .emit("publish-lyrics-progress", &progress)
        .unwrap();
    let nonce = lrclib::challenge_solver::solve_challenge(
        &challenge_response.prefix,
        &challenge_response.target,
    );
    progress.solve_challenge = "Done".to_owned();
    progress.publish_lyrics = "In Progress".to_owned();
    app_handle
        .emit("publish-lyrics-progress", &progress)
        .unwrap();
    let publish_token = format!("{}:{}", challenge_response.prefix, nonce);
    lrclib::publish::request(
        &title,
        &album_name,
        &artist_name,
        duration,
        plain_lyrics.as_deref(),
        synced_lyrics.as_deref(),
        lyricsfile.as_deref(),
        &publish_token,
        &config.lrclib_instance,
    )
    .await
    .map_err(|err| err.to_string())?;
    progress.publish_lyrics = "Done".to_owned();
    app_handle
        .emit("publish-lyrics-progress", &progress)
        .unwrap();
    Ok(())
}

/// Directory where album covers extracted during a scan are cached.
///
/// Lives under the app data dir so it survives library resets: a rescan should
/// not have to re-read embedded art it has already extracted.
fn scanner_cover_dir(app_handle: &AppHandle) -> std::path::PathBuf {
    app_handle
        .path()
        .app_data_dir()
        .map(|dir| dir.join("covers"))
        .unwrap_or_else(|_| std::env::temp_dir().join("sing-along-covers"))
}

/// Resolve a library track id to something the player can open.
fn resolve_library_track(app_handle: &AppHandle, track_id: i64) -> Result<PlayableTrack, String> {
    app_handle
        .db(|db| db::get_track_by_id(track_id, db))
        .map(PlayableTrack::from)
        .map_err(|err| err.to_string())
}

/// Shared HTTP client for NetEase requests. A single client keeps connections
/// pooled across the download queue.
fn netease_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .expect("failed to build HTTP client")
}

/// Search and embed album cover art for a track.
///
/// Called by the frontend when `syncCover` is enabled on a download batch.
/// Runs after lyrics have been saved, so a cover failure does not prevent
/// lyrics from being written.
#[tauri::command]
async fn sync_cover(
    track_path: String,
    artist: String,
    album: String,
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    crate::cover::embed_cover(&track_path, &client, &artist, &album)
        .await
        .map_err(|e| e.to_string())
}

/// Select a track in the player without starting playback.
///
/// The frontend calls this on startup so the player bar shows either the track
/// that was playing last time or the first track of the library. Playback is
/// deliberately not started: launching an app should not make noise.
#[tauri::command]
async fn select_track(
    track_id: i64,
    queue: Option<Vec<i64>>,
    index: Option<usize>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let playable = resolve_library_track(&app_handle, track_id)?;

    let app_state: State<AppState> = app_handle.state();
    let mut player_guard = app_state.player.lock().map_err(|e| e.to_string())?;

    if let Some(ref mut player) = *player_guard {
        match (queue, index) {
            (Some(ids), Some(start)) if !ids.is_empty() => {
                player.playlist_mut().load(ids, start)
            }
            _ => player.playlist_mut().load_single(track_id),
        }
        player.select(playable).map_err(|err| err.to_string())?;
    }

    Ok(())
}

/// Start a track that came from the queue (end-of-track advance, next, previous,
/// or a shuffle/selection change).
///
/// Deliberately uses `player.play()`, which keeps the `AudioManager` alive, and
/// never `release_audio_device()`: reopening the output device between tracks
/// would add a gap of device setup latency on every transition.
fn start_queued_track(app_handle: &AppHandle, track_id: i64) -> Result<(), String> {
    let playable_track = resolve_library_track(app_handle, track_id)?;

    let app_state: State<AppState> = app_handle.state();
    let mut player_guard = app_state.player.lock().map_err(|e| e.to_string())?;

    if let Some(ref mut player) = *player_guard {
        return player.play(playable_track).map_err(|err| err.to_string());
    }

    Err("Player is not available".to_owned())
}

#[tauri::command]
async fn play_track(
    track_id: Option<i64>,
    file_path: Option<String>,
    title: Option<String>,
    album_name: Option<String>,
    artist_name: Option<String>,
    album_artist_name: Option<String>,
    duration: Option<f64>,
    queue: Option<Vec<i64>>,
    index: Option<usize>,
    app_state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let playable_track: PlayableTrack = if let Some(id) = track_id {
        resolve_library_track(&app_handle, id)?
    } else if let Some(path) = file_path {
        // File-based track - create from metadata
        let path_obj = std::path::Path::new(&path);
        let file_name = path_obj
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .to_string();

        // Extract metadata from file if not provided
        let (resolved_title, resolved_duration) = if title.is_none() || duration.is_none() {
            match scanner::metadata::TrackMetadata::from_path(path_obj) {
                Ok(metadata) => (
                    title.unwrap_or(metadata.title),
                    duration.unwrap_or(metadata.duration),
                ),
                Err(_) => (
                    title.unwrap_or_else(|| file_name.clone()),
                    duration.unwrap_or(0.0),
                ),
            }
        } else {
            (title.unwrap(), duration.unwrap())
        };

        PlayableTrack {
            id: None,
            file_path: path,
            file_name,
            title: resolved_title,
            album_name: album_name.unwrap_or_default(),
            artist_name: artist_name.unwrap_or_default(),
            album_artist_name,
            image_path: None,
            track_number: None,
            duration: resolved_duration,
            instrumental: false,
            lyricsfile: None,
            lyricsfile_id: None,
        }
    } else {
        return Err("Either track_id or file_path must be provided".to_string());
    };

    let mut player_guard = app_state.player.lock().unwrap();
    if let Some(ref mut player) = *player_guard {
        // Keep the queue in sync with what is actually starting. A file-picker
        // track (or a bare `play_track` with no queue) collapses to a one-track
        // queue so that transport controls behave predictably instead of
        // reporting a stale queue from a previous selection.
        match (queue, index) {
            (Some(ids), Some(start)) if !ids.is_empty() => player.playlist_mut().load(ids, start),
            _ => match playable_track.id {
                Some(id) => player.playlist_mut().load_single(id),
                None => player.playlist_mut().clear(),
            },
        }

        // The audio device is opened on first playback; surface that failure as
        // a notification too, since it is no longer reported at startup.
        if let Err(err) = player.play(playable_track) {
            drop(player_guard);
            let message = format!("Failed to start playback: {}", err);
            push_notification(&app_handle, message.clone(), NotifyType::Error);
            return Err(message);
        }
    }

    Ok(())
}

#[tauri::command]
async fn export_lyrics(
    track_id: i64,
    formats: Vec<ExportLyricsFormat>,
    lyricsfile: Option<String>,
    app_handle: AppHandle,
) -> Result<Vec<export::ExportResult>, String> {
    if formats.is_empty() {
        return Err("Select at least one export format".to_owned());
    }

    let track = app_handle
        .db(|db| db::get_track_by_id(track_id, db))
        .map_err(|err| err.to_string())?;

    let lyricsfile_content = lyricsfile
        .filter(|content| !content.trim().is_empty())
        .or_else(|| {
            track
                .lyricsfile
                .clone()
                .filter(|content| !content.trim().is_empty())
        })
        .ok_or_else(|| "No lyrics available for export".to_owned())?;

    let parsed =
        lyricsfile::parse_lyricsfile(&lyricsfile_content).map_err(|err| err.to_string())?;
    let export_formats = formats.into_iter().map(Into::into).collect::<Vec<_>>();

    Ok(export::export_track_with_embed_gate(&track, &parsed, &export_formats, || {
        embedding_allowed(&app_handle)
    }))
}

/// Detail for a single format export result
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportFormatDetail {
    pub format: String,
    pub status: export::ExportStatus,
}

/// Result summary for track export (used by mass export)
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TrackExportSummary {
    pub success: bool,
    pub exported: i32,
    pub skipped: i32,
    pub errors: i32,
    pub message: String,
    pub details: Vec<ExportFormatDetail>,
}

#[tauri::command]
async fn export_track_lyrics(
    track_id: i64,
    formats: Vec<ExportLyricsFormat>,
    app_handle: AppHandle,
) -> Result<TrackExportSummary, String> {
    if formats.is_empty() {
        return Ok(TrackExportSummary {
            success: true,
            exported: 0,
            skipped: 0,
            errors: 0,
            message: "No formats selected".to_owned(),
            details: vec![],
        });
    }

    let track = app_handle
        .db(|db| db::get_track_by_id(track_id, db))
        .map_err(|err| err.to_string())?;

    // Get lyrics from lyricsfile column
    let lyricsfile_content = track
        .lyricsfile
        .clone()
        .filter(|content| !content.trim().is_empty());

    if lyricsfile_content.is_none() {
        return Ok(TrackExportSummary {
            success: true,
            exported: 0,
            skipped: 1,
            errors: 0,
            message: "No lyrics available for this track".to_owned(),
            details: vec![],
        });
    }

    let parsed = lyricsfile::parse_lyricsfile(&lyricsfile_content.unwrap())
        .map_err(|err| err.to_string())?;
    let export_formats = formats.into_iter().map(Into::into).collect::<Vec<_>>();

    let results = export::export_track_with_embed_gate(&track, &parsed, &export_formats, || {
        embedding_allowed(&app_handle)
    });

    // Count results based on status
    let exported = results
        .iter()
        .filter(|r| matches!(r.status, export::ExportStatus::Success))
        .count() as i32;
    let skipped = results
        .iter()
        .filter(|r| matches!(r.status, export::ExportStatus::Skipped(_)))
        .count() as i32;
    let errors = results
        .iter()
        .filter(|r| matches!(r.status, export::ExportStatus::Error(_)))
        .count() as i32;

    // Build detailed results with status info
    let details: Vec<ExportFormatDetail> = results
        .iter()
        .map(|r| ExportFormatDetail {
            format: format!("{:?}", r.format).to_lowercase(),
            status: r.status.clone(),
        })
        .collect();

    // Build message
    let message = if errors > 0 {
        let error_details: Vec<String> = results
            .iter()
            .filter(|r| matches!(r.status, export::ExportStatus::Error(_)))
            .map(|r| {
                let msg = match &r.status {
                    export::ExportStatus::Error(msg) => msg.clone(),
                    _ => String::new(),
                };
                format!("{:?}: {}", r.format, msg)
            })
            .collect();
        format!(
            "Exported {}, skipped {}, {} error(s) - {}",
            exported,
            skipped,
            errors,
            error_details.join("; ")
        )
    } else if exported > 0 {
        if skipped > 0 {
            format!("Exported {}, skipped {}", exported, skipped)
        } else {
            format!("Exported {} format(s)", exported)
        }
    } else if skipped > 0 {
        format!("Skipped {} format(s)", skipped)
    } else {
        "No formats exported".to_owned()
    };

    Ok(TrackExportSummary {
        success: errors == 0,
        exported,
        skipped,
        errors,
        message,
        details,
    })
}

#[tauri::command]
async fn get_track_ids_with_lyrics(app_state: State<'_, AppState>) -> Result<Vec<i64>, String> {
    let conn_guard = app_state.db.lock().unwrap();
    let conn = conn_guard.as_ref().unwrap();
    let track_ids = db::get_track_ids_with_lyrics(conn).map_err(|err| err.to_string())?;

    Ok(track_ids)
}

#[tauri::command]
fn pause_track(app_state: tauri::State<AppState>) -> Result<(), String> {
    let mut player_guard = app_state.player.lock().map_err(|e| e.to_string())?;

    if let Some(ref mut player) = *player_guard {
        player.pause();
    }

    Ok(())
}

#[tauri::command]
fn resume_track(app_state: tauri::State<AppState>) -> Result<(), String> {
    let mut player_guard = app_state.player.lock().map_err(|e| e.to_string())?;

    if let Some(ref mut player) = *player_guard {
        player.resume();
    }

    Ok(())
}

#[tauri::command]
fn seek_track(position: f64, app_state: tauri::State<AppState>) -> Result<(), String> {
    let mut player_guard = app_state.player.lock().map_err(|e| e.to_string())?;

    if let Some(ref mut player) = *player_guard {
        player.seek(position);
    }

    Ok(())
}

#[tauri::command]
fn stop_track(app_state: tauri::State<AppState>) -> Result<(), String> {
    let mut player_guard = app_state.player.lock().map_err(|e| e.to_string())?;

    if let Some(ref mut player) = *player_guard {
        // An explicit stop also hands the audio device back, so a stopped player
        // stops consuming CPU on an idle output stream. The queue survives, so
        // pressing play again resumes the same position in it.
        player.release_audio_device();
    }

    Ok(())
}

/// Restart the current track without changing the queue.
///
/// The transport button and the space bar use this when playback is stopped.
/// Replaying through `play_track` would send no queue and collapse it to a
/// single track, so the playlist would be lost by pressing play.
#[tauri::command]
async fn replay_current(app_handle: AppHandle) -> Result<(), String> {
    // Read the id from the queue, not from `player.track`: an explicit stop
    // clears the loaded track while deliberately keeping the queue, so the
    // track field is gone exactly when replay matters.
    let track_id = {
        let app_state: State<AppState> = app_handle.state();
        let player_guard = app_state.player.lock().map_err(|e| e.to_string())?;

        player_guard
            .as_ref()
            .and_then(|player| player.playlist().current_id())
    };

    match track_id {
        Some(track_id) => start_queued_track(&app_handle, track_id),
        None => Ok(()),
    }
}

/// Skip to the next queue entry.
///
/// Unlike the end-of-track path, an explicit skip always advances: repeat-one
/// should not swallow a deliberate "next" press.
#[tauri::command]
async fn play_next(app_handle: AppHandle) -> Result<(), String> {
    let next_id = {
        let app_state: State<AppState> = app_handle.state();
        let mut player_guard = app_state.player.lock().map_err(|e| e.to_string())?;

        match *player_guard {
            Some(ref mut player) => player.playlist_mut().advance(),
            None => None,
        }
    };

    match next_id {
        Some(track_id) => start_queued_track(&app_handle, track_id),
        None => Ok(()),
    }
}

#[tauri::command]
async fn play_previous(app_handle: AppHandle) -> Result<(), String> {
    let previous_id = {
        let app_state: State<AppState> = app_handle.state();
        let mut player_guard = app_state.player.lock().map_err(|e| e.to_string())?;

        match *player_guard {
            Some(ref mut player) => player.playlist_mut().rewind(),
            None => None,
        }
    };

    match previous_id {
        Some(track_id) => start_queued_track(&app_handle, track_id),
        None => Ok(()),
    }
}

#[tauri::command]
fn set_shuffle(shuffle: bool, app_state: tauri::State<AppState>) -> Result<(), String> {
    let mut player_guard = app_state.player.lock().map_err(|e| e.to_string())?;

    if let Some(ref mut player) = *player_guard {
        player.playlist_mut().set_shuffle(shuffle);
    }

    Ok(())
}

#[tauri::command]
fn set_repeat_mode(repeat: RepeatMode, app_state: tauri::State<AppState>) -> Result<(), String> {
    let mut player_guard = app_state.player.lock().map_err(|e| e.to_string())?;

    if let Some(ref mut player) = *player_guard {
        player.playlist_mut().set_repeat(repeat);
    }

    Ok(())
}

#[tauri::command]
fn set_volume(
    volume: f64,
    app_state: tauri::State<AppState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let mut player_guard = app_state.player.lock().map_err(|e| e.to_string())?;

    if let Some(ref mut player) = *player_guard {
        player.set_volume(volume);
    }
    drop(player_guard);

    // Persist volume to config
    app_handle
        .db(|db| db::set_volume_config(volume, db))
        .map_err(|err| err.to_string())?;

    Ok(())
}

#[tauri::command]
fn set_playback_speed(playback_speed: f64, app_state: tauri::State<AppState>) -> Result<(), String> {
    let mut player_guard = app_state.player.lock().map_err(|e| e.to_string())?;

    if let Some(ref mut player) = *player_guard {
        player.set_playback_speed(playback_speed);
    }

    Ok(())
}

#[tauri::command]
fn open_devtools(app_handle: AppHandle) {
    // `open_devtools` only exists when the `devtools` feature is on, so a release
    // build exposes the command but does nothing instead of failing to compile.
    #[cfg(feature = "devtools")]
    {
        if let Some(window) = app_handle.get_webview_window("main") {
            window.open_devtools();
        }
    }

    #[cfg(not(feature = "devtools"))]
    let _ = app_handle;
}

#[tauri::command]
fn drain_notifications(app_state: tauri::State<AppState>) -> Vec<Notify> {
    let mut queued_notifications = app_state.queued_notifications.lock().unwrap();
    let notifications = queued_notifications.drain(..).collect();
    notifications
}

/// Deliver one notification to the frontend.
///
/// Notifications are pushed as an event; the queue is only a backstop for
/// messages produced before the webview has a listener (startup) or for the rare
/// case where emitting fails. The frontend drains the queue once at boot.
fn push_notification(app_handle: &AppHandle, message: String, notify_type: NotifyType) {
    let notify = Notify {
        message,
        notify_type,
    };

    if let Err(e) = app_handle.emit("notification", &notify) {
        eprintln!("Failed to emit notification: {}", e);
        let app_state: State<AppState> = app_handle.state();
        app_state
            .queued_notifications
            .lock()
            .unwrap()
            .push(notify);
    }
}

#[tauri::command]
async fn read_text_file(file_path: String) -> Result<String, String> {
    std::fs::read_to_string(&file_path)
        .map_err(|err| format!("Failed to read file: {}", err))
}

#[tauri::command]
async fn segment_words(text: String) -> Result<Vec<String>, String> {
    Ok(word_segmentation::segment_words_for_timing(text.as_str()))
}

// The library scans use `block_in_place`; this app is I/O bound with only a
// handful of concurrent commands, so a large default worker pool (one thread per
// core plus extras) just burns stack and memory. A small pool is enough and
// keeps idle thread count down.
#[tokio::main(worker_threads = 4)]
async fn main() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(AppState {
            db: Default::default(),
            player: Default::default(),
            queued_notifications: std::sync::Mutex::new(Vec::new()),
        });

    // W3C WebDriver server (`tauri-plugin-webdriver`) — opt-in via the `webdriver`
    // feature, which `tauri dev` enables through the `dev` feature.
    // Listens on http://127.0.0.1:4445 (override with TAURI_WEBDRIVER_PORT);
    // see AGENTS.md for the agent inspection runbook. Release builds have no
    // server at all.
    #[cfg(feature = "webdriver")]
    let builder = builder.plugin(tauri_plugin_webdriver::init());

    builder
        .invoke_handler(tauri::generate_handler![
            get_directories,
            set_directories,
            get_init,
            get_config,
            set_config,
            set_export_preferences,
            set_lyric_script,
            uninitialize_library,
            full_scan_library,
            scan_library,
            get_tracks,
            get_track_ids,
            get_track,
            get_albums,
            get_album_ids,
            get_album,
            get_album_cover,
            backfill_album_covers,
            get_artists,
            get_artist_ids,
            get_artist,
            get_album_tracks,
            get_artist_tracks,
            get_album_track_ids,
            get_artist_track_ids,
            download_lyrics,
            apply_lyrics,
            retrieve_lyrics,
            retrieve_lyrics_by_id,
            search_lyrics,
            search_lyrics_multi,
            save_lyrics,
            // Publishing to the community database is disabled for now. The
            // command below is kept so re-enabling it is a one-line change;
            // unregistering it means a stale frontend build cannot reach the
            // network path even by accident.
            // publish_lyrics,
            export_lyrics,
            export_track_lyrics,
            get_track_ids_with_lyrics,
            play_track,
            select_track,
            sync_cover,
            replay_current,
            play_next,
            play_previous,
            set_shuffle,
            set_repeat_mode,
            pause_track,
            resume_track,
            seek_track,
            stop_track,
            set_volume,
            set_playback_speed,
            open_devtools,
            drain_notifications,
            find_matching_tracks,
            get_audio_metadata,
            prepare_search_query,
            prepare_lrclib_lyricsfile,
            prepare_search_hit_lyricsfile,
            refresh_lrclib_lyricsfile,
            read_text_file,
            segment_words,
        ])
        .setup(|app| {
            let handle = app.handle();

            let app_state: State<AppState> = handle.state();
            let db = db::initialize_database(&handle).expect("Database initialize should succeed");
            *app_state.db.lock().unwrap() = Some(db);

            // Load config to get initial volume
            let initial_volume = handle
                .db(|db| db::get_config(db))
                .map(|config| config.volume)
                .unwrap_or(1.0);

            // The audio device is opened lazily on first playback, so a session
            // that never plays anything keeps no ALSA/CPAL threads alive.
            *app_state.player.lock().unwrap() = Some(Player::new(initial_volume));

            let handle_clone = handle.clone();

            // State emitter.
            //
            // This used to push the whole serialized player 25 times a second,
            // regardless of whether anything had changed. Now it only emits
            // when the observable state actually moves, and while nothing is
            // playing it idles at a slow tick.
            //
            // The frontend needs at most ~100ms of seek-bar granularity (the
            // karaoke sweep interpolates between updates), and it only receives
            // a small snapshot rather than the serialized player (which used to
            // carry the full track and its lyricsfile YAML).
            tokio::spawn(async move {
                const PLAYING_TICK: std::time::Duration = std::time::Duration::from_millis(100);
                const IDLE_TICK: std::time::Duration = std::time::Duration::from_millis(500);

                // Compare against what was last sent, not against the state at the
                // start of this tick: commands (set_shuffle, set_repeat_mode,
                // play_track) mutate the player directly, so a
                // before/after comparison inside renew_state cannot see them.
                let mut last_emitted: Option<player::PlayerSnapshot> = None;

                loop {
                    // Advance the queue for tracks that ended on their own.
                    //
                    // `renew_state` only detects the end; resolving the next id
                    // to a file path needs the DB, so it happens out here, with
                    // the player lock released. This keeps the player lock from
                    // being held across file I/O and avoids a lock-order hazard
                    // between `player` and `db`.
                    let finished = {
                        let app_state: State<AppState> = handle_clone.state();
                        let mut finished = Vec::new();

                        if let Ok(mut player_guard) = app_state.player.lock() {
                            if let Some(ref mut player) = *player_guard {
                                while let Some(track_id) = player.take_finished() {
                                    if let Some(decision) = player.decide_after_end(track_id) {
                                        finished.push(decision);
                                    }
                                }
                            }
                        }

                        finished
                    };

                    for decision in finished {
                        match decision {
                            PlaybackDecision::Play(track_id) | PlaybackDecision::Replay(track_id) => {
                                if let Err(err) = start_queued_track(&handle_clone, track_id) {
                                    eprintln!("Failed to advance the playlist: {}", err);
                                }
                            }
                            PlaybackDecision::Stop => {}
                        }
                    }

                    let tick = {
                        let app_state: State<AppState> = handle_clone.state();

                        let tick = match app_state.player.lock() {
                            Ok(mut player_guard) => match *player_guard {
                                Some(ref mut player) => {
                                    // Refreshes progress/status from the audio
                                    // backend; the queue may also have moved.
                                    player.renew_state();
                                    let playing = player.is_playing();
                                    let snapshot = player.snapshot();

                                    if last_emitted.as_ref() != Some(&snapshot) {
                                        if let Err(e) = handle_clone.emit("player-state", &snapshot)
                                        {
                                            eprintln!("Failed to emit player state: {}", e);
                                        } else {
                                            last_emitted = Some(snapshot);
                                        }
                                    }

                                    if playing {
                                        PLAYING_TICK
                                    } else {
                                        IDLE_TICK
                                    }
                                }
                                None => IDLE_TICK,
                            },
                            Err(e) => {
                                eprintln!("Failed to lock player: {}", e);
                                IDLE_TICK
                            }
                        };

                        tick
                    };

                    tokio::time::sleep(tick).await;
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod auto_export_tests {
    use super::*;

    #[test]
    fn embedded_exports_write_supported_fixtures_and_skip_unsupported_or_disabled() {
        use export::{ExportFormat::Embedded, ExportStatus};
        let directory = std::env::temp_dir().join(format!("sing-along-embed-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let content = lyricsfile::build_lyricsfile(
            &lyricsfile::LyricsfileTrackMetadata::new("Song", "Album", "Artist", 1.0),
            Some("embed-test-line"),
            None,
        )
        .unwrap();
        let parsed = lyricsfile::parse_lyricsfile(&content).unwrap();
        for (name, bytes) in [
            (
                "song.MP3",
                include_bytes!("../tests/fixtures/embedding.mp3").as_slice(),
            ),
            (
                "song.FLAC",
                include_bytes!("../tests/fixtures/embedding.flac").as_slice(),
            ),
        ] {
            let path = directory.join(name);
            std::fs::write(&path, bytes).unwrap();
            let track = test_track(&path);
            // A disabled gate never reaches the writer, even for a valid audio file.
            let result = export::export_track_with_embed_gate(&track, &parsed, &[Embedded], || false);
            assert!(
                matches!(&result[0].status, ExportStatus::Skipped(reason) if reason.contains("disabled"))
            );
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
            let result = export::export_track(&track, &parsed, &[Embedded]);
            assert!(
                matches!(result[0].status, ExportStatus::Success),
                "{result:?}"
            );
            let written = std::fs::read(&path).unwrap();
            assert!(written
                .windows(b"embed-test-line".len())
                .any(|window| window == b"embed-test-line"));
        }

        let path = directory.join("song.ogg");
        let bytes = b"unsupported audio must not be touched";
        std::fs::write(&path, bytes).unwrap();
        let track = test_track(&path);
        let result = export::export_track(&track, &parsed, &[Embedded]);
        assert!(
            matches!(&result[0].status, ExportStatus::Skipped(reason) if reason.contains("unsupported"))
        );
        assert!(result[0].path.is_none());
        let result = download_export_message(
            "Plain lyrics downloaded",
            &track,
            &content,
            &[Embedded],
            || true,
        );
        assert!(result.starts_with("Plain lyrics downloaded; Embedded export skipped: unsupported"));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);

        // Check the current gate for each operation, even within one batch snapshot.
        let track = test_track(&directory.join("song.MP3"));
        let mut enabled = true;
        let result =
            export::export_track_with_embed_gate(&track, &parsed, &[Embedded, Embedded], || {
                let current = enabled;
                enabled = false;
                current
            });
        assert!(matches!(result[0].status, ExportStatus::Success));
        assert!(matches!(result[1].status, ExportStatus::Skipped(_)));
        let before = std::fs::read(&track.file_path).unwrap();
        let result = download_export_message(
            "Plain lyrics downloaded",
            &track,
            &content,
            &[Embedded],
            || false,
        );
        assert!(result.starts_with("Plain lyrics downloaded; Embedded export skipped: experimental"));
        assert_eq!(std::fs::read(&track.file_path).unwrap(), before);
        std::fs::remove_dir_all(directory).unwrap();
    }

    fn test_track(path: &std::path::Path) -> PersistentTrack {
        PersistentTrack {
            id: 1,
            file_path: path.to_string_lossy().into_owned(),
            file_name: "song.flac".into(),
            title: "Song".into(),
            album_name: "Album".into(),
            album_artist_name: None,
            album_id: 1,
            artist_name: "Artist".into(),
            artist_id: 1,
            image_path: None,
            track_number: None,
            txt_lyrics: None,
            lrc_lyrics: None,
            lyricsfile: None,
            lyricsfile_id: None,
            duration: 100.0,
            instrumental: false,
        }
    }

    #[test]
    fn selection_allows_embed_only_and_preserves_order() {
        assert_eq!(
            AutoExportOptions {
                plain_text: false,
                synced_lrc: false,
                embed_into_track: true,
            }
            .formats()
            .unwrap(),
            vec![export::ExportFormat::Embedded]
        );
        assert!(AutoExportOptions {
            plain_text: false,
            synced_lrc: false,
            embed_into_track: false,
        }
        .formats()
        .is_err());
        assert_eq!(
            AutoExportOptions {
                plain_text: true,
                synced_lrc: true,
                embed_into_track: true,
            }
            .formats()
            .unwrap(),
            vec![
                export::ExportFormat::Txt,
                export::ExportFormat::Lrc,
                export::ExportFormat::Embedded
            ],
        );
    }

    #[test]
    fn download_export_messages_keep_success_skips_and_errors_in_writer_order() {
        let directory =
            std::env::temp_dir().join(format!("sing-along-auto-export-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let track = test_track(&directory.join("song.flac"));
        let content = lyricsfile::build_lyricsfile(
            &lyricsfile::LyricsfileTrackMetadata::new("Song", "Album", "Artist", 100.0),
            Some("hello"),
            Some("[00:01.00] hello"),
        )
        .unwrap();
        let formats = [export::ExportFormat::Txt, export::ExportFormat::Lrc];
        std::fs::write(directory.join("song.lrc"), "old lyrics").unwrap();
        let result =
            download_export_message("Synced lyrics downloaded", &track, &content, &formats, || true);
        assert_eq!(
            result,
            "Synced lyrics downloaded; TXT export completed; LRC export completed"
        );
        assert!(!directory.join("song.txt").exists());
        assert!(std::fs::read_to_string(directory.join("song.lrc"))
            .unwrap()
            .contains("hello"));

        let plain = lyricsfile::build_lyricsfile(
            &lyricsfile::LyricsfileTrackMetadata::new("Song", "Album", "Artist", 100.0),
            Some("plain only"),
            None,
        )
        .unwrap();
        let result = download_export_message("Plain lyrics downloaded", &track, &plain, &formats, || true);
        assert_eq!(result, "Plain lyrics downloaded; TXT export completed; LRC export skipped: no synced lyrics available");
        assert!(directory.join("song.txt").exists());
        // A directory at the LRC target deterministically fails writing, even when tests run as root.
        std::fs::create_dir(directory.join("song.lrc")).unwrap();
        let result =
            download_export_message("Synced lyrics downloaded", &track, &content, &formats, || true);
        assert!(result
            .starts_with("Synced lyrics downloaded; TXT export completed; LRC export failed:"));
        assert!(!directory.join("song.txt").exists()); // Existing destructive semantics are retained.
        let result = download_export_message("Lyrics downloaded", &track, "[invalid", &formats, || true);
        assert!(result.starts_with("Lyrics downloaded; export failed:"));
        std::fs::remove_dir_all(directory).unwrap();
    }
}
