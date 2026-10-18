use std::io::{self, BufRead, Write};

/// Charabia-based segmentation, mirroring lrcget's merge_segment_tokens():
/// separators-only segments are folded into neighbouring word tokens so that
/// no token is pure whitespace/punctuation.
fn merge(raw: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut prefix = String::new();
    for seg in raw {
        let is_word = seg.chars().any(|c| c.is_alphanumeric());
        if is_word {
            let mut t = std::mem::take(&mut prefix);
            t.push_str(&seg);
            out.push(t);
        } else {
            prefix.push_str(&seg);
        }
    }
    if !prefix.is_empty() {
        match out.last_mut() {
            Some(last) => last.push_str(&prefix),
            None => out.push(prefix),
        }
    }
    out
}

fn main() {
    use charabia::Segment;
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = line.unwrap_or_default();
        let raw: Vec<String> = (line.as_str() as &str).segment_str()
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        let tokens = merge(raw);
        let json = serde_json::to_string(&tokens).unwrap_or_else(|_| "[]".into());
        writeln!(stdout, "{json}").ok();
    }
}
