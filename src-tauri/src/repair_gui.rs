//! The commands that repair the metadata of the library.
//!
//! The repair is a lookup rather than a guess: the track's title and artist
//! are looked up in the release database, which knows the real release, and
//! the album name and the artwork come from there. A track that is not in
//! the database keeps what it has, and the report says so.
//!
//! Nothing is removed and nothing is guessed. The album names that came
//! from a folder are reported first, so the user can see what would be
//! changed before anything is written.

use crate::repair::{self, AlbumFacts, RepairOptions};
use serde::Serialize;
use std::path::PathBuf;

/// An album whose name looks like it came from a folder.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuspiciousAlbum {
    pub id: i64,
    pub name: String,
    /// How many artists have tracks under it.
    ///
    /// A real album belongs to one artist, so several is the sign.
    pub artists: usize,
    /// How many tracks it holds.
    pub tracks: usize,
}

/// A track of an album, with what the release database says about it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumTrack {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub file_path: String,
    /// The album the release database has, when it has one.
    pub real_album: Option<String>,
    /// The year the release database has.
    pub year: Option<String>,
    /// Whether the release database has artwork for it.
    pub has_cover: bool,
}

/// The albums whose name looks like it came from a folder.
#[tauri::command]
pub async fn find_suspicious_albums(
    app_state: tauri::State<'_, crate::state::AppState>,
) -> Result<Vec<SuspiciousAlbum>, String> {
    let conn_guard = app_state.db.lock().map_err(|err| err.to_string())?;
    let conn = conn_guard.as_ref().unwrap();

    let albums = repair::albums_worth_repairing(conn).map_err(|err| err.to_string())?;

    let mut result = Vec::new();
    for (id, name, artists, tracks) in albums {
        result.push(SuspiciousAlbum {
            id,
            name,
            artists,
            tracks,
        });
    }

    Ok(result)
}

/// The tracks of an album, with what the release database says about each.
///
/// The lookup is done here rather than in the repair, so the user can see
/// what would be changed before it is.
#[tauri::command]
pub async fn look_up_album_tracks(
    album_id: i64,
    app_state: tauri::State<'_, crate::state::AppState>,
) -> Result<Vec<AlbumTrack>, String> {
    // The name, not the row: the name that came from a folder is several
    // rows, and the tracks of all of them are the ones being looked at.
    let (album_name, tracks) = {
        let conn_guard = app_state.db.lock().map_err(|err| err.to_string())?;
        let conn = conn_guard.as_ref().unwrap();
        let name: String = conn
            .query_row("SELECT name FROM albums WHERE id = ?1", [album_id], |row| {
                row.get(0)
            })
            .map_err(|err| err.to_string())?;
        let tracks = repair::tracks_of_album_name(&name, conn)
            .map_err(|err| err.to_string())?;
        (name, tracks)
    };

    let config = {
        let conn_guard = app_state.db.lock().map_err(|err| err.to_string())?;
        let conn = conn_guard.as_ref().unwrap();
        crate::db::get_config(conn).map_err(|err| err.to_string())?
    };

    let client = crate::http::client(&config.proxy_url, 20).map_err(|err| err.to_string())?;

    let mut result = Vec::new();
    for track in tracks {
        let facts = crate::youtube::lookup(&client, &track.title, &track.artist_name)
            .await
            .ok()
            .flatten();

        result.push(AlbumTrack {
            id: track.id,
            title: track.title,
            artist: track.artist_name,
            file_path: track.file_path,
            real_album: facts.as_ref().and_then(|facts| facts.album.clone()),
            year: facts.as_ref().and_then(|facts| facts.year.clone()),
            has_cover: facts
                .as_ref()
                .and_then(|facts| facts.cover_url.as_ref())
                .is_some(),
        });
    }

    Ok(result)
}

