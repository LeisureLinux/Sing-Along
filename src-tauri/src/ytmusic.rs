//! Searching YouTube Music.
//!
//! The download dialog used to take a pasted URL, which is the wrong shape
//! for the thing people want: a list of songs they can pick from. YouTube
//! Music has a catalogue and a search, and the search answers with the songs
//! rather than with the videos.
//!
//! ## The endpoint
//!
//! `music.youtube.com/youtubei/v1/search` is the same internal API the web
//! player uses, and it is not a documented interface. It answers a JSON
//! request that names the client, the language and the country, and it
//! answers with the raw renderer structures the player draws.
//!
//! The filter in the request decides what comes back. The one for songs is
//! `EgWKAQIIAWoMEA4QChADEAQQCRAF`; without it the answer is albums and
//! artists, which carry no video id and so cannot be downloaded.
//!
//! ## The country and the language
//!
//! The country decides which catalogue is searched, and the language
//! decides how the results are spelled. Both come from the user's locale,
//! because a Chinese reader in China wants the songs the Chinese charts
//! have and the titles in Chinese, and a reader in Germany wants the German
//! ones. The chart is the extreme case: the same endpoint, with a chart's
//! browse id, is the list of the twenty songs the country is listening to.

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;

/// The endpoint the web player searches through.
const SEARCH_ENDPOINT: &str = "https://music.youtube.com/youtubei/v1/search";

/// The request filter that asks for songs.
///
/// Without it the answer is albums and artists. The three parts are the
/// filter itself, the "songs" kind, and the spelling correction.
const SONGS_FILTER: &str = "EgWKAQIIAWoMEA4QChADEAQQCRAF";

/// The client the web player identifies as.
const CLIENT_NAME: &str = "WEB_REMIX";
const CLIENT_VERSION: &str = "1.20240101.00.00";

/// The client number the headers carry.
///
/// The name and the number have to agree: the body says which client it is
/// and the headers say the same thing, and the endpoint checks that they do.
const CLIENT_NUMBER: &str = "67";

/// A song from the catalogue.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Song {
    /// The video id, which is what the download uses.
    pub video_id: String,
    /// The title.
    pub title: String,
    /// The artist.
    pub artist: String,
    /// The album, when the entry has one.
    pub album: Option<String>,
    /// The length, in seconds.
    pub duration: Option<f64>,
    /// The link to the song.
    pub url: String,
}

/// The country whose catalogue is searched.
///
/// The catalogue is not the same everywhere: the songs that are popular in
/// Taiwan are not the ones that are popular in Germany, and a search for an
/// artist returns different things in different countries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    /// The two-letter country code the endpoint takes.
    pub code: &'static str,
    /// The language the results are spelled in.
    pub language: &'static str,
}

/// Whether a result is a song rather than a collection.
///
/// The search for the popular songs answers with the songs *and* with the
/// collections that were put together around them: a "20 songs of X" album,
/// a DJ mix, a playlist. Those have no single artist, they are long, and
/// they are not what a person means by a song.
///
/// The sign is the length: a song's title is short, and a collection's title
/// is a sentence with a count in it. The limit is generous, because a song
/// with a long subtitle — "如果可以 (電影《月老》主題曲)" — is still a song.
const MAX_SONG_TITLE_LENGTH: usize = 60;

/// Whether a result looks like a song rather than a collection.
fn looks_like_a_song(song: &Song) -> bool {
    if song.title.chars().count() > MAX_SONG_TITLE_LENGTH {
        return false;
    }

    // A collection names a number of songs, and usually a chart or a list.
    let lower = song.title.to_lowercase();
    for marker in [
        "top 10", "top 20", "top hits", "playlist", "mix", "合集", "精选", "热歌榜", "熱門榜", "榜单",
        "串烧", "串燒", "精选集", "vol.",
    ] {
        if lower.contains(marker) {
            return false;
        }
    }

    // The count of the songs in the collection: "20首", "50 songs".
    if song.title.contains("首") || lower.contains("songs") {
        return false;
    }

    // A song has an artist. A collection has a channel, and the two are
    // told apart by the length of the name: an artist is a name, a channel
    // is a brand.
    !song.artist.is_empty() && song.artist.chars().count() <= 40
}

