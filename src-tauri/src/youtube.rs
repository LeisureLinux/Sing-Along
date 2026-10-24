//! Correcting the metadata of a file that `yt-dlp` downloaded.
//!
//! `yt-dlp` writes what YouTube tells it, and YouTube tells it very little.
//! For a song on YouTube Music the album comes back as a compilation
//! ("2021熱歌合集" — a "2021 hits" collection), the cover is a square video
//! thumbnail rather than the album art, and the Chinese text is in whatever
//! script the uploader's metadata used.
//!
//! None of that is what the listener wants to see in a player. This module
//! takes the file and makes it right:
//!
//! * the album and the cover are looked up from MusicBrainz, which knows the
//!   real release, and the cover art archive has the real artwork;
//! * the text is converted between the Chinese scripts, using the same tables
//!   the lyrics conversion uses, so a track downloaded in Traditional shows up
//!   in Simplified when that is what the user asked for.
//!
//! The lookup is by title and artist, and the release that matches is chosen
//! by how well the recording fits: the year the song was released beats the
//! year a compilation was put together, and a release by the artist beats one
//! they merely appear on.

use crate::script_convert::{self, ScriptTarget};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

/// The MusicBrainz search endpoint.
const MUSICBRAINZ_SEARCH: &str = "https://musicbrainz.org/ws/2/recording/";
/// The cover art archive, which serves the artwork for a MusicBrainz release.
const COVER_ART: &str = "https://coverartarchive.org/release/";

/// A recording as MusicBrainz returns it.
#[derive(Debug, Deserialize)]
struct RecordingSearchResponse {
    #[serde(default)]
    recordings: Vec<MusicBrainzRecording>,
}

#[derive(Debug, Deserialize)]
struct MusicBrainzRecording {
    #[serde(default)]
    title: String,
    #[serde(default)]
    releases: Vec<MusicBrainzRelease>,
}

#[derive(Debug, Deserialize)]
struct MusicBrainzRelease {
    #[serde(default)]
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    date: Option<String>,
    #[serde(default)]
    country: Option<String>,
    /// The release status: "Official", "Bootleg", "Compilation" and so on.
    #[serde(default)]
    status: Option<String>,
    /// The kinds of release: "Album", "Single", "Compilation" and so on.
    #[serde(default, rename = "secondary-types")]
    secondary_types: Vec<String>,
    #[serde(default)]
    #[serde(rename = "artist-credit")]
    artist_credit: Vec<ArtistCredit>,
}

#[derive(Debug, Deserialize)]
struct ArtistCredit {
    #[serde(default)]
    name: String,
}

/// The artwork the cover art archive has for a release.
#[derive(Debug, Deserialize)]
struct CoverArtResponse {
    #[serde(default)]
    images: Vec<CoverArtImage>,
}

#[derive(Debug, Deserialize)]
struct CoverArtImage {
    #[serde(default)]
    image: String,
    #[serde(default)]
    front: bool,
    #[serde(default)]
    thumbnails: CoverArtThumbnails,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct CoverArtThumbnails {
    #[serde(default)]
    large: Option<String>,
    #[serde(default)]
    #[serde(rename = "1200")]
    large_1200: Option<String>,
    #[serde(default)]
    small: Option<String>,
}

/// What was found for a track.
#[derive(Debug, Clone)]
pub struct TrackCorrection {
    /// The album the recording is really on.
    pub album: Option<String>,
    /// The release year, when the source has one.
    pub year: Option<String>,
    /// The cover art URL.
    pub cover_url: Option<String>,
    /// Where the album name came from.
    pub album_source: AlbumSource,
}

/// Where the album name came from.
///
/// Recorded because the difference matters to the user: a name from the
/// release database is the real one, while a name from YouTube is often a
/// compilation that has nothing to do with the recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlbumSource {
    /// The real release, found in the release database.
    Release,
    /// The name YouTube gave, which is what was there before.
    YouTube,
}

/// What the user wants the metadata to be.
#[derive(Debug, Clone, Copy)]
pub struct CorrectionOptions {
    /// The script the text should end up in.
    pub script: ScriptTarget,
    /// Whether the cover should be looked up and written.
    pub fix_cover: bool,
}

impl Default for CorrectionOptions {
    fn default() -> Self {
        Self {
            script: ScriptTarget::Off,
            fix_cover: true,
        }
    }
}

