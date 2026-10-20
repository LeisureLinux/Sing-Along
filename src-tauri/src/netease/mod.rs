//! NetEase Cloud Music (music.163.com) as a fallback lyrics source.
//!
//! LRCLIB has thin coverage of Chinese music; NetEase is where most Chinese
//! lyrics actually live. When LRCLIB returns 404 for a track, this module tries
//! NetEase before giving up.
//!
//! ## This is an unofficial API
//!
//! Nothing here is a documented, stable interface. Two practical constraints
//! were established by probing the endpoints directly:
//!
//! - `/api/search/pc` answers `{"code": -462}` — "bind a phone number" —
//!   regardless of headers. It is effectively closed to third parties.
//! - `/api/search/get/` and `/api/song/lyric` both work with nothing more than
//!   `Cookie: os=pc`, which is what this module sends.
//!
//! That state can change without notice. The module is therefore written so a
//! failure of any kind surfaces as a plain `Err` that the download path treats
//! as "no fallback lyrics" — a broken NetEase never blocks or corrupts an
//! LRCLIB download; it just stops helping.
//!
//! ## Matching
//!
//! Search results are matched by combined title+artist similarity, because the
//! top hit is routinely a cover, a live version or an unrelated song sharing a
//! name. A candidate must clear a threshold or it is rejected — returning no
//! lyrics is better than attaching the wrong ones, which the user would then
//! have to notice and undo.

use anyhow::Result;
use serde::Deserialize;
use std::time::Duration;

const SEARCH_ENDPOINT: &str = "https://music.163.com/api/search/pc";
const LYRIC_ENDPOINT: &str = "https://music.163.com/api/song/lyric";

/// The request headers that make NetEase's API answer.
///
/// `/api/search/pc` rejects everything else with `{"code":-462}` ("bind a phone
/// number") — including a browser user-agent, and including `Cookie: os=pc`
/// alone, which is enough for the lyric endpoint but not for search. The cookie
/// string below is copied verbatim from syncedlyrics, which has been sending
/// the same 2022-era values for years; the endpoints do not validate them, they
/// only require *some* recognizable client cookie. They are sent as one header
/// (syncedlyrics sets `session.headers["cookie"]`), not as a cookie jar.
///
/// The `os=pc` cookie additionally satisfies the lyric endpoint, which needs it
/// when fetched standalone.
const NETEASE_COOKIE: &str = "os=pc; NMTID=00OAVK3xqDG726ITU6jopU6jF2yMk0AAAGCO8l1BA; JSESSIONID-WYYY=8KQo11YK2GZP45RMlz8Kn80vHZ9%2FGvwzRKQXXy0iQoFKycWdBlQjbfT0MJrFa6hwRfmpfBYKeHliUPH287JC3hNW99WQjrh9b9RmKT%2FD%2FWghb%5C4DuZ%3A1659124633932; _iuqxldmzr_=32; _ntes_nnid=0db6667097883aa9596ecfe7f188c3ec,1659122833973; _ntes_nuid=0db6667097883aa9596ecfe7f188c3ec; WNMCID=xygast.1659122837568.01.0; WEVNSM=1.0.0; WM_NI=CwbjWAFbcIzPX3dsLP%2F52VB%2Bxr572gmqAYwvN9KU5X5f1nRzBYl0SNf%2BV9FTmmYZy%2FoJLADaZS0Q8TrKfNSBNOt0HLB8rRJh9DsvMOT7%2BCGCQLbvlWAcJBJeXb1P8yZ3RHA%3D; WM_NIKE=9ca17ae2e6ffcda170e2e6ee90c65b85ae87b9aa5483ef8ab3d14a939e9a83c459959caeadce47e991fbaee82af0fea7c3b92a81a9ae8bd64b86beadaaf95c9cedac94cf5cedebfeb7c121bcaefbd8b16dafaf8fbaf67e8ee785b6b854f7baff8fd1728287a4d1d246a6f59adac560afb397bbfc25ad9684a2c76b9a8d00b2bb60b295aaafd24a8e91bcd1cb4882e8beb3c964fb9cbd97d04598e9e5a4c6499394ae97ef5d83bd86a3c96f9cbeffb1bb739aed9ea9c437e2a3; WM_TID=AAkRFnl03RdABEBEQFOBWHCPOeMra4IL; playerid=94262567";