/// The query that finds the songs the region is listening to.
///
/// The charts themselves are not available without a paid subscription, so
/// the popular songs are found by searching for the words the region uses
/// for them. The search is answered from that region's catalogue, so the
/// answer is the songs that region is listening to.
fn trending_query(language: &str) -> &'static str {
    match language {
        "zh-Hans" => "热门歌曲",
        "zh-Hant" => "熱門歌曲",
        "ja" => "人気曲",
        "ko" => "인기 노래",
        "es" => "canciones populares",
        "fr" => "chansons populaires",
        "de" => "beliebte Songs",
        "it" => "canzoni popolari",
        "pt" => "músicas populares",
        "pl" => "popularne piosenki",
        "tr" => "popüler şarkılar",
        "id" => "lagu populer",
        "vi" => "bài hát phổ biến",
        "th" => "เพลงยอดนิยม",
        "ru" => "популярные песни",
        "uk" => "популярні пісні",
        _ => "top hits",
    }
}

/// The region that goes with a locale.
///
/// The locale says the language, and the language says the country, because
/// that is the only connection between them that is reliable: a reader
/// whose interface is in Simplified Chinese is most likely in China, Singapore
/// or Malaysia, and the Traditional one in Taiwan or Hong Kong. Where the
/// language and the country could disagree, the language wins, because it
/// is what the user chose.
pub fn region_for_locale(locale: &str) -> Region {
    match locale {
        "zh-Hans" => Region {
            code: "TW",
            language: "zh-Hans",
        },
        "zh-Hant" => Region {
            code: "TW",
            language: "zh-Hant",
        },
        "ja" => Region {
            code: "JP",
            language: "ja",
        },
        "ko" => Region {
            code: "KR",
            language: "ko",
        },
        "en" => Region {
            code: "US",
            language: "en",
        },
        "es" => Region {
            code: "ES",
            language: "es",
        },
        "fr" => Region {
            code: "FR",
            language: "fr",
        },
        "de" => Region {
            code: "DE",
            language: "de",
        },
        "it" => Region {
            code: "IT",
            language: "it",
        },
        "pt-BR" => Region {
            code: "BR",
            language: "pt",
        },
        "pl" => Region {
            code: "PL",
            language: "pl",
        },
        "tr" => Region {
            code: "TR",
            language: "tr",
        },
        "id" => Region {
            code: "ID",
            language: "id",
        },
        "vi" => Region {
            code: "VN",
            language: "vi",
        },
        "th" => Region {
            code: "TH",
            language: "th",
        },
        "ru" => Region {
            code: "RU",
            language: "ru",
        },
        "uk" => Region {
            code: "UA",
            language: "uk",
        },
        _ => Region {
            code: "US",
            language: "en",
        },
    }
}

/// Search the catalogue for songs.
///
/// An empty query is answered with the songs that are popular in the region,
/// which is what the charts are: the same endpoint, with no query.
pub async fn search_songs(
    client: &reqwest::Client,
    query: &str,
    region: Region,
    limit: usize,
) -> Result<Vec<Song>> {
    let mut body = serde_json::json!({
        "context": {
            "client": {
                "clientName": CLIENT_NAME,
                "clientVersion": CLIENT_VERSION,
                "hl": region.language,
                "gl": region.code,
            }
        },
    });

    let query = query.trim();
    if query.is_empty() {
        // No query means the songs the region is listening to.
        body["query"] = serde_json::Value::String(
            trending_query(region.language).to_owned(),
        );
    } else {
        body["query"] = serde_json::Value::String(query.to_owned());
    }
    // The filter is what makes the answer songs. Without it the answer is
    // albums and artists, which carry no video id.
    body["params"] = serde_json::Value::String(SONGS_FILTER.to_owned());

    let response = client
        .post(SEARCH_ENDPOINT)
        .header("Content-Type", "application/json")
        .header("X-YouTube-Client-Name", CLIENT_NUMBER)
        .header("X-YouTube-Client-Version", CLIENT_VERSION)
        .header("Origin", "https://music.youtube.com")
        .json(&body)
        .send()
        .await
        .context("YouTube Music could not be reached")?;

    let text = response
        .text()
        .await
        .context("YouTube Music sent something unreadable")?;

    let parsed: Value =
        serde_json::from_str(&text).context("YouTube Music sent something unreadable")?;

    // The collections are dropped, and the limit is applied after: the search
    // answers with twenty items of which some are collections, so taking the
    // first twenty would leave the list short.
    Ok(parse_songs(&parsed)
        .into_iter()
        .filter(looks_like_a_song)
        .take(limit)
        .collect())
}

/// The songs in a search answer.
///
/// The answer is a tree of renderer structures, and the songs are the items
/// under a list. The tree is walked rather than read at a fixed path,
/// because the path differs between the kinds of list the endpoint can
/// return.
fn parse_songs(value: &Value) -> Vec<Song> {
    let mut songs = Vec::new();
    collect_songs(value, &mut songs);
    songs
}

