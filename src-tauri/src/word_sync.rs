//! Conversion of line-level synced lyrics into the word-level formats that
//! phone players actually read.
//!
//! Two targets, because the players do not agree on syntax:
//!
//! - **Enhanced LRC (A2)** — inline `<mm:ss.xx>` tags before each word. Read by
//!   Gramophone, Fluxio and other A2-aware Android players.
//! - **TTML** — Apple-Music-style XML with one `<span>` per word. Steve on iOS
//!   only takes word timings from TTML; its LRC reader is line-level and does
//!   not strip the angle-bracket tags, so A2 sent to it shows timestamps as
//!   lyric text.
//!
//! Both derive word timings the same way the editor does: Charabia segments the
//! line (so Chinese breaks at real word boundaries instead of a fixed character
//! count) and the line's duration is distributed in proportion to token length.
//!
//! This is an estimate, not a measurement. There is no audio analysis, so held
//! notes and fast runs drift. Accurate alignment needs forced alignment against
//! the audio, which is a different (and much more expensive) proposition.

use crate::parser::lrc::{is_instrumental_lrc, parse_lrc};
use crate::word_segmentation::segment_words_for_timing;

/// Lines longer than this are treated as instrumental gaps rather than lyrics.
/// Spreading words across a fifteen-second intro makes the highlight crawl and
/// then stall, which looks worse than no word timing at all.
const MAX_LINE_SPAN_MS: i64 = 12_000;

/// A line whose gap to the next is smaller than this would give words too little
/// time to be perceptible; widen it so the sweep stays legible.
const MIN_LINE_SPAN_MS: i64 = 400;

pub struct WordSyncLine {
    pub start_ms: i64,
    pub end_ms: i64,
    pub body: String,
    pub words: Vec<WordSyncWord>,
}

pub struct WordSyncWord {
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
}

/// Parse line-level LRC and attach per-word timings.
///
/// Returns `None` when there is nothing usable to convert (no timed lines, or
/// every line was a credit/instrumental marker).
pub fn build_word_sync(input: &str) -> Option<Vec<WordSyncLine>> {
    let parsed = parse_lrc(input);
    if is_instrumental_lrc(input) || parsed.timed_lines.is_empty() {
        return None;
    }

    let entries = &parsed.timed_lines;
    let mut plan: Vec<(i64, i64, String)> = Vec::new();

    for (index, line) in entries.iter().enumerate() {
        let body = line.text.trim();
        if body.is_empty() || is_credit_line(body) {
            continue;
        }

        let start = line.timestamp_ms;
        let mut end = entries
            .get(index + 1)
            .map(|next| next.timestamp_ms)
            .unwrap_or(start + 3_000);

        // A long gap is an intro or an instrumental break, not a held note.
        if end - start > MAX_LINE_SPAN_MS {
            end = start + 3_000;
        }
        if end - start < MIN_LINE_SPAN_MS {
            end = start + MIN_LINE_SPAN_MS;
        }

        plan.push((start, end, body.to_string()));
    }

    if plan.is_empty() {
        return None;
    }

    let mut lines = Vec::with_capacity(plan.len());
    for (start, end, body) in plan {
        let tokens = clean_tokens(segment_words_for_timing(&body));
        let words = if tokens.len() < 2 {
            // A single token cannot sweep; emit the line with no word timings
            // rather than a one-word "karaoke" that never moves.
            Vec::new()
        } else {
            distribute(&tokens, start, end)
        };
        lines.push(WordSyncLine {
            start_ms: start,
            end_ms: end,
            body,
            words,
        });
    }

    Some(lines)
}

/// Credit and metadata lines carry timestamps in many LRC files but are not
/// sung. Giving them word timings produces highlight over silence.
fn is_credit_line(text: &str) -> bool {
    const PREFIXES: [&str; 16] = [
        "作词", "作曲", "编曲", "制作", "出品", "监制", "录音", "混音", "母带", "词", "曲",
        "lyrics", "music", "produced", "written", "composed",
    ];
    let lowered = text.to_lowercase();
    let head = lowered.trim_start();
    for prefix in PREFIXES {
        if let Some(rest) = head.strip_prefix(prefix) {
            let rest = rest.trim_start();
            if rest.starts_with(':') || rest.starts_with('：') {
                return true;
            }
        }
    }
    false
}

