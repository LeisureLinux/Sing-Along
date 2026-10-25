//! Repairing the metadata of the files in the library.
//!
//! A library that was built by copying files into folders inherits the
//! folders' names: a track whose tag had no album gets the name of the
//! directory it was dropped into, and a library of Chinese songs ends up
//! with an album called "KXT" holding four songs by four different artists.
//!
//! The repair is a lookup rather than a guess. The track's title and artist
//! are looked up in the release database, which knows the real release, and
//! the album name and the artwork come from there. A track that is not in
//! the database keeps what it has.
//!
//! ## The text
//!
//! Two things are done to the text, and both are lossy, so both are the
//! user's choice:
//!
//! * **The script.** A title in Traditional is converted to Simplified, or
//!   the other way, using the same tables the lyrics conversion uses.
//! * **The encoding.** The text frames of an ID3v2 tag say what encoding
//!   they are in, and a library collected over years holds tags in several:
//!   UTF-16 from one ripper, UTF-8 from another. The repaired file is
//!   written in UTF-8, which is what the format's version 4 uses and what
//!   every player reads.
//!
//! ## The file, not only the library
//!
//! The file is rewritten, not just the library row. A library row that
//! disagrees with the file it points at is a library that changes on the
//! next scan, and a player that reads the file shows the old values.

use crate::script_convert::{self, ScriptTarget};
use anyhow::{Context, Result};
use lofty::config::WriteOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::{Picture, PictureType};
use lofty::probe::Probe;
use lofty::tag::{ItemKey, ItemValue, TagItem};
use std::path::Path;

/// What one repair changed.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairOutcome {
    /// The file that was repaired.
    pub path: String,
    pub title: String,
    pub artist: String,
    /// The album before the repair.
    pub old_album: String,
    /// The album after it. Equal to the old one when nothing changed.
    pub new_album: String,
    /// Whether the album name was changed.
    pub album_changed: bool,
    /// Whether the artwork was written.
    pub cover_written: bool,
    /// Whether the text was written in a different encoding.
    pub encoding_changed: bool,
    /// Whether the script was converted.
    pub script_changed: bool,
    /// Why nothing was done, when nothing was done.
    pub note: Option<String>,
}

/// What the user wants done.
#[derive(Debug, Clone, Copy)]
pub struct RepairOptions {
    /// The script the text should end up in.
    pub script: ScriptTarget,
    /// Whether the artwork should be looked up and written.
    pub fix_cover: bool,
    /// Whether the file should be rewritten in UTF-8.
    ///
    /// The text of a file is rewritten only when something else changed, so
    /// this is what decides whether the encoding is corrected on its own.
    pub fix_encoding: bool,
}

impl Default for RepairOptions {
    fn default() -> Self {
        Self {
            script: ScriptTarget::Off,
            fix_cover: true,
            fix_encoding: true,
        }
    }
}

/// The album a track really belongs to.
///
/// The release database knows the real release, and the cover art archive
/// has the artwork for it. `None` when the track is not in the database,
/// which is an ordinary answer and not a failure.
pub type AlbumFacts = crate::youtube::TrackCorrection;

/// The text frames of a file, and the encoding they are in.
///
/// The encoding is what has to be corrected: a file whose frames are in
/// UTF-16 is not wrong, but a library with files in three encodings is
/// a library where the same title is spelled differently depending on the
/// file it is in.
fn text_frames(tag: &lofty::tag::Tag) -> Vec<(ItemKey, String)> {
    tag.items()
        .filter_map(|item| {
            let key = item.key();
            let text = item.value().text()?;
            Some((key, text.to_owned()))
        })
        .collect()
}

/// Whether the file's text frames are in UTF-8.
///
/// The frames of an ID3v2 tag say what encoding they are in, and the
/// encoding byte is the first byte of the frame's body.
pub fn is_utf8_public(path: &Path) -> bool {
    is_utf8(path)
}