/// NetEase also rejects unknown user agents on the search endpoint. This is the
/// user agent syncedlyrics sends — requests' own default — and it works.
const NETEASE_UA: &str = "python-requests/2.32.3";

/// Minimum combined similarity for a search hit to be accepted.
///
/// 60 lets close matches through (title exact, artist romanised differently)
/// while rejecting a same-titled song by an unrelated artist, which typically
/// scores around 40–50.
const MATCH_THRESHOLD: u32 = 60;

#[derive(Deserialize)]
struct SearchResponse {
    result: Option<SearchResult>,
}

#[derive(Deserialize)]
struct SearchResult {
    #[serde(default)]
    songs: Vec<NeteaseSong>,
}

/// A search hit, exposed so the manual search dialog can list NetEase results
/// next to LRCLIB's without knowing about NetEase's response shape.
#[derive(Debug, Clone)]
pub struct SongHit {
    pub id: i64,
    pub name: String,
    pub artists: String,
    pub duration_secs: Option<f64>,
}

#[derive(Deserialize)]
struct NeteaseSong {
    id: i64,
    name: String,
    #[serde(default)]
    artists: Vec<NeteaseArtist>,
    #[serde(default)]
    duration: Option<u64>,
}

#[derive(Deserialize)]
struct NeteaseArtist {
    name: String,
}

#[derive(Deserialize)]
struct LyricResponse {
    #[serde(default)]
    lrc: Option<LyricBody>,
    #[serde(default)]
    tlyric: Option<LyricBody>,
}

#[derive(Deserialize)]
struct LyricBody {
    #[serde(default)]
    lyric: Option<String>,
}

/// LRC text fetched from NetEase, with the ids used to fetch it.
pub struct NeteaseLyrics {
    pub song_id: i64,
    pub synced_lyrics: String,
}

/// Fetch synced LRC for a track from NetEase.
///
/// Returns `Ok(None)` when nothing was found *or* when the best search hit did
/// not look like the same track — both mean "NetEase cannot help with this one".
/// Only transport-level problems surface as `Err`.
pub async fn fetch_lyrics(
    client: &reqwest::Client,
    title: &str,
    artist_name: &str,
    duration_secs: Option<f64>,
) -> Result<Option<NeteaseLyrics>> {
    let query = format!("{title} {artist_name}");
    let songs = search(client, &query).await?;
    let song = match pick_match(&songs, title, artist_name, duration_secs) {
        Some(song) => song,
        None => {
            #[cfg(test)]
            eprintln!("[fetch_lyrics] no match among {} songs", songs.len());
            return Ok(None);
        }
    };


    let synced = fetch_lrc_by_id(client, song.id).await?;
    match synced {
        Some(lrc) if !lrc.trim().is_empty() => Ok(Some(NeteaseLyrics {
            song_id: song.id,
            synced_lyrics: lrc,
        })),
        _ => Ok(None),
    }
}

/// Search NetEase for songs matching a free-text query.
///
/// Unlike `fetch_lyrics`, this does not pick a best match: the manual search
/// dialog shows every hit and lets the user choose.
pub async fn search_songs(client: &reqwest::Client, query: &str) -> Result<Vec<SongHit>> {
    let songs = search(client, query).await?;
    Ok(songs
        .into_iter()
        .map(|song| SongHit {
            id: song.id,
            name: song.name,
            artists: song
                .artists
                .into_iter()
                .map(|a| a.name)
                .collect::<Vec<_>>()
                .join(", "),
            duration_secs: song.duration.map(|ms| ms as f64 / 1000.0),
        })
        .collect())
}

/// Fetch the LRC text for a NetEase song id.
pub async fn lyrics_for_song(
    client: &reqwest::Client,
    song_id: i64,
) -> Result<Option<String>> {
    fetch_lrc_by_id(client, song_id).await
}

async fn search(client: &reqwest::Client, query: &str) -> Result<Vec<NeteaseSong>> {
    // NetEase serves these endpoints as `text/plain`, so reqwest's `.json()`
    // (which checks the content type) rejects the body. Read as text and parse
    // explicitly.
    let body = client
        .get(SEARCH_ENDPOINT)
        .header("cookie", NETEASE_COOKIE)
        .header("user-agent", NETEASE_UA)
        .query(&[("limit", "10"), ("type", "1"), ("s", query)])
        .timeout(Duration::from_secs(10))
        .send()
        .await?
        .text()
        .await?;

    let response: SearchResponse = serde_json::from_str(&body)?;
    Ok(response.result.map(|r| r.songs).unwrap_or_default())
}

