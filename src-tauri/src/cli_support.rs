//! Helpers shared by the GUI and the CLI.
//!
//! These lived in `main.rs` / `gui.rs`, which the `cli` feature excludes. Anything
//! both halves need belongs here.

use crate::lrclib;

/// Message shown when LRCLIB has no record for the requested track.
pub const LRCLIB_TRACK_NOT_FOUND: &str = "This track does not exist in LRCLIB database";

/// Lyrics resolved from an LRCLIB record, with the source of the text recorded.
pub struct ResolvedLyricsPayload {
    pub plain_lyrics: String,
    pub synced_lyrics: String,
    pub is_instrumental: bool,
    pub provided_lyricsfile: Option<String>,
}

pub fn resolve_lrclib_lyrics_payload(
    lrclib_response: crate::lrclib::get::RawResponse,
) -> Result<ResolvedLyricsPayload, String> {
    let provided_lyricsfile = lrclib_response
        .lyricsfile
        .clone()
        .filter(|content| !content.trim().is_empty());

    if let Some(lyricsfile_content) = provided_lyricsfile {
        // The `lyricsfile` field is not always the YAML document this app writes:
        // LRCLIB also serves plain LRC there. Fall back to parsing it as LRC
        // rather than rejecting an otherwise valid record.
        let parsed = match crate::lyricsfile::parse_lyricsfile(&lyricsfile_content) {
            Ok(parsed) => parsed,
            Err(_) => {
                let lrc = crate::parser::lrc::parse_lrc(&lyricsfile_content);
                if lrc.timed_lines.is_empty() {
                    return Err(LRCLIB_TRACK_NOT_FOUND.to_owned());
                }
                let mut synced = String::new();
                for line in &lrc.timed_lines {
                    synced.push_str(&crate::parser::lrc::format_timestamp(line.timestamp_ms));
                    synced.push_str(&line.text);
                    synced.push('\n');
                }
                let plain = crate::utils::strip_timestamp(&synced);
                return Ok(ResolvedLyricsPayload {
                    plain_lyrics: plain,
                    synced_lyrics: synced,
                    is_instrumental: false,
                    provided_lyricsfile: Some(lyricsfile_content),
                });
            }
        };
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

    match crate::lrclib::get::Response::from_raw_response(lrclib_response) {
        crate::lrclib::get::Response::SyncedLyrics(synced_lyrics, plain_lyrics) => {
            Ok(ResolvedLyricsPayload {
                plain_lyrics,
                synced_lyrics,
                is_instrumental: false,
                provided_lyricsfile: None,
            })
        }
        crate::lrclib::get::Response::UnsyncedLyrics(plain_lyrics) => Ok(ResolvedLyricsPayload {
            plain_lyrics,
            synced_lyrics: String::new(),
            is_instrumental: false,
            provided_lyricsfile: None,
        }),
        crate::lrclib::get::Response::IsInstrumental => Ok(ResolvedLyricsPayload {
            plain_lyrics: String::new(),
            synced_lyrics: crate::lyricsfile::INSTRUMENTAL_LRC.to_owned(),
            is_instrumental: true,
            provided_lyricsfile: None,
        }),
        crate::lrclib::get::Response::None => Err(LRCLIB_TRACK_NOT_FOUND.to_owned()),
    }
}

/// Compare dotted versions without pulling in a semver crate.
///
/// Only needs to answer "is the release newer than what is running", so a
/// numeric comparison of the first three components is enough. A pre-release
/// suffix is ignored: this project does not publish them.
pub fn is_newer_version(latest: &str, current: &str) -> bool {
    let parse = |value: &str| -> Vec<u64> {
        value
            .split('.')
            .map(|part| {
                part.chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
            })
            .map(|part| part.parse().unwrap_or(0))
            .collect()
    };

    let latest = parse(latest);
    let current = parse(current);
    for index in 0..latest.len().max(current.len()) {
        let a = latest.get(index).copied().unwrap_or(0);
        let b = current.get(index).copied().unwrap_or(0);
        if a != b {
            return a > b;
        }
    }
    false
}