/// Collect the songs under a value.
fn collect_songs(value: &Value, songs: &mut Vec<Song>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if key == "musicResponsiveListItemRenderer" {
                    if let Some(song) = parse_song(child) {
                        songs.push(song);
                    }
                    continue;
                }
                collect_songs(child, songs);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_songs(item, songs);
            }
        }
        _ => {}
    }
}

/// One entry of a search answer.
fn parse_song(value: &Value) -> Option<Song> {
    let video_id = find_video_id(value)?;
    let columns = value.get("flexColumns")?.as_array()?;

    // The first column holds the title, the second the artist, the album and
    // the length.
    let mut title = None;
    let mut artist = None;
    let mut album = None;
    let mut duration = None;

    for (index, column) in columns.iter().enumerate() {
        let renderer = column.get("musicResponsiveListItemFlexColumnRenderer")?;
        let runs = renderer
            .get("text")?
            .get("runs")?
            .as_array()?;

        if index == 0 {
            title = runs.first().and_then(|run| run.get("text")).and_then(|t| t.as_str());
        } else {
            // The second column is the artist, the album and the length, with
            // a separator between them. The artist is the one that links to
            // an artist page, and the album the one that links to an album
            // page, so the links are what tell them apart.
            for run in runs {
                let text = run.get("text").and_then(|t| t.as_str()).unwrap_or_default();
                if text == " • " || text.is_empty() {
                    continue;
                }
                let endpoint = run.get("navigationEndpoint");
                let page = endpoint
                    .and_then(|e| e.get("browseEndpoint"))
                    .and_then(|b| b.get("browseEndpointContextSupportedConfigs"))
                    .and_then(|c| c.get("browseEndpointContextMusicConfig"))
                    .and_then(|c| c.get("pageType"))
                    .and_then(|p| p.as_str())
                    .unwrap_or_default();

                if page == "MUSIC_PAGE_TYPE_ARTIST" {
                    artist = Some(text.to_owned());
                } else if page == "MUSIC_PAGE_TYPE_ALBUM" {
                    album = Some(text.to_owned());
                } else if duration.is_none() && text.contains(':') {
                    duration = parse_duration(text);
                } else if artist.is_none() {
                    artist = Some(text.to_owned());
                }
            }
        }
    }

    Some(Song {
        url: format!("https://music.youtube.com/watch?v={video_id}"),
        video_id,
        title: title?.to_owned(),
        artist: artist.unwrap_or_default(),
        album,
        duration,
    })
}

/// The video id of an entry.
///
/// The title links to the song, and so does the play button, and both carry
/// it. The first one found is taken.
fn find_video_id(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if key == "watchEndpoint" {
                    if let Some(id) = child.get("videoId").and_then(|id| id.as_str()) {
                        return Some(id.to_owned());
                    }
                }
                if let Some(found) = find_video_id(child) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(items) => items.iter().find_map(find_video_id),
        _ => None,
    }
}