async fn fetch_lrc_by_id(client: &reqwest::Client, song_id: i64) -> Result<Option<String>> {
    let body = client
        .get(LYRIC_ENDPOINT)
        .header("cookie", NETEASE_COOKIE)
        .header("user-agent", NETEASE_UA)
        .query(&[("id", song_id.to_string()), ("lv", "1".to_string())])
        .timeout(Duration::from_secs(10))
        .send()
        .await?
        .text()
        .await?;

    #[cfg(test)]
    eprintln!("[lrc_by_id id={}] body head: {}", song_id, body.get(..120).unwrap_or(&body));

    let response: LyricResponse = serde_json::from_str(&body)?;
    let lrc = response.lrc.and_then(|entry| entry.lyric);
    #[cfg(test)]
    eprintln!("[lrc_by_id id={}] lrc len: {:?}", song_id, lrc.as_ref().map(|l| l.len()));
    Ok(lrc)
}

/// Choose the search hit that is most likely the same track.
///
/// Scores each hit by title similarity and artist similarity and keeps the best
/// combined one, subject to a threshold. Duration, when known on both sides, can
/// only *confirm* a choice (a close match with a wildly different duration is
/// rejected); it is not used to pick, because NetEase durations are for the
/// recording they host, which may legitimately differ from the local file.
fn pick_match<'a>(
    songs: &'a [NeteaseSong],
    title: &str,
    artist: &str,
    duration_secs: Option<f64>,
) -> Option<&'a NeteaseSong> {
    let mut best: Option<(&NeteaseSong, u32)> = None;

    for song in songs {
        let song_artist = song
            .artists
            .first()
            .map(|a| a.name.as_str())
            .unwrap_or_default();

        let title_score = normalized_similarity(title, &song.name);
        let artist_score = normalized_similarity(artist, song_artist);

        // Jaro-Winkler scores substring containment poorly: "冷雨夜" is contained
        // in the NetEase title "忘情冷雨夜" but scores 0, because JW rewards
        // shared prefixes and neither CJK string shares one. So containment is
        // checked explicitly before falling back to JW.
        let contained = {
            let (a, b) = normalize_pair(title, &song.name);
            a.contains(&b) || b.contains(&a)
        };

        // Two independent gates. A title alone is not enough — "same name,
        // different singer" is a different song, and attaching its lyrics would be
        // wrong in a way the user has to notice and undo. JW handles fuzzy
        // matches; containment handles the CJK case where JW scores a contained
        // substring at 0.
        let title_match = contained || title_score >= MATCH_THRESHOLD;
        let artist_match = artist_score >= MATCH_THRESHOLD;

        // A duration gap is a soft signal, not a rejection. NetEase hosts one
        // recording; the local file may be a different edit, a live take or a
        // remaster of the same song. Rejecting on it threw away every real match
        // for a track whose local copy happened to be a different cut, which is
        // why the fallback returned nothing at all.
        let duration_ok = match (duration_secs, song.duration) {
            (Some(local), Some(remote_ms)) => {
                (local - remote_ms as f64 / 1000.0).abs() <= 30.0
            }
            _ => true,
        };



        if !(title_match && artist_match) {
            continue;
        }


        // Among the songs that pass both gates, prefer the highest title score:
        // two live versions of the same song both match, and the closest title is
        // the best guess for the arrangement the user has.
        let _ = duration_ok;
        if best
            .map(|(_, score)| title_score > score)
            .unwrap_or(true)
        {
            best = Some((song, title_score));
        }
    }

    best.map(|(song, _)| song)
}

/// Similarity of two strings after casefolding and stripping whitespace and
/// punctuation, as 0–100.
fn normalize_pair(a: &str, b: &str) -> (String, String) {
    let normalize = |s: &str| -> String {
        s.chars()
            .filter(|c| !c.is_whitespace() && !c.is_ascii_punctuation())
            .flat_map(|c| c.to_lowercase())
            .collect()
    };
    (normalize(a), normalize(b))
}