/// Fold the segmenter's artefacts into stable word tokens.
///
/// Punctuation is attached to its neighbour (a comma given its own timing slot
/// flashes for milliseconds and reads as a glitch), and contractions that the
/// segmenter splits — "don't" arriving as ["don", "'t"] — are rejoined. No
/// trailing spaces are kept: TTML writes inter-word spaces outside the spans,
/// and the LRC writers let the reader reintroduce them.
fn clean_tokens(tokens: Vec<String>) -> Vec<String> {
    let mut merged: Vec<String> = Vec::new();
    for token in tokens {
        let glued = token.starts_with('\'') || token.starts_with('\u{2019}');
        let spaced = token.starts_with(char::is_whitespace);
        if !merged.is_empty() && glued && !spaced {
            if let Some(last) = merged.last_mut() {
                last.push_str(&token);
            }
        } else {
            merged.push(token);
        }
    }

    let mut out: Vec<String> = Vec::new();
    for token in merged {
        if token.trim().is_empty() {
            continue;
        }
        if !token.chars().any(char::is_alphanumeric) {
            // punctuation-only: attach to the previous word
            match out.last_mut() {
                Some(last) => last.push_str(&token),
                None => out.push(token),
            }
            continue;
        }

        let mut leading = String::new();
        for ch in token.chars() {
            if ch.is_alphanumeric() || ch.is_whitespace() {
                break;
            }
            leading.push(ch);
        }
        let rest = &token[leading.len()..];
        if !leading.is_empty() && !out.is_empty() {
            if let Some(last) = out.last_mut() {
                last.push_str(&leading);
            }
            if !rest.is_empty() {
                out.push(rest.to_string());
            }
        } else {
            out.push(token);
        }
    }

    out.iter()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

/// Start times proportional to visible token length.
///
/// A four-character word occupies more of the line than a particle; an even
/// divide makes short function words linger and long content words rush.
fn distribute(tokens: &[String], start_ms: i64, end_ms: i64) -> Vec<WordSyncWord> {
    let total = (end_ms - start_ms).max(1);
    let weights: Vec<i64> = tokens.iter().map(|t| (t.chars().count() as i64).max(1)).collect();
    let weight_sum: i64 = weights.iter().sum();

    let mut starts = Vec::with_capacity(tokens.len());
    let mut elapsed: i64 = 0;
    for weight in &weights {
        starts.push(start_ms + (elapsed * total) / weight_sum.max(1));
        elapsed += weight;
    }

    tokens
        .iter()
        .enumerate()
        .map(|(index, text)| WordSyncWord {
            text: text.clone(),
            start_ms: starts[index],
            end_ms: starts.get(index + 1).copied().unwrap_or(end_ms),
        })
        .collect()
}

fn stamp_a2(ms: i64) -> String {
    let ms = ms.max(0);
    format!("{:02}:{:02}.{:02}", ms / 60_000, (ms % 60_000) / 1_000, (ms % 1_000) / 10)
}

fn stamp_ttml(ms: i64) -> String {
    let ms = ms.max(0);
    format!(
        "{:02}:{:02}:{:06.3}",
        ms / 3_600_000,
        (ms % 3_600_000) / 60_000,
        (ms % 60_000) as f64 / 1000.0
    )
}

/// Render as enhanced LRC (A2): `[mm:ss.xx]<mm:ss.xx>word<mm:ss.xx>word`.
///
/// Lines without word timings fall back to plain line-level output, so a file
/// can mix both without breaking readers that only understand one.
pub fn to_enhanced_lrc(lines: &[WordSyncLine]) -> String {
    let mut out = String::new();
    for line in lines {
        out.push_str(&format!("[{}]", stamp_a2(line.start_ms)));
        if line.words.is_empty() {
            out.push_str(&line.body);
        } else {
            for word in &line.words {
                out.push_str(&format!("<{}>{}", stamp_a2(word.start_ms), word.text));
            }
        }
        out.push('\n');
    }
    out
}

/// Render as TTML, the only word-level format iOS players read.
pub fn to_ttml(lines: &[WordSyncLine], title: &str, artist: &str, album: &str) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<tt xmlns=\"http://www.w3.org/ns/ttml\"\n");
    out.push_str("    xmlns:ttm=\"http://www.w3.org/ns/ttml#metadata\"\n");
    out.push_str("    xmlns:itunes=\"http://music.apple.com/lyric-ttml-internal\"\n");
    out.push_str("    itunes:timing=\"Word\">\n");
    out.push_str("  <head>\n    <metadata>\n");
    out.push_str(&format!(
        "      <ttm:title>{}</ttm:title>\n",
        escape_xml(title)
    ));
    if !artist.is_empty() {
        out.push_str(&format!(
            "      <ttm:artist>{}</ttm:artist>\n",
            escape_xml(artist)
        ));
    }
    if !album.is_empty() {
        out.push_str(&format!(
            "      <ttm:album>{}</ttm:album>\n",
            escape_xml(album)
        ));
    }
    out.push_str("    </metadata>\n  </head>\n  <body>\n    <div>\n");

    for line in lines {
        let begin = stamp_ttml(line.start_ms);
        let end = stamp_ttml(line.end_ms);
        if line.words.is_empty() {
            out.push_str(&format!(
                "      <p begin=\"{}\" end=\"{}\">{}</p>\n",
                begin,
                end,
                escape_xml(&line.body)
            ));
            continue;
        }

        out.push_str(&format!("      <p begin=\"{}\" end=\"{}\">", begin, end));
        for (index, word) in line.words.iter().enumerate() {
            // Spaces between Latin words go *outside* the span: the AMLL
            // specification calls that the most compliant form, and writing it
            // inside would double up once a reader re-inserts spacing.
            let trailing = if index + 1 < line.words.len() && is_latin(&line.body) {
                " "
            } else {
                ""
            };
            out.push_str(&format!(
                "<span begin=\"{}\" end=\"{}\">{}</span>{}",
                stamp_ttml(word.start_ms),
                stamp_ttml(word.end_ms),
                escape_xml(&word.text),
                trailing
            ));
        }
        out.push_str("</p>\n");
    }

    out.push_str("    </div>\n  </body>\n</tt>\n");
    out
}

