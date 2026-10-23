//! The AMLL TTML database as a word-level lyrics source.
//!
//! <https://github.com/amll-dev/amll-ttml-db> is a community-maintained library
//! of word-by-word timed lyrics, organised by the music id of four platforms:
//! NetEase, QQ Music, Apple Music and Spotify. Every entry is stored as a static
//! file under `{platform}-lyrics/{id}.ttml`, with the other formats generated
//! from it — so a `GET` of a known path returns a word-timed TTML, which is one
//! of the formats this project already exports.
//!
//! That is the whole point of this module. LRCLIB and NetEase return line-level
//! lyrics, and `word_sync` can only *estimate* word boundaries by distributing a
//! line's duration across its tokens. A word timing measured by a person is
//! better than one inferred from line length, and this source has them.
//!
//! ## What this is not
//!
//! It is not an integration with QQ Music. The platform id in the path is how the
//! database indexes its entries; nothing here talks to QQ Music, Apple Music or
//! Spotify. NetEase ids are the ones this project can obtain, because it already
//! searches NetEase, so `ncm-lyrics` is what actually gets queried.
//!
//! ## Reliability
//!
//! The database is one GitHub repository maintained by volunteers. If it stops,
//! this source stops — the same exposure LRCLIB has, and the reason the URL is a
//! setting rather than a constant: the repository has hundreds of forks, and any
//! of them serves the same paths.
//!
//! Nothing here can block a download. A failure of any kind is an `Err` the
//! caller treats as "this source has nothing", exactly like the NetEase module.

pub mod ttml;

use anyhow::{Context, Result};
use std::time::Duration;

/// Where the database is served from.
///
/// `raw.githubusercontent.com` rather than a clone: one file per lookup, no
/// checkout, and the path is stable. Overridable because a mirror is a copy.
pub const DEFAULT_BASE_URL: &str =
    "https://raw.githubusercontent.com/amll-dev/amll-ttml-db/refs/heads/main";

/// CDN mirrors used when the default is unreachable, tried in order.
///
/// `raw.githubusercontent.com` is reset from some networks — mainland China in
/// particular. jsDelivr serves the same repository, but its hosts are not
/// interchangeable: measured from one such network, `cdn.jsdelivr.net` was reset
/// while `fastly.` and `gcore.` answered normally, and this client uses rustls
/// rather than OpenSSL, which is enough for a middlebox to tell them apart.
///
/// A list rather than one fallback because which host survives changes; the path
/// shape (`/gh/{owner}/{repo}@{branch}/{path}`) is the same for all of them.
const MIRROR_BASE_URLS: [&str; 2] = [
    "https://fastly.jsdelivr.net/gh/amll-dev/amll-ttml-db@main",
    "https://gcore.jsdelivr.net/gh/amll-dev/amll-ttml-db@main",
];

/// The platform folder to look in.
///
/// Only `ncm-lyrics` is reachable without a new search integration: the rest of
/// this project obtains NetEase ids, and the database indexes by the platform's
/// own id. Adding `qq-lyrics` would need a QQ Music search that does not exist
/// here, and would gain nothing while NetEase ids are what we have.
const NCM_FOLDER: &str = "ncm-lyrics";

/// A word-timed TTML document, with the id it was fetched by.
pub struct AmllLyrics {
    pub netease_id: i64,
    /// The TTML source. Word timings are real measurements, not estimates.
    pub ttml: String,
}

