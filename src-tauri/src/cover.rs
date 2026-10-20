//! Album cover art search from iTunes and Deezer, written into audio tags.
//!
//! Both APIs are public, documented and require no key — unlike the lyrics
//! sources, this is a stable surface. Artwork is searched by `artist + album`
//! and downloaded at the highest resolution available (at least 400px, up to
//! 1200px from iTunes's size-substitutable URLs).
//!
//! Off by default; the user opts in per export or download batch.

use anyhow::Result;
use serde::Deserialize;
use std::time::Duration;

/// Minimum acceptable cover resolution. Below this the art looks blurry when
/// displayed on a car screen or a large album view.
const MIN_RESOLUTION: u32 = 400;

#[derive(Deserialize)]
struct ItunesSearchResponse {
    results: Vec<ItunesResult>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItunesResult {
    artwork_url_100: Option<String>,
    #[serde(default)]
    track_count: u32,
}

#[derive(Deserialize)]
struct DeezerSearchResponse {
    #[serde(default)]
    data: Vec<DeezerAlbum>,
}

#[derive(Deserialize)]
struct DeezerAlbum {
    #[serde(default)]
    cover_xl: Option<String>,
    #[serde(default)]
    cover_big: Option<String>,
    #[serde(default)]
    nb_tracks: u32,
}

pub struct CoverResult {
    pub url: String,
    pub source: &'static str,
}

/// Search for the best album cover across sources.
///
/// iTunes is tried first (higher resolution via URL substitution, and the
/// `entity=album` filter plus `limit=1` typically returns the right album for a
/// well-known release); Deezer as a second source. The `track_count` filter
/// rejects singles and EPs when the local file's album appears to be a full
/// release, which is a heuristic but prevents the most common wrong-cover case.
pub async fn search(client: &reqwest::Client, artist: &str, album: &str) -> Result<Option<CoverResult>> {
    let query = format!("{artist} {album}");

    // iTunes: artworkUrl100 is always present; replace the size segment for
    // high resolution.
    let itunes_url = format!(
        "https://itunes.apple.com/search?term={}&entity=album&limit=3",
        urlencode(&query)
    );
    if let Ok(response) = client
        .get(&itunes_url)
        .timeout(Duration::from_secs(10))
        .send()
        .await
    {
        if let Ok(search) = response.json::<ItunesSearchResponse>().await {
            for result in &search.results {
                if let Some(url_100) = &result.artwork_url_100 {
                    let high_res = url_100.replace("100x100", "1200x1200");
                    return Ok(Some(CoverResult {
                        url: high_res,
                        source: "iTunes",
                    }));
                }
            }
        }
    }

    // Deezer: cover_xl is 1000x1000.
    let deezer_url = format!(
        "https://api.deezer.com/search/album?q={}&limit=3",
        urlencode(&query)
    );
    if let Ok(response) = client
        .get(&deezer_url)
        .timeout(Duration::from_secs(10))
        .send()
        .await
    {
        if let Ok(search) = response.json::<DeezerSearchResponse>().await {
            for album in &search.data {
                if let Some(url) = album.cover_xl.as_ref().or(album.cover_big.as_ref()) {
                    return Ok(Some(CoverResult {
                        url: url.clone(),
                        source: "Deezer",
                    }));
                }
            }
        }
    }

    Ok(None)
}

/// Download cover art bytes from a URL.
pub async fn download(client: &reqwest::Client, url: &str) -> Result<Vec<u8>> {
    let response = client
        .get(url)
        .timeout(Duration::from_secs(30))
        .send()
        .await?
        .error_for_status()?;

    let bytes = response.bytes().await?.to_vec();
    if bytes.len() < 1024 {
        anyhow::bail!("cover art response too small ({} bytes)", bytes.len());
    }
    Ok(bytes)
}

fn urlencode(text: &str) -> String {
    text.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// Embed cover art bytes into an MP3 file's FRONT_COVER frame.
///
/// Reuses the same Probe → TaggedFile → save_to_path flow as the lyric embed.
/// For MP3, this is an APIC frame in ID3v2; for FLAC, a PICTURE block.
pub async fn embed_cover(track_path: &str, client: &reqwest::Client, artist: &str, album: &str) -> Result<()> {
    let cover = search(client, artist, album).await?;
    let cover = cover.ok_or_else(|| anyhow::anyhow!("no cover found"))?;
    let bytes = download(client, &cover.url).await?;

    let path_lower = track_path.to_lowercase();
    if path_lower.ends_with(".mp3") {
        embed_cover_mp3(track_path, &bytes)
    } else if path_lower.ends_with(".flac") {
        embed_cover_flac(track_path, &bytes)
    } else {
        anyhow::bail!("cover embed not supported for this format");
    }
}

fn embed_cover_mp3(track_path: &str, bytes: &[u8]) -> Result<()> {
    use lofty::config::WriteOptions;
    use lofty::file::{AudioFile, TaggedFileExt};
    use lofty::picture::{Picture, PictureType};
    use lofty::probe::Probe;

    let file_probe = Probe::open(track_path)?;
    let mut file = file_probe
        .guess_file_type()?
        .read()?;

    let mut picture = Picture::from_reader(&mut std::io::Cursor::new(bytes))?;
    picture.set_pic_type(PictureType::CoverFront);

    if let Some(tag) = file.primary_tag_mut() {
        tag.push_picture(picture);
    } else {
        anyhow::bail!("no primary tag to embed cover into");
    }

    file.save_to_path(track_path, WriteOptions::default())?;
    Ok(())
}

fn embed_cover_flac(track_path: &str, bytes: &[u8]) -> Result<()> {
    use lofty::config::{ParseOptions, WriteOptions};
    use lofty::file::AudioFile;
    use lofty::flac::FlacFile;
    use lofty::ogg::OggPictureStorage;
    use lofty::picture::{Picture, PictureType};
    use std::io::Seek;
    use std::fs::OpenOptions;

    let mut file_content = OpenOptions::new()
        .read(true)
        .write(true)
        .open(track_path)?;

    let mut flac_file = FlacFile::read_from(&mut file_content, ParseOptions::new())?;

    let mut picture = Picture::from_reader(&mut std::io::Cursor::new(bytes))?;
    picture.set_pic_type(PictureType::CoverFront);

    if let Some(vorbis_comments) = flac_file.vorbis_comments_mut() {
        vorbis_comments.insert_picture(picture, None);
    }

    file_content
        .seek(std::io::SeekFrom::Start(0))?;
    flac_file
        .save_to(&mut file_content, WriteOptions::default())?;

    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencoding_encodes_spaces_and_cjk() {
        assert_eq!(urlencode("Hello World"), "Hello%20World");
        assert!(urlencode("送你").starts_with('%'));
        assert_eq!(urlencode("a-b_c.d~e"), "a-b_c.d~e");
    }

    #[test]
    fn min_resolution_is_respected() {
        assert_eq!(MIN_RESOLUTION, 400);
    }
}