/// The shortest gap between two requests to the release database.
///
/// They ask for no more than one request a second, and answer anything
/// faster with "503 Service Unavailable". A download triggers a lookup, so a
/// batch of downloads would otherwise trip the limit — and the failure looks
/// like the service being down rather than like being too fast.
const REQUEST_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1100);

/// The time of the last request, so the next one can wait.
///
/// A process-wide lock rather than one per client: the limit is on the
/// account, not on the connection, and the application makes one client.
static LAST_REQUEST: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);

/// Wait until the next request may be made.
fn wait_for_the_rate_limit() {
    let Ok(mut last) = LAST_REQUEST.lock() else {
        // A poisoned lock means another request panicked. Waiting a full
        // interval is the safe answer.
        std::thread::sleep(REQUEST_INTERVAL);
        return;
    };

    if let Some(previous) = *last {
        let elapsed = previous.elapsed();
        if elapsed < REQUEST_INTERVAL {
            std::thread::sleep(REQUEST_INTERVAL - elapsed);
        }
    }

    *last = Some(std::time::Instant::now());
}

/// The user agent MusicBrainz asks for.
///
/// They require one that identifies the application and gives a way to reach
/// whoever runs it, and they rate-limit anonymous traffic. The address is the
/// project's own contact address, the same one the AppStream metadata already
/// publishes, so nothing private is sent to a third party.
const USER_AGENT: &str = "Sing-Along/2.9 ( https://github.com/LeisureLinux/Sing-Along; contact@freelamp.com )";

/// Look up the real release and artwork for a track.
pub async fn lookup(
    client: &reqwest::Client,
    title: &str,
    artist: &str,
) -> Result<Option<TrackCorrection>> {
    let release = search_recording(client, title, artist)
        .await?
        .ok_or_else(|| anyhow::anyhow!("no release found for {artist} - {title}"))?;

    let album = Some(release.title.clone());
    let year = release.date.as_ref().map(|date| date.chars().take(4).collect());

    // The artwork is looked for on this release and, when it has none, on the
    // other releases the recording appears on. A release without a cover is
    // common — a single, or a release nobody scanned — and the artwork of a
    // different release of the same song is far better than none.
    let cover_url = match cover_for_release(client, &release.id).await? {
        Some(url) => Some(url),
        None => {
            let others = other_releases(client, title, artist).await?;
            let mut found = None;
            for other in others {
                if other.id == release.id {
                    continue;
                }
                if let Some(url) = cover_for_release(client, &other.id).await? {
                    found = Some(url);
                    break;
                }
            }
            found
        }
    };

    Ok(Some(TrackCorrection {
        album,
        year,
        cover_url,
        album_source: AlbumSource::Release,
    }))
}

/// The other releases the recording appears on, best first.
///
/// Used when the chosen release has no artwork: the cover of the same song
/// on a different release is a better answer than no cover at all.
async fn other_releases(
    client: &reqwest::Client,
    title: &str,
    artist: &str,
) -> Result<Vec<MusicBrainzRelease>> {
    let query = format!("recording:\"{title}\" AND artist:\"{artist}\"");
    let url = reqwest::Url::parse_with_params(MUSICBRAINZ_SEARCH, [
        ("query", query.as_str()),
        ("fmt", "json"),
        ("limit", "20"),
    ])?;

    wait_for_the_rate_limit();

    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .send()
        .await
        .context("the release database is unreachable")?;

    if !response.status().is_success() {
        return Ok(Vec::new());
    }

    let parsed: RecordingSearchResponse = match response.json().await {
        Ok(parsed) => parsed,
        Err(_) => return Ok(Vec::new()),
    };

    let mut releases: Vec<MusicBrainzRelease> = parsed
        .recordings
        .into_iter()
        .filter(|recording| titles_match(&recording.title, title))
        .flat_map(|recording| recording.releases)
        .collect();
    releases.sort_by_key(|release| std::cmp::Reverse(release_score(release, artist)));
    Ok(releases)
}