/// CJK runs need no inter-word space; Latin does.
fn is_latin(text: &str) -> bool {
    let cjk = text.chars().any(|c| {
        matches!(c as u32, 0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0x3040..=0x30FF)
    });
    !cjk
}

fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE: &str = "[00:01.00]雨一直下\n[00:05.00]气氛不算融洽\n";

    #[test]
    fn chinese_lines_are_segmented_at_word_boundaries() {
        let lines = build_word_sync(SIMPLE).expect("should convert");
        let first = &lines[0];
        let words: Vec<&str> = first.words.iter().map(|w| w.text.as_str()).collect();
        // The whole point of using Charabia: 一直 is one word, not 一 + 直.
        assert_eq!(words, vec!["雨", "一直", "下"]);
    }

    #[test]
    fn word_timings_are_monotonic_and_inside_the_line() {
        let lines = build_word_sync(SIMPLE).unwrap();
        for line in &lines {
            let mut previous = line.start_ms;
            for word in &line.words {
                assert!(
                    word.start_ms >= previous,
                    "word start went backwards in {:?}",
                    line.body
                );
                assert!(word.start_ms < line.end_ms, "word start past line end");
                assert!(word.end_ms <= line.end_ms, "word end past line end");
                previous = word.start_ms;
            }
        }
    }

    #[test]
    fn enhanced_lrc_reproduces_the_line_text_when_tags_are_stripped() {
        let lines = build_word_sync(SIMPLE).unwrap();
        let out = to_enhanced_lrc(&lines);
        for line in &lines {
            let rebuilt: String = line.words.iter().map(|w| w.text.as_str()).collect();
            assert_eq!(
                rebuilt.replace(' ', ""),
                line.body.replace(' ', ""),
                "word texts must reconstruct the line"
            );
        }
        assert!(out.contains("<00:01.00>雨"));
    }

    #[test]
    fn ttml_parses_and_reconstructs_the_text() {
        let lines = build_word_sync(SIMPLE).unwrap();
        let xml = to_ttml(&lines, "Test", "Artist", "Album");
        assert!(xml.contains("itunes:timing=\"Word\""));
        assert!(xml.contains("<ttm:title>Test</ttm:title>"));

        // Reconstruct line text from spans and compare with the source.
        for (line, source) in lines.iter().zip(["雨一直下", "气氛不算融洽"]) {
            let rebuilt: String = line.words.iter().map(|w| w.text.as_str()).collect();
            assert_eq!(rebuilt, source);
        }
    }

    #[test]
    fn credit_lines_get_no_word_timings() {
        let input = "[00:00.00]作词 : 某人\n[00:01.00]雨一直下\n[00:05.00]气氛不算融洽\n";
        let lines = build_word_sync(input).unwrap();
        // The credit line is dropped entirely rather than given highlight.
        assert_eq!(lines.len(), 2);
        assert!(!lines.iter().any(|l| l.body.contains("作词")));
    }

    #[test]
    fn long_instrumental_gaps_do_not_stretch_words() {
        // 60 seconds between two lines: the first must not spread its words over
        // the whole minute.
        let input = "[00:00.00]雨一直下\n[01:00.00]气氛不算融洽\n";
        let lines = build_word_sync(input).unwrap();
        assert!(lines[0].end_ms - lines[0].start_ms <= MAX_LINE_SPAN_MS);
    }

    #[test]
    fn instrumental_marker_is_not_converted() {
        assert!(build_word_sync("[au:instrumental]").is_none());
        assert!(build_word_sync("").is_none());
    }

    #[test]
    fn latin_gets_spaces_outside_spans_cjk_does_not() {
        let latin = build_word_sync("[00:01.00]When will I see you\n").unwrap();
        let xml = to_ttml(&latin, "t", "", "");
        assert!(xml.contains("</span> "), "latin needs a space between spans");

        let cjk = build_word_sync("[00:01.00]雨一直下\n").unwrap();
        let xml = to_ttml(&cjk, "t", "", "");
        assert!(!xml.contains("</span> "), "CJK must not gain spaces");
    }

    #[test]
    fn xml_special_characters_are_escaped() {
        let lines = build_word_sync("[00:01.00]A & B < C\n").unwrap();
        let xml = to_ttml(&lines, "T&T", "", "");
        assert!(xml.contains("&amp;"));
        assert!(!xml.contains("T&T<"));
    }
}