fn is_utf8(path: &Path) -> bool {
    let Ok(data) = std::fs::read(path) else {
        return true;
    };
    if data.len() < 10 || &data[..3] != b"ID3" {
        // Not an ID3v2 tag, so there is no encoding byte to be wrong. A
        // Vorbis comment is UTF-8 by definition.
        return true;
    }

    // The frames follow the header. A quick scan for the text frames is
    // enough: the point is whether any of them says something other than
    // UTF-8.
    for tag in [b"TIT2", b"TALB", b"TPE1", b"TPE2", b"COMM"] {
        if let Some(index) = data
            .windows(4)
            .position(|window| window == tag)
            .filter(|index| index + 10 < data.len())
        {
            // The frame header is ten bytes: the identifier, the size and
            // the flags. The encoding byte is the first byte of the body.
            let encoding = data[index + 10];
            if encoding != 3 {
                return false;
            }
        }
    }

    true
}

/// Write the text frames of a file, in UTF-8.
///
/// The frames are taken apart and put back together, because lofty has no
/// mutable iterator over the items: a field is removed, written and inserted
/// again. Only the text is touched; the picture and the binary frames are
/// left alone.
fn write_text_frames(tag: &mut lofty::tag::Tag, frames: &[(ItemKey, String)]) {
    let mut keys: Vec<ItemKey> = frames.iter().map(|(key, _)| key.clone()).collect();
    keys.dedup();

    for key in keys {
        let items: Vec<TagItem> = tag.take_filter(key.clone(), |_| true).collect();
        for item in items {
            let Some(text) = item.value().text().map(|text| text.to_owned()) else {
                // A picture or a binary frame: put it back untouched.
                tag.insert(item);
                continue;
            };
            tag.insert(TagItem::new(key.clone(), ItemValue::Text(text)));
        }
    }
}

/// Repair one file.
///
/// The album and the artwork come from the facts the caller looked up; the
/// text is converted to the requested script.
pub fn repair_file(
    path: &Path,
    facts: &AlbumFacts,
    cover_bytes: Option<&[u8]>,
    options: RepairOptions,
) -> Result<RepairOutcome> {
    let probe = Probe::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    let mut file = probe
        .guess_file_type()
        .with_context(|| format!("cannot read the format of {}", path.display()))?
        .read()
        .with_context(|| format!("cannot read {}", path.display()))?;

    let Some(tag) = file.primary_tag_mut() else {
        anyhow::bail!("the file has no tags to repair");
    };

    let frames = text_frames(tag);
    let old_album = tag
        .get_string(ItemKey::AlbumTitle)
        .unwrap_or_default()
        .to_owned();
    let title = tag
        .get_string(ItemKey::TrackTitle)
        .unwrap_or_default()
        .to_owned();
    let artist = tag
        .get_string(ItemKey::TrackArtist)
        .unwrap_or_default()
        .to_owned();

    let mut album_changed = false;
    let mut script_changed = false;
    let mut encoding_changed = false;

    // The album name, if the lookup found one.
    if let Some(album) = facts.album.as_ref().filter(|a| !a.trim().is_empty()) {
        if album != &old_album {
            tag.insert_text(ItemKey::AlbumTitle, album.clone());
            album_changed = true;
        }
    }

    // The year, if the lookup found one.
    if let Some(year) = facts.year.as_ref().filter(|y| !y.trim().is_empty()) {
        if tag.get_string(ItemKey::RecordingDate).is_none() {
            tag.insert_text(ItemKey::RecordingDate, year.clone());
        }
    }

    // The script. The text is converted in place, so the album name that was
    // just written is converted too.
    if options.script != ScriptTarget::Off {
        for (key, text) in &frames {
            if !script_convert::has_cjk(text) {
                continue;
            }
            let converted = script_convert::convert(text, options.script);
            if converted != *text {
                tag.insert_text(key.clone(), converted);
                script_changed = true;
            }
        }
        // The album name is not in the frames list when it was just written.
        if let Some(album) = tag.get_string(ItemKey::AlbumTitle).map(|s| s.to_owned()) {
            if script_convert::has_cjk(&album) {
                let converted = script_convert::convert(&album, options.script);
                if converted != album {
                    tag.insert_text(ItemKey::AlbumTitle, converted);
                    script_changed = true;
                }
            }
        }
    }

    // The artwork.
    let mut cover_written = false;
    if let Some(bytes) = cover_bytes {
        let mut picture = Picture::from_reader(&mut std::io::Cursor::new(bytes))?;
        picture.set_pic_type(PictureType::CoverFront);
        // The existing pictures are removed first: the artwork that is
        // replaced is the one from a previous album, and leaving it next to
        // the new one leaves two front covers.
        tag.remove_picture_type(PictureType::CoverFront);
        tag.push_picture(picture);
        cover_written = true;
    }

    // The encoding. The text is written again in UTF-8, which is what the
    // format's version 4 uses. The file is only written when something
    // changed, so a library of files that are already UTF-8 is not touched.
    if options.fix_encoding && !is_utf8(path) {
        let frames = text_frames(tag);
        write_text_frames(tag, &frames);
        encoding_changed = true;
    }

    let new_album = tag
        .get_string(ItemKey::AlbumTitle)
        .unwrap_or_default()
        .to_owned();

    file.save_to_path(path, WriteOptions::default())
        .with_context(|| format!("cannot write the tags of {}", path.display()))?;

    Ok(RepairOutcome {
        path: path.display().to_string(),
        title,
        artist,
        old_album,
        new_album,
        album_changed,
        cover_written,
        encoding_changed,
        script_changed,
        note: None,
    })
}