/// Find the release that best matches a title and an artist.
///
/// MusicBrainz returns the matches sorted by score, but the score is
/// computed over a lot of fields this project does not use, and a
/// compilation entry often outranks the original release. So the candidates
/// are re-ranked here, by the things that actually identify a release: the
/// artist is the one on it, it is not a compilation, and it is not a bootleg.
///
/// The unit of the answer is the release, not the recording, because the
/// release is what carries the album name and the artwork. A recording can
/// appear on several releases, and only one of them is the album the user
/// would name.
async fn search_recording(
    client: &reqwest::Client,
    title: &str,
    artist: &str,
) -> Result<Option<MusicBrainzRelease>> {
    let query = format!("recording:\"{title}\" AND artist:\"{artist}\"");
    let url = reqwest::Url::parse_with_params(MUSICBRAINZ_SEARCH, [
        ("query", query.as_str()),
        ("fmt", "json"),
        ("limit", "20"),
    ])?;

    wait_for_the_rate_limit();

    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .send()
        .await
        .context("the release database is unreachable")?;

    if !response.status().is_success() {
        anyhow::bail!(
            "the release database answered {}",
            response.status()
        );
    }

    let parsed: RecordingSearchResponse = response
        .json()
        .await
        .context("the release database sent something unreadable")?;

    Ok(pick_best(parsed.recordings, title, artist))
}

/// The name the database gives to a release by many artists.
const VARIOUS_ARTISTS: &str = "various artists";

/// Choose the release that is most likely the one the user meant.
///
/// The recordings the search returns are flattened into their releases first:
/// a recording appears once per release it is on, and only one of those
/// releases is the album.
fn pick_best(
    recordings: Vec<MusicBrainzRecording>,
    title: &str,
    artist: &str,
) -> Option<MusicBrainzRelease> {
    // The title has to match: the search is a phrase search, but the
    // recordings it returns include the ones where the title only appears in
    // the release's track list.
    let releases: Vec<MusicBrainzRelease> = recordings
        .into_iter()
        .filter(|recording| titles_match(&recording.title, title))
        .flat_map(|recording| recording.releases)
        .collect();

    if releases.is_empty() {
        return None;
    }

    releases.into_iter().max_by_key(|release| release_score(release, artist))
}

/// How much a release looks like the album the user meant.
fn release_score(release: &MusicBrainzRelease, artist: &str) -> i64 {
    let mut score = 0i64;

    // The release by the artist, not one they merely appear on.
    let credited: Vec<&str> = release
        .artist_credit
        .iter()
        .map(|credit| credit.name.as_str())
        .collect();
    if credited.iter().any(|name| names_match(name, artist)) {
        score += 200;
    }
    // A release by many artists is a collection. Its name is not the album
    // the track belongs to, and this is the single most common wrong answer:
    // the song appears on a "greatest hits" of some unrelated artist.
    if credited
        .iter()
        .any(|name| name.eq_ignore_ascii_case(VARIOUS_ARTISTS))
    {
        score -= 300;
    }
    if credited.len() > 1 {
        score -= 50;
    }

    // An official release is a real publication; a bootleg is not.
    if let Some(status) = &release.status {
        if status.eq_ignore_ascii_case("official") {
            score += 40;
        }
        if status.eq_ignore_ascii_case("bootleg") {
            score -= 200;
        }
    }

    // A compilation is a collection, even when it is by the artist alone.
    if release
        .secondary_types
        .iter()
        .any(|kind| kind.eq_ignore_ascii_case("compilation"))
    {
        score -= 120;
    }

    // An earlier release is more likely to be the original.
    if let Some(year) = release
        .date
        .as_ref()
        .and_then(|date| date.get(..4))
        .and_then(|year| year.parse::<i64>().ok())
    {
        score += (2026 - year).clamp(0, 40);
    }

    score
}

/// The cover art of a release.
async fn cover_for_release(client: &reqwest::Client, release_id: &str) -> Result<Option<String>> {
    if release_id.is_empty() {
        return Ok(None);
    }

    let url = format!("{COVER_ART}{release_id}");
    wait_for_the_rate_limit();

    let response = match client
        .get(&url)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .send()
        .await
    {
        Ok(response) => response,
        // The archive is a separate service and its failure is not a failure
        // of the lookup: the album name is still right without the artwork.
        Err(_) => return Ok(None),
    };

    if !response.status().is_success() {
        return Ok(None);
    }

    let parsed: CoverArtResponse = match response.json().await {
        Ok(parsed) => parsed,
        Err(_) => return Ok(None),
    };

    Ok(parsed
        .images
        .iter()
        .find(|image| image.front)
        .or_else(|| parsed.images.first())
        .and_then(|image| {
            image
                .thumbnails
                .large_1200
                .clone()
                .or_else(|| image.thumbnails.large.clone())
                .or_else(|| image.thumbnails.small.clone())
                .or(Some(image.image.clone()))
        }))
}