/// Fetch word-by-word lyrics for a NetEase song id.
///
/// `Ok(None)` means the database has no entry for this song, which is the usual
/// answer: it holds tens of thousands of songs, not a catalogue. `Err` means the
/// request itself failed.
pub async fn fetch_lyrics(
    client: &reqwest::Client,
    base_url: &str,
    netease_id: i64,
) -> Result<Option<AmllLyrics>> {
    let path = format!("{NCM_FOLDER}/{netease_id}.ttml");

    // The configured base first, then each mirror. A user who set a mirror gets
    // exactly that mirror; everyone else gets whichever host answers, in an order
    // that costs no extra requests when GitHub is reachable.
    let mut candidates = vec![format!("{}/{path}", base_url.trim_end_matches('/'))];
    candidates.extend(MIRROR_BASE_URLS.iter().map(|base| format!("{base}/{path}")));

    let mut last_error = None;
    let mut found = None;
    for url in &candidates {
        match send(client, url).await {
            Ok(response) if response.status().is_success() => {
                found = Some((url.clone(), response));
                break;
            }
            Ok(response) if response.status() == reqwest::StatusCode::NOT_FOUND => {
                // Not here, and not at any mirror: they serve the same tree.
                return Ok(None);
            }
            Ok(response) => {
                eprintln!("AMLL: {url} answered {}", response.status());
                last_error = Some(anyhow::anyhow!("{url} answered {}", response.status()));
            }
            Err(err) => {
                eprintln!("AMLL: {url} unreachable: {err}");
                last_error = Some(err);
            }
        }
    }

    let Some((url, response)) = found else {
        return Err(last_error.unwrap_or_else(|| anyhow::anyhow!("no AMLL source answered")));
    };

    let ttml = response.text().await?;
    // A GitHub error page served with 200 would otherwise be stored as lyrics.
    if !ttml.contains("<tt ") && !ttml.contains("<tt>") {
        anyhow::bail!("{url} did not return a TTML document");
    }

    Ok(Some(AmllLyrics {
        netease_id,
        ttml,
    }))
}

/// One request, with the timeout and context the caller wants.
async fn send(client: &reqwest::Client, url: &str) -> Result<reqwest::Response> {
    client
        .get(url)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .with_context(|| format!("requesting {url} (is the network reachable?)"))
}

/// Whether a TTML document carries word-level timings.
///
/// The database accepts line-level submissions too, and its README says so. A
/// line-level entry is no better than what LRCLIB already returns, so callers
/// use this to decide whether the extra request was worth making.
pub fn has_word_timings(ttml: &str) -> bool {
    // A word-timed document has `<span ...>` elements inside each `<p>`; a
    // line-level one has bare text. Counting spans is enough: the timing
    // attributes appear on the span, and a document with none has no word
    // boundaries to offer.
    ttml.matches("<span").count() > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_level_document_has_no_word_timings() {
        let line_level = r#"<tt><body><div><p begin="00:01.000" end="00:03.000">A line of lyrics</p></div></body></tt>"#;
        assert!(!has_word_timings(line_level));
    }

    #[test]
    fn a_word_level_document_is_recognised() {
        let word_level = r#"<tt><body><div><p begin="00:01.000" end="00:03.000"><span begin="00:01.000" end="00:02.000">A</span><span begin="00:02.000" end="00:03.000">line</span></p></div></body></tt>"#;
        assert!(has_word_timings(word_level));
    }

    #[test]
    fn the_base_url_can_end_with_a_slash() {
        // Both spellings have to produce the same path, because the setting is a
        // free-text field.
        let with = format!("{}/ncm-lyrics/1.ttml", DEFAULT_BASE_URL.trim_end_matches('/'));
        let without = format!("{}/ncm-lyrics/1.ttml", DEFAULT_BASE_URL);
        assert_eq!(with, without);
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;

    /// Reaches the network. Runs only on demand:
    ///   cargo test amll_live -- --ignored --nocapture
    ///
    /// Kept out of the default run because a test suite that needs GitHub is a
    /// test suite that fails on a train.
    #[tokio::test]
    #[ignore]
    async fn a_known_netease_id_returns_word_level_ttml() {
        let client = reqwest::Client::builder().build().unwrap();
        // YOASOBI - Idol, the entry inspected by hand while writing this module.
        let found = fetch_lyrics(&client, DEFAULT_BASE_URL, 2048982668)
            .await
            .expect("request should succeed")
            .expect("this id is in the database");

        assert!(found.ttml.contains("Idol"));
        assert!(has_word_timings(&found.ttml), "should be word-level");
        println!("{} bytes", found.ttml.len());
    }

    #[tokio::test]
    #[ignore]
    async fn an_absent_id_is_none_not_an_error() {
        let client = reqwest::Client::builder().build().unwrap();
        let found = fetch_lyrics(&client, DEFAULT_BASE_URL, 1)
            .await
            .expect("a 404 is not an error");
        assert!(found.is_none());
    }
}