/// Repair the metadata of a set of tracks.
///
/// The album and the artwork are written into the file, the text is
/// converted to the requested script, and the file is written in UTF-8. The
/// library is told about the new album afterwards, so that it agrees with
/// the file.
#[tauri::command]
pub async fn repair_tracks(
    track_ids: Vec<i64>,
    // The script the text should end up in: "off", "simplified" or
    // "traditional".
    lyric_script: Option<String>,
    fix_cover: Option<bool>,
    app_state: tauri::State<'_, crate::state::AppState>,
) -> Result<Vec<repair::RepairOutcome>, String> {
    let script = crate::script_convert::ScriptTarget::parse(
        lyric_script.as_deref().unwrap_or("off"),
    )
    .unwrap_or(crate::script_convert::ScriptTarget::Off);

    let options = RepairOptions {
        script,
        fix_cover: fix_cover.unwrap_or(true),
        fix_encoding: true,
    };

    let config = {
        let conn_guard = app_state.db.lock().map_err(|err| err.to_string())?;
        let conn = conn_guard.as_ref().unwrap();
        crate::db::get_config(conn).map_err(|err| err.to_string())?
    };

    let client = crate::http::client(&config.proxy_url, 20).map_err(|err| err.to_string())?;

    let mut outcomes = Vec::new();

    for track_id in track_ids {
        let track = {
            let conn_guard = app_state.db.lock().map_err(|err| err.to_string())?;
            let conn = conn_guard.as_ref().unwrap();
            crate::db::get_track_by_id(track_id, conn).ok()
        };

        let Some(track) = track else {
            continue;
        };

        let path = PathBuf::from(&track.file_path);
        if !path.exists() {
            outcomes.push(repair::RepairOutcome {
                path: track.file_path.clone(),
                title: track.title.clone(),
                artist: track.artist_name.clone(),
                old_album: track.album_name.clone(),
                new_album: track.album_name.clone(),
                album_changed: false,
                cover_written: false,
                encoding_changed: false,
                script_changed: false,
                note: Some("the file is not on the disk".to_owned()),
            });
            continue;
        }

        // The release database is asked what the track really is.
        let facts = crate::youtube::lookup(&client, &track.title, &track.artist_name)
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| AlbumFacts {
                album: None,
                year: None,
                cover_url: None,
                album_source: crate::youtube::AlbumSource::YouTube,
            });

        let cover_bytes = match (&facts.cover_url, options.fix_cover) {
            (Some(url), true) => crate::youtube::download_cover(&client, url).await.ok(),
            _ => None,
        };

        let outcome = match repair::repair_file(
            &path,
            &facts,
            cover_bytes.as_deref(),
            options,
        ) {
            Ok(outcome) => outcome,
            Err(error) => {
                let mut outcome = repair::RepairOutcome {
                    path: track.file_path.clone(),
                    title: track.title.clone(),
                    artist: track.artist_name.clone(),
                    old_album: track.album_name.clone(),
                    new_album: track.album_name.clone(),
                    album_changed: false,
                    cover_written: false,
                    encoding_changed: false,
                    script_changed: false,
                    note: None,
                };
                outcome.note = Some(format!("{error:#}"));
                outcome
            }
        };

        // The library is told about the new album, so that it agrees with the
        // file. A track whose file is on a different album is a library that
        // changes on the next scan.
        if outcome.album_changed {
            let mut conn_guard = app_state.db.lock().map_err(|err| err.to_string())?;
            let conn = conn_guard.as_mut().unwrap();
            let _ = repair::move_track_to_album(track_id, &outcome.new_album, conn);
        }

        outcomes.push(outcome);
    }

    Ok(outcomes)
}

/// The tracks whose text is not in UTF-8.
///
/// The encoding is worth reporting on its own: a library collected over
/// years holds files whose text frames say they are in UTF-16, and the same
/// title is spelled differently depending on the file it is in.
#[tauri::command]
pub async fn find_non_utf8_tracks(
    app_state: tauri::State<'_, crate::state::AppState>,
) -> Result<Vec<NonUtf8Track>, String> {
    let tracks = {
        let conn_guard = app_state.db.lock().map_err(|err| err.to_string())?;
        let conn = conn_guard.as_ref().unwrap();
        crate::db::get_tracks(conn).map_err(|err| err.to_string())?
    };

    let mut result = Vec::new();
    for track in tracks {
        let path = PathBuf::from(&track.file_path);
        if !path.exists() {
            continue;
        }
        if !repair::is_utf8_public(&path) {
            result.push(NonUtf8Track {
                id: track.id,
                title: track.title,
                artist: track.artist_name,
                file_path: track.file_path,
            });
        }
    }

    Ok(result)
}

/// A track whose text is not in UTF-8.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NonUtf8Track {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub file_path: String,
}