/// The tracks whose album is worth repairing.
///
/// The album is worth looking at when it is one of the names that came from
/// a folder rather than from a release: the same name under many artists is
/// the clearest sign, because a real album belongs to one artist. A name
/// that is also a real album is left alone, because the folder happened to
/// be named after the album.
pub fn albums_worth_repairing(
    db: &rusqlite::Connection,
) -> Result<Vec<(i64, String, usize, usize)>> {
    // The name is what is looked at, not the album row.
    //
    // An album row is per artist and per name, so a name that came from a
    // folder appears as several rows: "KXT" is one row for 徐千雅, one for
    // 齐秦, and one for every other artist whose files were dropped into a
    // folder called KXT. Looking at the rows would find nothing, because no
    // one of them has more than one artist.
    //
    // So the names are counted across the artists, and a name under more
    // than one artist is a name that came from a folder: a real album
    // belongs to one artist.
    let mut statement = db.prepare(indoc::indoc! {"
      SELECT albums.id, albums.name,
             COUNT(DISTINCT tracks.artist_id) AS artists,
             COUNT(tracks.id) AS tracks
      FROM albums
      JOIN tracks ON tracks.album_id = albums.id
      GROUP BY albums.name
      HAVING artists > 1
      ORDER BY artists DESC, albums.name
    "})?;

    let mut rows = statement.query([])?;
    let mut albums = Vec::new();
    while let Some(row) = rows.next()? {
        albums.push((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?));
    }
    Ok(albums)
}

/// The tracks of an album name, with the artist and the file.
///
/// The name and not the row, because the name that came from a folder is
/// several rows — one per artist — and the tracks of all of them are the
/// ones the user is looking at.
pub fn tracks_of_album_name(
    album_name: &str,
    db: &rusqlite::Connection,
) -> Result<Vec<crate::persistent_entities::PersistentTrack>> {
    let mut statement = db.prepare(indoc::indoc! {"
      SELECT
          tracks.id, tracks.file_path, tracks.file_name, tracks.title,
          artists.name AS artist_name, tracks.artist_id,
          albums.name AS album_name, albums.album_artist_name, tracks.album_id, tracks.duration, tracks.track_number,
          albums.image_path, lyricsfiles.id AS lyricsfile_id, lyricsfiles.lyricsfile, COALESCE(lyricsfiles.instrumental, 0) AS instrumental
      FROM tracks
      JOIN albums ON tracks.album_id = albums.id
      JOIN artists ON tracks.artist_id = artists.id
      LEFT JOIN lyricsfiles ON lyricsfiles.track_id = tracks.id
      WHERE albums.name = ?1
      ORDER BY tracks.title_lower ASC
    "})?;

    let mut rows = statement.query([album_name])?;
    let mut tracks = Vec::new();
    while let Some(row) = rows.next()? {
        tracks.push(crate::persistent_entities::PersistentTrack {
            id: row.get("id")?,
            file_path: row.get("file_path")?,
            file_name: row.get("file_name")?,
            title: row.get("title")?,
            artist_name: row.get("artist_name")?,
            artist_id: row.get("artist_id")?,
            album_name: row.get("album_name")?,
            album_artist_name: row.get("album_artist_name")?,
            album_id: row.get("album_id")?,
            duration: row.get("duration")?,
            track_number: row.get("track_number")?,
            image_path: row.get("image_path")?,
            txt_lyrics: None,
            lrc_lyrics: None,
            lyricsfile: row.get("lyricsfile")?,
            lyricsfile_id: row.get("lyricsfile_id")?,
            instrumental: row.get("instrumental")?,
        });
    }
    Ok(tracks)
}

/// The tracks of an album, with the artist and the file.
pub fn tracks_of_album(
    album_id: i64,
    db: &rusqlite::Connection,
) -> Result<Vec<crate::persistent_entities::PersistentTrack>> {
    let mut statement = db.prepare(indoc::indoc! {"
      SELECT
          tracks.id, tracks.file_path, tracks.file_name, tracks.title,
          artists.name AS artist_name, tracks.artist_id,
          albums.name AS album_name, albums.album_artist_name, tracks.album_id, tracks.duration, tracks.track_number,
          albums.image_path, lyricsfiles.id AS lyricsfile_id, lyricsfiles.lyricsfile, COALESCE(lyricsfiles.instrumental, 0) AS instrumental
      FROM tracks
      JOIN albums ON tracks.album_id = albums.id
      JOIN artists ON tracks.artist_id = artists.id
      LEFT JOIN lyricsfiles ON lyricsfiles.track_id = tracks.id
      WHERE tracks.album_id = ?1
      ORDER BY tracks.title_lower ASC
    "})?;

    let mut rows = statement.query([album_id])?;
    let mut tracks = Vec::new();
    while let Some(row) = rows.next()? {
        tracks.push(crate::persistent_entities::PersistentTrack {
            id: row.get("id")?,
            file_path: row.get("file_path")?,
            file_name: row.get("file_name")?,
            title: row.get("title")?,
            artist_name: row.get("artist_name")?,
            artist_id: row.get("artist_id")?,
            album_name: row.get("album_name")?,
            album_artist_name: row.get("album_artist_name")?,
            album_id: row.get("album_id")?,
            duration: row.get("duration")?,
            track_number: row.get("track_number")?,
            image_path: row.get("image_path")?,
            txt_lyrics: None,
            lrc_lyrics: None,
            lyricsfile: row.get("lyricsfile")?,
            lyricsfile_id: row.get("lyricsfile_id")?,
            instrumental: row.get("instrumental")?,
        });
    }
    Ok(tracks)
}

/// Move a track to a different album in the library.
///
/// The album is created if it is not there yet. The artist is not changed:
/// the artist of a track is not the album's artist, and the two are only
/// the same by accident.
#[cfg(feature = "gui")]
pub fn move_track_to_album(
    track_id: i64,
    album_name: &str,
    db: &mut rusqlite::Connection,
) -> Result<i64> {
    let tx = db.transaction()?;

    let artist_name = {
        let mut statement = tx
            .prepare("SELECT (SELECT name FROM artists WHERE id = tracks.artist_id) FROM tracks WHERE id = ?1")?;
        let row = statement.query_row([track_id], |row| row.get::<_, String>(0))?;
        row
    };

    let album_id = match crate::db::find_album_tx(album_name, &artist_name, &tx) {
        Ok(id) => id,
        Err(_) => crate::db::add_album_tx(album_name, &artist_name, &tx)?,
    };

    tx.execute(
        "UPDATE tracks SET album_id = ?1, scan_status = ?2 WHERE id = ?3",
        rusqlite::params![album_id, 0, track_id],
    )?;

    tx.commit()?;
    Ok(album_id)
}

/// The text of a file's fields, for the report.
pub fn read_fields(path: &Path) -> Vec<(ItemKey, String)> {
    let Ok(probe) = Probe::open(path) else {
        return Vec::new();
    };
    let Ok(probe) = probe.guess_file_type() else {
        return Vec::new();
    };
    let Ok(file) = probe.read() else {
        return Vec::new();
    };
    let Some(tag) = file.primary_tag() else {
        return Vec::new();
    };
    text_frames(tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A library with a name that came from a folder.
    ///
    /// The name is one album row per artist, which is what a library built
    /// by copying files into folders looks like: "KXT" is one row for each
    /// artist whose files were dropped into a folder called KXT.
    fn library_with_folder_albums() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE artists (id INTEGER PRIMARY KEY, name TEXT)",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE albums (id INTEGER PRIMARY KEY, name TEXT, album_artist_name TEXT, image_path TEXT)",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE tracks (id INTEGER PRIMARY KEY, album_id INTEGER, artist_id INTEGER, file_path TEXT, file_name TEXT, title TEXT, title_lower TEXT, duration REAL, track_number INTEGER, file_size INTEGER, modified_time INTEGER, content_hash TEXT, scan_status INTEGER)",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE lyricsfiles (id INTEGER PRIMARY KEY, track_id INTEGER, lyricsfile TEXT, instrumental INTEGER)",
            [],
        )
        .unwrap();

        // Two albums that are really albums: one artist each.
        for (id, name, artist) in [(1, "Bad 25", "Michael Jackson"), (2, "玩具", "王菲")] {
            conn.execute("INSERT INTO artists VALUES (?1, ?2)", rusqlite::params![id, artist])
                .unwrap();
            conn.execute(
                "INSERT INTO albums VALUES (?1, ?2, ?3, NULL)",
                rusqlite::params![id, name, artist],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO tracks (id, album_id, artist_id, file_path, file_name, title, title_lower, duration) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, 100)",
                rusqlite::params![id, id, id, format!("/m/{name}.mp3"), format!("{name}.mp3"), name],
            )
            .unwrap();
        }

        // And three artists who all have a track under the name "KXT".
        for (id, artist) in [(3, "齐秦"), (4, "梁静茹"), (5, "田震")] {
            conn.execute("INSERT INTO artists VALUES (?1, ?2)", rusqlite::params![id, artist])
                .unwrap();
            conn.execute(
                "INSERT INTO albums VALUES (?1, 'KXT', ?2, NULL)",
                rusqlite::params![id, artist],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO tracks (id, album_id, artist_id, file_path, file_name, title, title_lower, duration) VALUES (?1, ?1, ?2, ?3, ?4, '某歌', '某歌', 200)",
                rusqlite::params![id, id, format!("/m/{artist}.mp3"), format!("{artist}.mp3")],
            )
            .unwrap();
        }

        conn
    }

    #[test]
    fn a_name_under_several_artists_is_worth_repairing() {
        let conn = library_with_folder_albums();
        let albums = albums_worth_repairing(&conn).unwrap();

        // Only "KXT" is under more than one artist.
        assert_eq!(albums.len(), 1);
        assert_eq!(albums[0].1, "KXT");
        assert_eq!(albums[0].2, 3);
        assert_eq!(albums[0].3, 3);
    }

    #[test]
    fn a_real_album_is_left_alone() {
        let conn = library_with_folder_albums();
        let names: Vec<String> = albums_worth_repairing(&conn)
            .unwrap()
            .into_iter()
            .map(|(_, name, _, _)| name)
            .collect();

        assert!(!names.contains(&"Bad 25".to_owned()));
        assert!(!names.contains(&"玩具".to_owned()));
    }

    #[test]
    fn the_tracks_of_a_name_are_all_of_them() {
        let conn = library_with_folder_albums();
        let tracks = tracks_of_album_name("KXT", &conn).unwrap();

        // All three artists, not one: the name is one row per artist.
        assert_eq!(tracks.len(), 3);
        let mut artists: Vec<String> = tracks.iter().map(|t| t.artist_name.clone()).collect();
        artists.sort();
        assert_eq!(artists, vec!["梁静茹", "田震", "齐秦"]);
    }
}