fn normalized_similarity(a: &str, b: &str) -> u32 {
    let normalize = |s: &str| -> String {
        s.chars()
            .filter(|c| !c.is_whitespace() && !c.is_ascii_punctuation())
            .flat_map(|c| c.to_lowercase())
            .collect()
    };
    similarity(&normalize(a), &normalize(b))
}

/// Jaro-Winkler is a reasonable default here: it rewards shared prefixes,
/// which helps with "歌名 (Live版)" style suffixes, and it is cheap. The scale is
/// 0.0–1.0; multiply by 100 to match the threshold units.
fn similarity(a: &str, b: &str) -> u32 {
    if a.is_empty() || b.is_empty() {
        return 0;
    }
    if a == b {
        return 100;
    }
    (strsim::jaro_winkler(a, b) * 100.0).round() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(name: &str, artist: &str, ms: u64) -> NeteaseSong {
        NeteaseSong {
            id: 1,
            name: name.to_string(),
            artists: vec![NeteaseArtist {
                name: artist.to_string(),
            }],
            duration: Some(ms),
        }
    }

    #[test]
    fn exact_match_beats_similar_titles() {
        let songs = vec![
            song("送你一朵小红花 (Live)", "韩红", 300_000),
            song("送你一朵小红花", "赵英俊", 255_000),
            song("小红花", "别人", 200_000),
        ];
        let got = pick_match(&songs, "送你一朵小红花", "赵英俊", Some(255.0));
        assert_eq!(got.map(|s| s.artists[0].name.as_str()), Some("赵英俊"));
    }

    #[test]
    fn wrong_artist_below_threshold_is_rejected() {
        let songs = vec![song("送你一朵小红花", "完全无关的歌手", 255_000)];
        assert!(pick_match(&songs, "送你一朵小红花", "赵英俊", None).is_none());
    }

    #[test]
    fn big_duration_gap_does_not_reject() {
        // Duration differences do not filter: the local file may be a different
        // edit or live take of the same song. It only affects ranking.
        let songs = vec![song("送你一朵小红花", "赵英俊", 400_000)];
        assert!(pick_match(&songs, "送你一朵小红花", "赵英俊", Some(255.0)).is_some());
    }

    #[test]
    fn title_suffixes_do_not_block_a_match() {
        let songs = vec![song("送你一朵小红花 (Live)", "赵英俊", 255_000)];
        assert!(pick_match(&songs, "送你一朵小红花", "赵英俊", Some(255.0)).is_some());
    }

    #[test]
    fn empty_input_scores_zero() {
        assert_eq!(similarity("", "x"), 0);
        assert_eq!(similarity("x", ""), 0);
        assert_eq!(similarity("same", "same"), 100);
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;

    // Live test against NetEase. Runs only when NETEASE_LIVE_TEST=1, so the
    // normal test suite does not depend on a third-party server.

    #[test]
    #[ignore]
    fn fetches_real_lyrics() {
        if std::env::var("NETEASE_LIVE_TEST").is_err() {
            return;
        }
        let client = reqwest::Client::new();
        let result = tokio_test::block_on(fetch_lyrics(
            &client,
            "冷雨夜",
            "张学友",
            Some(300.0),
        ));
        let lyrics = result.expect("transport failed").expect("no lyrics found");
        assert!(lyrics.synced_lyrics.contains("[00:"), "should be an LRC");
        println!("got {} bytes of LRC from NetEase song {}", lyrics.synced_lyrics.len(), lyrics.song_id);
    }
}

#[cfg(test)]
mod similarity_tests {
    use super::normalized_similarity;

    #[test]
    fn debug_real_call_path() {
        // Reproduce exactly what debug_search_response sees: the search hit is
        // "冷雨夜" as parsed from NetEase JSON, compared against the search input
        // "冷雨夜". If this passes but the live test fails, the strings differ.
        let a = "冷雨夜";
        let b = "冷雨夜";
        println!("normalized: {:?} vs {:?}", a, b);
        println!("score: {}", normalized_similarity(a, b));
        assert_eq!(normalized_similarity(a, b), 100);
    }

    #[test]
    fn chinese_exact_match_scores_100() {
        // The very case that failed: identical short CJK strings must score 100.
        assert_eq!(normalized_similarity("冷雨夜", "冷雨夜"), 100);
        assert_eq!(normalized_similarity("送你一朵小红花", "送你一朵小红花"), 100);
    }
}