/// A length as `M:SS` or `H:MM:SS`, in seconds.
fn parse_duration(text: &str) -> Option<f64> {
    let mut total = 0.0;
    for part in text.split(':') {
        let value: f64 = part.trim().parse().ok()?;
        total = total * 60.0 + value;
    }
    Some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_locale_gives_a_region() {
        assert_eq!(region_for_locale("zh-Hans").language, "zh-Hans");
        assert_eq!(region_for_locale("ja").code, "JP");
        assert_eq!(region_for_locale("de").code, "DE");
        // A language the project does not ship falls back to English.
        assert_eq!(region_for_locale("xx").code, "US");
    }

    #[test]
    fn a_song_is_read_from_a_search_answer() {
        // The shape the endpoint answers with, reduced to the parts a song is
        // made of: the title links to the video, the second column holds the
        // artist, the album and the length, and the links are what tell the
        // artist and the album apart.
        let answer = serde_json::json!({
            "contents": {
                "musicShelfRenderer": {
                    "contents": [
                        {
                            "musicResponsiveListItemRenderer": {
                                "flexColumns": [
                                    {
                                        "musicResponsiveListItemFlexColumnRenderer": {
                                            "text": {
                                                "runs": [
                                                    {
                                                        "text": "太陽之子",
                                                        "navigationEndpoint": {
                                                            "watchEndpoint": {
                                                                "videoId": "B2QoeaMJfps"
                                                            }
                                                        }
                                                    }
                                                ]
                                            }
                                        }
                                    },
                                    {
                                        "musicResponsiveListItemFlexColumnRenderer": {
                                            "text": {
                                                "runs": [
                                                    {
                                                        "text": "周杰倫",
                                                        "navigationEndpoint": {
                                                            "browseEndpoint": {
                                                                "browseEndpointContextSupportedConfigs": {
                                                                    "browseEndpointContextMusicConfig": {
                                                                        "pageType": "MUSIC_PAGE_TYPE_ARTIST"
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    },
                                                    { "text": " • " },
                                                    {
                                                        "text": "太陽之子",
                                                        "navigationEndpoint": {
                                                            "browseEndpoint": {
                                                                "browseEndpointContextSupportedConfigs": {
                                                                    "browseEndpointContextMusicConfig": {
                                                                        "pageType": "MUSIC_PAGE_TYPE_ALBUM"
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    },
                                                    { "text": " • " },
                                                    { "text": "4:58" }
                                                ]
                                            }
                                        }
                                    }
                                ]
                            }
                        }
                    ]
                }
            }
        });

        let songs = parse_songs(&answer);
        assert_eq!(songs.len(), 1);
        assert_eq!(songs[0].video_id, "B2QoeaMJfps");
        assert_eq!(songs[0].title, "太陽之子");
        assert_eq!(songs[0].artist, "周杰倫");
        assert_eq!(songs[0].album.as_deref(), Some("太陽之子"));
        assert_eq!(songs[0].duration, Some(298.0));
        assert_eq!(
            songs[0].url,
            "https://music.youtube.com/watch?v=B2QoeaMJfps"
        );
    }

    #[test]
    fn an_album_has_no_video_id_and_is_skipped() {
        // Without the songs filter the answer carries albums, which have a
        // browse id rather than a video id and so cannot be downloaded.
        let answer = serde_json::json!({
            "contents": {
                "musicShelfRenderer": {
                    "contents": [{
                        "musicResponsiveListItemRenderer": {
                            "flexColumns": [
                                {"musicResponsiveListItemFlexColumnRenderer": {"text": {"runs": [{
                                    "text": "太陽之子",
                                    "navigationEndpoint": {"browseEndpoint": {"browseId": "MPREb"}}
                                }]}}}
                            ]
                        }
                    }]
                }
            }
        });

        assert!(parse_songs(&answer).is_empty());
    }

    #[test]
    fn a_duration_is_read() {
        assert_eq!(parse_duration("4:58"), Some(298.0));
        assert_eq!(parse_duration("1:02:03"), Some(3723.0));
        assert_eq!(parse_duration("nope"), None);
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;

    /// The search, against the real endpoint.
    ///
    /// Ignored by default because it needs the network. Run it with:
    ///
    /// ```text
    /// cargo test live_tests -- --ignored --nocapture
    /// ```
    /// The charts: the same endpoint with no query.
    #[tokio::test]
    #[ignore]
    async fn the_charts_answer() {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .unwrap();

        let region = region_for_locale("zh-Hant");
        let songs = search_songs(&client, "", region, 20)
            .await
            .expect("the charts should answer");

        println!("the charts have {} songs", songs.len());
        for song in songs.iter().take(5) {
            println!("  {} | {} | {}", song.video_id, song.title, song.artist);
        }
    }

    #[tokio::test]
    #[ignore]
    async fn the_catalogue_answers() {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .unwrap();

        let region = region_for_locale("zh-Hant");
        let songs = search_songs(&client, "周杰倫", region, 20)
            .await
            .expect("the search should succeed");

        println!("found {} songs", songs.len());
        for song in songs.iter().take(5) {
            println!(
                "  {} | {} | {} | {:?} | {:?}",
                song.video_id, song.title, song.artist, song.album, song.duration
            );
        }
        assert!(!songs.is_empty());
    }
}

#[cfg(test)]
mod filter_tests {
    use super::*;

    fn song(title: &str, artist: &str) -> Song {
        Song {
            video_id: "x".to_owned(),
            title: title.to_owned(),
            artist: artist.to_owned(),
            album: None,
            duration: None,
            url: String::new(),
        }
    }

    #[test]
    fn a_song_is_kept() {
        assert!(looks_like_a_song(&song("有一种爱叫做放手", "阿木")));
        assert!(looks_like_a_song(&song(
            "如果可以 (電影《月老》主題曲)",
            "韦礼安"
        )));
    }

    #[test]
    fn a_collection_is_dropped() {
        // A collection of twenty songs, a DJ mix, a chart.
        assert!(!looks_like_a_song(&song(
            "2026 最好听的华语情歌 | 20首催泪伤感情歌 | Chinese Love Songs",
            "THY Moon Music"
        )));
        assert!(!looks_like_a_song(&song(
            "2026抖音热歌榜TOP10 全中文劲爆DJ舞曲｜超嗨车载慢摇串烧",
            "Luy Sokmean"
        )));
        assert!(!looks_like_a_song(&song("Summer Mix", "Some Channel")));
    }

    #[test]
    fn a_result_without_an_artist_is_dropped() {
        assert!(!looks_like_a_song(&song("Something", "")));
    }
}