/// Whether two titles are the same song.
///
/// The scripts differ, the punctuation differs, and the spacing differs. The
/// script is the hard one: 城裡的月光 and 城里的月光 are the same song, and the
/// release database has both spellings depending on where the release was
/// published. So the comparison happens in one script — the text is converted
/// to Simplified on both sides first, which is the same conversion the
/// application offers the user, so the two use the same tables.
fn titles_match(left: &str, right: &str) -> bool {
    normalise_title(left) == normalise_title(right)
}

/// A title reduced to a comparable form.
///
/// The Chinese is converted to one script, and everything that is not a
/// letter or a digit is dropped: the punctuation and the spacing differ
/// between the database and the uploader, and they carry no information.
fn normalise_title(title: &str) -> String {
    let converted = script_convert::convert(title, ScriptTarget::Simplified);
    converted
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Whether two artist names are the same artist.
///
/// The database writes the credit with the artists joined by a comma, and a
/// track can be by more than one, so a name that contains the other counts as
/// a match.
fn names_match(left: &str, right: &str) -> bool {
    let left = normalise_title(left);
    let right = normalise_title(right);
    left == right || left.contains(&right) || right.contains(&left)
}

/// Convert the Chinese script of every text field of an audio file.
///
/// The fields are the ones a player shows: title, album, artist and the
/// comment. A song whose name is in Traditional shows up in Simplified when
/// the user asked for Simplified, and the same is true of the album.
pub fn convert_file_script(path: &Path, target: ScriptTarget) -> Result<()> {
    if target == ScriptTarget::Off {
        return Ok(());
    }
    if !script_convert::has_cjk(&read_text_fields(path)?) {
        return Ok(());
    }

    use lofty::config::WriteOptions;
    use lofty::file::{AudioFile, TaggedFileExt};
    use lofty::probe::Probe;
    use lofty::tag::{ItemKey, ItemValue, TagItem};

    let file_probe = Probe::open(path)
        .with_context(|| format!("cannot open {}", path.display()))?;
    let mut file = file_probe
        .guess_file_type()
        .with_context(|| format!("cannot read the format of {}", path.display()))?
        .read()
        .with_context(|| format!("cannot read {}", path.display()))?;

    let Some(tag) = file.primary_tag_mut() else {
        // A file with no tags has nothing to convert.
        return Ok(());
    };

    // The tag is taken apart and put back together, because lofty has no
    // mutable iterator over the items: a field is removed, converted and
    // inserted again. Only the text fields are touched; the picture and the
    // binary frames are left alone.
    let keys: Vec<ItemKey> = tag.items().map(|item| item.key()).collect();
    let mut keys = keys;
    keys.dedup();

    for key in keys {
        let items: Vec<TagItem> = tag.take_filter(key.clone(), |_| true).collect();
        for item in items {
            let Some(text) = item.value().text().map(|text| text.to_owned()) else {
                // A picture or a binary frame: put it back untouched.
                tag.insert(item);
                continue;
            };
            let converted = script_convert::convert(&text, target);
            if converted == text {
                tag.insert(item);
            } else {
                tag.insert(TagItem::new(key.clone(), ItemValue::Text(converted)));
            }
        }
    }

    file.save_to_path(path, WriteOptions::default())
        .with_context(|| format!("cannot write the tags of {}", path.display()))?;

    Ok(())
}

/// Every text field of a file, joined, for the "does this even need
/// converting" check.
fn read_text_fields(path: &Path) -> Result<String> {
    use lofty::file::TaggedFileExt;
    use lofty::probe::Probe;

    let Ok(file_probe) = Probe::open(path) else {
        return Ok(String::new());
    };
    let probe = match file_probe.guess_file_type() {
        Ok(probe) => probe,
        Err(_) => return Ok(String::new()),
    };
    let file = match probe.read() {
        Ok(file) => file,
        Err(_) => return Ok(String::new()),
    };
    let Some(tag) = file.primary_tag() else {
        return Ok(String::new());
    };

    Ok(tag
        .items()
        .filter_map(|item| item.value().text().map(|text| text.to_owned()))
        .collect::<Vec<_>>()
        .join("\n"))
}

/// Set the album, the year and the cover of a file.
///
/// The album and the year are text frames; the cover is a picture. The
/// picture is fetched separately, because the archive serves the artwork from
/// a different host than the metadata.
pub fn write_metadata(
    path: &Path,
    album: Option<&str>,
    year: Option<&str>,
    cover_bytes: Option<&[u8]>,
) -> Result<()> {
    use lofty::config::WriteOptions;
    use lofty::file::{AudioFile, TaggedFileExt};
    use lofty::picture::{Picture, PictureType};
    use lofty::probe::Probe;
    use lofty::tag::ItemKey;

    let file_probe =
        Probe::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    let mut file = file_probe
        .guess_file_type()
        .with_context(|| format!("cannot read the format of {}", path.display()))?
        .read()
        .with_context(|| format!("cannot read {}", path.display()))?;

    {
        let Some(tag) = file.primary_tag_mut() else {
            anyhow::bail!("the downloaded file has no tags to correct");
        };

        if let Some(album) = album.filter(|value| !value.trim().is_empty()) {
            tag.insert_text(ItemKey::AlbumTitle, album.to_owned());
        }
        if let Some(year) = year.filter(|value| !value.trim().is_empty()) {
            // `RecordingDate`, not `Year`: the ID3v2 frame that holds the
            // release date is TDRC, and lofty maps that to `RecordingDate`.
            // Writing `Year` would put a frame nothing reads, and the year
            // YouTube wrote would stay.
            tag.insert_text(ItemKey::RecordingDate, year.to_owned());
        }
    }

    if let Some(bytes) = cover_bytes {
        let mut picture = Picture::from_reader(&mut std::io::Cursor::new(bytes))?;
        picture.set_pic_type(PictureType::CoverFront);
        if let Some(tag) = file.primary_tag_mut() {
            // The existing pictures are removed first.
            //
            // `yt-dlp` embeds the video thumbnail, which is a picture of the
            // video, not the album art, and it is a different size and shape.
            // Adding the real cover next to it leaves two front covers, and a
            // player shows one of them — the wrong one half the time.
            tag.remove_picture_type(PictureType::CoverFront);
            tag.push_picture(picture);
        }
    }

    file.save_to_path(path, WriteOptions::default())
        .with_context(|| format!("cannot write the tags of {}", path.display()))?;

    Ok(())
}

/// Download the bytes of a cover image.
pub async fn download_cover(client: &reqwest::Client, url: &str) -> Result<Vec<u8>> {
    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .send()
        .await
        .context("the artwork is unreachable")?;

    if !response.status().is_success() {
        anyhow::bail!("the artwork answered {}", response.status());
    }

    let bytes = response.bytes().await.context("the artwork could not be read")?;
    Ok(bytes.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(title: &str, artist: &str, date: &str, status: &str) -> MusicBrainzRelease {
        MusicBrainzRelease {
            id: "id".to_owned(),
            title: title.to_owned(),
            date: Some(date.to_owned()),
            country: None,
            status: Some(status.to_owned()),
            secondary_types: Vec::new(),
            artist_credit: vec![ArtistCredit {
                name: artist.to_owned(),
            }],
        }
    }

    #[test]
    fn the_original_release_beats_a_compilation() {
        let recordings = vec![
            MusicBrainzRecording {
                title: "城裡的月光".to_owned(),
                releases: vec![release("2021熱歌合集", "Various Artists", "2021", "Official")],
            },
            MusicBrainzRecording {
                title: "城裡的月光".to_owned(),
                releases: vec![release("遺憾", "許美靜", "1996", "Official")],
            },
        ];

        let picked = pick_best(recordings, "城裡的月光", "許美靜").unwrap();
        assert_eq!(picked.title, "遺憾");
    }

    #[test]
    fn a_release_by_the_artist_beats_a_bootleg() {
        let bootleg = MusicBrainzRelease {
            secondary_types: Vec::new(),
            ..release("城裡的月光", "許美靜", "2005", "Bootleg")
        };
        let official = MusicBrainzRelease {
            secondary_types: Vec::new(),
            ..release("遺憾", "許美靜", "1996", "Official")
        };

        let bootleg_score = release_score(&bootleg, "許美靜");
        let official_score = release_score(&official, "許美靜");
        assert!(official_score > bootleg_score);
    }

    #[test]
    fn a_compilation_by_the_artist_loses_to_an_album() {
        let compilation = MusicBrainzRelease {
            secondary_types: vec!["Compilation".to_owned()],
            ..release("精選輯", "許美靜", "1999", "Official")
        };
        let album = MusicBrainzRelease {
            secondary_types: Vec::new(),
            ..release("遺憾", "許美靜", "1996", "Official")
        };

        assert!(release_score(&album, "許美靜") > release_score(&compilation, "許美靜"));
    }

    #[test]
    fn a_recording_whose_title_differs_is_not_matched() {
        let recordings = vec![MusicBrainzRecording {
            title: "Something else".to_owned(),
            releases: vec![release("Album", "Artist", "2000", "Official")],
        }];
        assert!(pick_best(recordings, "城裡的月光", "許美靜").is_none());
    }

    #[test]
    fn titles_match_across_scripts_and_punctuation() {
        assert!(titles_match("城裡的月光", "城里的月光"));
        assert!(titles_match("Don't Stop", "Dont Stop"));
        assert!(!titles_match("城裡的月光", "城裡的太陽"));
    }

    #[test]
    fn artist_names_match_when_one_contains_the_other() {
        assert!(names_match("許美靜", "許美靜"));
        assert!(names_match("許美靜 & 友", "許美靜"));
        assert!(!names_match("許美靜", "陳佳明"));
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// The metadata correction, exercised against a real file when one is
    /// available.
    #[test]
    fn the_script_conversion_rewrites_the_tags() {
        let path = std::path::Path::new("/tmp/mtest.mp3");
        if !path.exists() {
            eprintln!("skipped: no test file");
            return;
        }

        // The file was downloaded with Traditional Chinese tags.
        convert_file_script(path, ScriptTarget::Simplified).unwrap();

        use lofty::file::TaggedFileExt;
        use lofty::probe::Probe;
        let probe = Probe::open(path).unwrap();
        let file = probe.guess_file_type().unwrap().read().unwrap();
        let tag = file.primary_tag().unwrap();
        let title = tag.get_string(lofty::tag::ItemKey::TrackTitle).unwrap();
        let album = tag.get_string(lofty::tag::ItemKey::AlbumTitle).unwrap();
        println!("title={} album={}", title, album);
        assert!(title.contains("城里的月光"), "title was not converted: {title}");
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;

    /// The release lookup, against the real service.
    ///
    /// Ignored by default because it needs the network. Run it with:
    ///
    /// ```text
    /// cargo test live_tests -- --ignored --nocapture
    /// ```
    #[tokio::test]
    #[ignore]
    async fn a_release_is_found_for_a_known_song() {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .unwrap();

        let correction = lookup(&client, "城裡的月光", "許美靜")
            .await
            .expect("the lookup should succeed")
            .expect("a release should be found");

        println!("album={:?} year={:?} cover={:?}", correction.album, correction.year, correction.cover_url);
        assert!(correction.album.is_some());
        assert!(correction.cover_url.is_some());
    }
}

#[cfg(test)]
mod tag_tests {
    use super::*;

    /// A one-pixel PNG, so that the tag writer has a picture it recognises.
    const ONE_PIXEL_PNG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
        0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
        0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
        0x42, 0x60, 0x82,
    ];

    /// A file with the shapes `yt-dlp` produces, built from any MP3 on the
    /// machine.
    ///
    /// The point of the test is what the correction does to a file that
    /// already has a video thumbnail and a date, so the test needs such a
    /// file. Any MP3 will do: the tags are what matter, not the audio.
    fn make_test_file(path: &std::path::Path) -> bool {
        make_test_file_inner(path).is_ok()
    }

    fn make_test_file_inner(path: &std::path::Path) -> Result<()> {
        use lofty::config::WriteOptions;
        use lofty::file::{AudioFile, TaggedFileExt};
        use lofty::picture::{MimeType, Picture, PictureType};
        use lofty::probe::Probe;
        use lofty::tag::{ItemKey, ItemValue, TagItem};

        // An MP3 to copy. Any audio file would do, but the picture frames
        // differ per format.
        let source = [
            "/usr/share/javascript/mathjax/unpacked/extensions/a11y/invalid_keypress.mp3",
            "/usr/share/sounds/sound-icons/guitar-12.wav",
        ]
        .into_iter()
        .map(std::path::PathBuf::from)
        .find(|candidate| candidate.exists());

        let Some(source) = source else {
            anyhow::bail!("no audio file on the machine");
        };

        std::fs::copy(&source, path)?;

        let probe = Probe::open(path)?;
        let mut file = probe.guess_file_type()?.read()?;
        let tag = file
            .primary_tag_mut()
            .ok_or_else(|| anyhow::anyhow!("the file has no tags"))?;

        tag.insert_text(ItemKey::TrackTitle, "城裡的月光".to_owned());
        tag.insert_text(ItemKey::AlbumTitle, "2021熱歌合集".to_owned());
        tag.insert_text(ItemKey::RecordingDate, "2021".to_owned());
        // The frame YouTube writes the date into, in a form that is not the
        // one a player reads.
        tag.insert(TagItem::new(
            ItemKey::Year,
            ItemValue::Text("2021".to_owned()),
        ));

        // Two front covers: the video thumbnail and the album art. The data
        // is a real one-pixel PNG, because the tag writer refuses to store a
        // picture it cannot recognise.
        for _ in 0..2 {
            let picture = Picture::unchecked(ONE_PIXEL_PNG.to_vec())
                .pic_type(PictureType::CoverFront)
                .mime_type(MimeType::Png)
                .build();
            tag.push_picture(picture);
        }

        file.save_to_path(path, WriteOptions::default())?;
        Ok(())
    }

    /// The year has to land in the frame a player reads.
    ///
    /// `ItemKey::Year` is a different key from the one ID3v2 uses for the
    /// release date, so writing it would leave the year YouTube wrote in
    /// place and the correction would appear to have done nothing.
    #[test]
    fn the_year_lands_in_the_frame_a_player_reads() {
        let path = std::env::temp_dir().join("singalong-year-test.mp3");
        if !make_test_file(&path) {
            eprintln!("skipped: no audio file on the machine to work with");
            return;
        }

        write_metadata(&path, Some("遺憾"), Some("1996"), Some(ONE_PIXEL_PNG)).unwrap();

        use lofty::file::TaggedFileExt;
        use lofty::probe::Probe;
        let probe = Probe::open(&path).unwrap();
        let file = probe.guess_file_type().unwrap().read().unwrap();
        let tag = file.primary_tag().unwrap();

        assert_eq!(
            tag.get_string(lofty::tag::ItemKey::RecordingDate),
            Some("1996")
        );
        assert_eq!(
            tag.get_string(lofty::tag::ItemKey::AlbumTitle),
            Some("遺憾")
        );

        std::fs::remove_file(&path).ok();
    }

    /// The cover has to be replaced, not joined.
    ///
    /// `yt-dlp` embeds the video thumbnail as a front cover, which is a
    /// picture of the video rather than the album art. Adding the album art
    /// next to it leaves several front covers, and a player shows one of them
    /// — the wrong one half the time.
    ///
    /// The invariant is about the result rather than the mechanism: after the
    /// correction, the front cover is the album art. The file this needs is
    /// one with a cover that is not the album art, which is what a download
    /// produces.
    #[test]
    fn the_cover_is_replaced_not_joined() {
        let path = std::env::temp_dir().join("singalong-cover-test.mp3");
        if !make_test_file(&path) {
            eprintln!("skipped: no audio file on the machine to work with");
            return;
        }

        use lofty::file::TaggedFileExt;
        use lofty::probe::Probe;
        let probe = Probe::open(&path).unwrap();
        let file = probe.guess_file_type().unwrap().read().unwrap();
        let before = file
            .primary_tag()
            .unwrap()
            .pictures()
            .iter()
            .filter(|picture| {
                picture.pic_type() == lofty::picture::PictureType::CoverFront
            })
            .count();
        assert!(before >= 1, "the test file should start with a cover");

        // A cover that is unmistakably not the one being written.
        write_metadata(&path, Some("遺憾"), Some("1996"), Some(ONE_PIXEL_PNG)).unwrap();

        let probe = Probe::open(&path).unwrap();
        let file = probe.guess_file_type().unwrap().read().unwrap();
        let tag = file.primary_tag().unwrap();

        let fronts: Vec<&lofty::picture::Picture> = tag
            .pictures()
            .iter()
            .filter(|picture| {
                picture.pic_type() == lofty::picture::PictureType::CoverFront
            })
            .collect();
        assert_eq!(fronts.len(), 1, "there should be exactly one front cover");
        assert_eq!(
            fronts[0].data(),
            ONE_PIXEL_PNG,
            "the cover that is left should be the album art, not the thumbnail"
        );

        std::fs::remove_file(&path).ok();
    }}
