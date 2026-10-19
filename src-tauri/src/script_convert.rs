//! Convert lyrics between Simplified and Traditional Chinese.
//!
//! Two passes, because a character table alone gets common words wrong:
//!
//! 1. **Phrases** — only the entries where a character-by-character pass would
//!    pick the wrong variant. 头发 must become 頭髮, not 頭發; 发展 must become
//!    發展, not 髮展. The table holds just those cases, which is why it is 9k
//!    entries rather than OpenCC's full 46k phrase list.
//! 2. **Characters** — everything the phrase pass did not cover.
//!
//! Tables are generated from OpenCC (Apache-2.0) and live in `data/`. This is
//! still not perfect: a handful of one-to-many characters depend on context that
//! neither table can see, which is why conversion is an explicit user choice
//! recorded once on the download popup, never applied silently.

use std::collections::HashMap;
use std::sync::OnceLock;

#[path = "../data/script_s2t.rs"]
mod script_s2t;
#[path = "../data/script_t2s.rs"]
mod script_t2s;

struct Table {
    chars: HashMap<char, char>,
    /// Longest-first so the longest match wins.
    phrases: Vec<(&'static str, &'static str)>,
    max_phrase_len: usize,
}

fn build(chars: &'static [(char, char)], phrases: &'static [(&'static str, &'static str)]) -> Table {
    let mut sorted = phrases.to_vec();
    sorted.sort_by_key(|(source, _)| std::cmp::Reverse(source.chars().count()));
    let max_phrase_len = sorted
        .iter()
        .map(|(source, _)| source.chars().count())
        .max()
        .unwrap_or(0);
    Table {
        chars: chars.iter().copied().collect(),
        phrases: sorted,
        max_phrase_len,
    }
}

fn s2t() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| build(script_s2t::S2T_CHARS, script_s2t::S2T_PHRASES))
}

fn t2s() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| build(script_t2s::T2S_CHARS, script_t2s::T2S_PHRASES))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScriptTarget {
    /// Leave the lyrics exactly as published.
    Off,
    Simplified,
    Traditional,
}

impl ScriptTarget {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            // 'ask' means the user has not answered yet; treating it as Off keeps
            // a batch that starts before the answer arrives from converting.
            "off" | "ask" => Some(ScriptTarget::Off),
            "simplified" => Some(ScriptTarget::Simplified),
            "traditional" => Some(ScriptTarget::Traditional),
            _ => None,
        }
    }
}

fn convert_with(table: &Table, text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + text.len() / 4);
    let mut index = 0;

    while index < chars.len() {
        let mut matched = 0;

        if chars[index].is_alphanumeric() || is_ideograph(chars[index]) {
            let remaining = chars.len() - index;
            let longest = table.max_phrase_len.min(remaining);
            for length in (2..=longest).rev() {
                let candidate: String = chars[index..index + length].iter().collect();
                if let Some((_, replacement)) =
                    table.phrases.iter().find(|(source, _)| *source == candidate)
                {
                    out.push_str(replacement);
                    index += length;
                    matched = length;
                    break;
                }
            }
        }

        if matched == 0 {
            let ch = chars[index];
            out.push(table.chars.get(&ch).copied().unwrap_or(ch));
            index += 1;
        }
    }

    out
}

fn is_ideograph(ch: char) -> bool {
    matches!(ch as u32, 0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0xF900..=0xFAFF)
}

/// Convert text to the requested script. `Off` and text with no CJK return the
/// input unchanged.
pub fn convert(text: &str, target: ScriptTarget) -> String {
    if !has_cjk(text) {
        return text.to_string();
    }

    match target {
        ScriptTarget::Off => text.to_string(),
        ScriptTarget::Simplified => convert_with(t2s(), text),
        ScriptTarget::Traditional => convert_with(s2t(), text),
    }
}

/// Whether conversion could change the text at all.
pub fn has_cjk(text: &str) -> bool {
    text.chars().any(is_ideograph)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phrase_table_resolves_ambiguous_characters() {
        // 发 is 發 in 发展 but 髮 in 头发; the phrase pass is what makes this right.
        assert_eq!(convert("头发很长", ScriptTarget::Traditional), "頭髮很長");
        assert_eq!(convert("发展经济", ScriptTarget::Traditional), "發展經濟");
    }

    #[test]
    fn falls_back_to_character_mapping() {
        assert_eq!(
            convert("送你一朵小红花", ScriptTarget::Traditional),
            "送你一朵小紅花"
        );
    }

    #[test]
    fn off_returns_input_unchanged() {
        let text = "[00:01.00]送你一朵小红花";
        assert_eq!(convert(text, ScriptTarget::Off), text);
    }

    #[test]
    fn timestamps_and_latin_are_untouched() {
        let text = "[00:12.34]When will I see you again?";
        assert_eq!(convert(text, ScriptTarget::Traditional), text);
    }

    #[test]
    fn round_trip_restores_the_original() {
        let original = "送你一朵小红花";
        let traditional = convert(original, ScriptTarget::Traditional);
        assert_eq!(convert(&traditional, ScriptTarget::Simplified), original);
    }

    #[test]
    fn parse_accepts_every_stored_value() {
        assert_eq!(ScriptTarget::parse("off"), Some(ScriptTarget::Off));
        assert_eq!(ScriptTarget::parse("ask"), Some(ScriptTarget::Off));
        assert_eq!(ScriptTarget::parse("simplified"), Some(ScriptTarget::Simplified));
        assert_eq!(ScriptTarget::parse("traditional"), Some(ScriptTarget::Traditional));
        assert_eq!(ScriptTarget::parse("klingon"), None);
    }

    #[test]
    fn has_cjk_distinguishes_scripts() {
        assert!(has_cjk("小紅花"));
        assert!(!has_cjk("When will I see you"));
        assert!(!has_cjk("[00:01.00]"));
    }
}
