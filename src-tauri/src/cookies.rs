//! Reading and writing the cookie file that `yt-dlp` needs.
//!
//! YouTube is not reachable anonymously from a residential network without
//! either a proof-of-origin token or a signed-in session. The token is
//! produced by the POT provider; the session comes from the browser, and the
//! only way to get it out of a Chromium browser on Linux is to export it: the
//! cookie database is encrypted with a key in the login keyring, which a
//! desktop session that is not the browser's own cannot read.
//!
//! So the user exports the cookies as JSON with a browser extension, and this
//! module turns that into the Netscape text format `yt-dlp` reads. The
//! conversion lives here rather than in a helper script so that the GUI can
//! do it in one step: the user picks the JSON file the extension wrote, and
//! never has to run a second tool.
//!
//! ## The expiry problem
//!
//! A browser rotates the session cookies (`__Secure-1PSID` and friends)
//! whenever it decides to, and a rotated cookie is silently useless: the
//! request is answered with "Sign in to confirm you're not a bot" rather than
//! with a message about the cookie. So the JSON is checked for expiry when it
//! is read, and the GUI says so plainly, because the alternative is a user
//! staring at a bot error with no idea that the file on disk is the problem.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The Netscape cookie file format, as `yt-dlp` and `curl` both read it.
const NETSCAPE_HEADER: &str = "# Netscape HTTP Cookie File";

/// A cookie as a browser extension writes it.
///
/// The exporters disagree on the field names and on how they write the
/// expiry, so every field has a fallback and the expiry is parsed leniently.
#[derive(Debug, Deserialize, Clone)]
struct RawCookie {
    #[serde(default)]
    name: String,
    #[serde(default)]
    value: String,
    #[serde(default)]
    domain: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    expires: Option<serde_json::Value>,
    #[serde(default)]
    expiration_date: Option<serde_json::Value>,
    #[serde(default)]
    http_only: Option<bool>,
    #[serde(default)]
    httpOnly: Option<bool>,
    #[serde(default)]
    secure: Option<bool>,
    #[serde(default)]
    host_only: Option<bool>,
    #[serde(default)]
    hostOnly: Option<bool>,
}

/// The whole file, in the shapes the common exporters use.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum CookieFile {
    /// `[{...}, {...}]`
    List(Vec<RawCookie>),
    /// `{"cookies": [{...}]}`
    Wrapped {
        #[serde(default)]
        cookies: Vec<RawCookie>,
    },
    /// A single cookie object.
    Single(RawCookie),
}

impl CookieFile {
    fn cookies(&self) -> Vec<RawCookie> {
        match self {
            CookieFile::List(list) => list.clone(),
            CookieFile::Wrapped { cookies } => cookies.clone(),
            CookieFile::Single(single) => vec![single.clone()],
        }
    }
}

/// A cookie after conversion: the fields the Netscape format needs.
#[derive(Debug, Clone)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    /// Seconds since the epoch, or `0` for a session cookie.
    pub expires: i64,
}

impl Cookie {
    /// The one line this cookie becomes in the Netscape file.
    fn to_netscape_line(&self) -> String {
        // A leading dot means "and every subdomain", which the format records
        // as TRUE in the second column.
        let include_subdomains = if self.domain.starts_with('.') {
            "TRUE"
        } else {
            "FALSE"
        };
        let secure = if self.secure { "TRUE" } else { "FALSE" };
        // httpOnly cookies are unreadable from JavaScript. yt-dlp is not
        // JavaScript, but the marker is how the format says "this one is
        // marked httpOnly", and some readers key off it.
        let prefix = if self.http_only { "#HttpOnly_" } else { "" };

        format!(
            "{prefix}{domain}\t{include_subdomains}\t{path}\t{secure}\t{expires}\t{name}\t{value}",
            domain = self.domain,
            path = self.path,
            expires = self.expires,
            name = self.name,
            value = self.value,
        )
    }
}

/// What the GUI needs to know after reading a cookie file.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CookieStatus {
    /// Where the Netscape file was written.
    pub path: String,
    /// How many cookies it holds.
    pub count: usize,
    /// How many of them have an expiry in the past.
    pub expired: usize,
    /// Whether the file holds a signed-in session.
    ///
    /// The session cookies are what identify a signed-in browser, so a file
    /// without them is a signed-out session: usable for public videos, and
    /// not enough for the ones that demand a login.
    pub signed_in: bool,
    /// The earliest expiry among the session cookies, if any.
    pub session_expires: Option<i64>,
}

/// Read a browser-exported cookie JSON and write the Netscape file beside it.
///
/// The Netscape file gets the `.txt` suffix, because that is what the
/// exporters themselves call it and what a user will look for.
pub fn convert_json_file(json_path: &Path) -> Result<CookieStatus> {
    let text = std::fs::read_to_string(json_path)
        .with_context(|| format!("cannot read {}", json_path.display()))?;
    let cookies = parse_json(&text)?;
    if cookies.is_empty() {
        anyhow::bail!("no cookies found in {}", json_path.display());
    }

    let target = netscape_path_for(json_path);
    let body = render_netscape(&cookies);
    std::fs::write(&target, body)
        .with_context(|| format!("cannot write {}", target.display()))?;

    Ok(status_for(&target, &cookies))
}

/// The Netscape file that belongs to a JSON file.
pub fn netscape_path_for(json_path: &Path) -> PathBuf {
    let mut target = json_path.to_path_buf();
    target.set_extension("txt");
    target
}

/// Turn the text of an export into cookies.
pub fn parse_json(text: &str) -> Result<Vec<Cookie>> {
    let parsed: CookieFile = serde_json::from_str(text).context("the file is not a cookie export")?;
    let raw = parsed.cookies();

    let mut cookies = Vec::with_capacity(raw.len());
    for entry in raw {
        if entry.name.is_empty() {
            continue;
        }
        let host_only = entry.host_only.or(entry.hostOnly).unwrap_or(false);
        let domain = normalise_domain(&entry.domain, host_only);
        if domain.is_empty() {
            continue;
        }
        cookies.push(Cookie {
            name: entry.name,
            value: entry.value,
            domain,
            path: if entry.path.is_empty() {
                "/".to_owned()
            } else {
                entry.path
            },
            secure: entry.secure.unwrap_or(false),
            http_only: entry.http_only.or(entry.httpOnly).unwrap_or(false),
            expires: parse_expiry(entry.expires, entry.expiration_date),
        });
    }

    Ok(cookies)
}

/// The Netscape file's text.
pub fn render_netscape(cookies: &[Cookie]) -> String {
    let mut out = String::from(NETSCAPE_HEADER);
    out.push('\n');
    for cookie in cookies {
        out.push_str(&cookie.to_netscape_line());
        out.push('\n');
    }
    out
}

/// Read a Netscape cookie file and report what is in it.
///
/// This is what the GUI calls on startup, to say whether the cookies the
/// user exported are still usable.
pub fn inspect_netscape(path: &Path) -> Result<CookieStatus> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read {}", path.display()))?;
    let cookies = parse_netscape(&text);
    Ok(status_for(path, &cookies))
}

/// Parse a Netscape cookie file.
pub fn parse_netscape(text: &str) -> Vec<Cookie> {
    let mut cookies = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            continue;
        }
        // The "#HttpOnly_" marker prefixes the domain, so it has to come off
        // before the fields are split, and the httpOnly flag has to be kept.
        let (line, http_only) = match trimmed.strip_prefix("#HttpOnly_") {
            Some(rest) => (rest, true),
            None => (trimmed, false),
        };
        if line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 7 {
            continue;
        }
        cookies.push(Cookie {
            domain: fields[0].to_owned(),
            path: fields[2].to_owned(),
            secure: fields[3].eq_ignore_ascii_case("TRUE"),
            expires: fields[4].parse().unwrap_or(0),
            name: fields[5].to_owned(),
            value: fields[6].to_owned(),
            http_only,
        });
    }
    cookies
}

/// The names that identify a signed-in YouTube session.
///
/// `SID` is the session id itself. The `__Secure-1PSID` family are the
/// per-application session cookies. A signed-out browser still carries
/// `__Secure-3PAPISID` and friends, so those are not counted here.
const SESSION_COOKIE_NAMES: [&str; 3] = ["SID", "__Secure-1PSID", "__Secure-3PSID"];

fn status_for(path: &Path, cookies: &[Cookie]) -> CookieStatus {
    let now = now_seconds();
    let mut expired = 0;
    let mut signed_in = false;
    let mut session_expires: Option<i64> = None;

    for cookie in cookies {
        if cookie.expires > 0 && cookie.expires < now {
            expired += 1;
        }
        if SESSION_COOKIE_NAMES.contains(&cookie.name.as_str()) && !cookie.value.is_empty() {
            signed_in = true;
            if cookie.expires > 0 {
                session_expires = Some(match session_expires {
                    Some(current) => current.min(cookie.expires),
                    None => cookie.expires,
                });
            }
        }
    }

    CookieStatus {
        path: path.display().to_string(),
        count: cookies.len(),
        expired,
        signed_in,
        session_expires,
    }
}

/// The domain to write, with the leading dot restored.
///
/// Some exporters drop it, and it matters: without it the cookie is only sent
/// to the exact host, and `music.youtube.com` and `www.youtube.com` would not
/// both receive it.
fn normalise_domain(domain: &str, host_only: bool) -> String {
    let domain = domain.trim();
    if domain.is_empty() {
        return String::new();
    }
    if host_only || domain.starts_with('.') {
        return domain.to_owned();
    }
    // A host-only cookie for a subdomain stays host-only; a plain domain like
    // "youtube.com" covers the subdomains.
    if domain.contains('.') && !domain.starts_with("www.") {
        format!(".{domain}")
    } else {
        domain.to_owned()
    }
}

/// The expiry as seconds since the epoch, or `0` for a session cookie.
///
/// The exporters write it as a float in seconds, as an integer in
/// milliseconds, or as an ISO date, and they disagree about which. A value
/// large enough to be a millisecond timestamp is divided down; a value in the
/// past stays in the past, because a cookie that has already expired is
/// still expired after any unit correction.
fn parse_expiry(expires: Option<serde_json::Value>, expiration_date: Option<serde_json::Value>) -> i64 {
    let raw = expires.or(expiration_date);
    let Some(raw) = raw else {
        return 0;
    };

    match raw {
        serde_json::Value::Number(number) => {
            let value = number.as_f64().unwrap_or(0.0);
            normalise_epoch(value)
        }
        serde_json::Value::String(text) => {
            let text = text.trim();
            if text.is_empty() {
                return 0;
            }
            if let Ok(value) = text.parse::<f64>() {
                return normalise_epoch(value);
            }
            // An ISO 8601 date, which is what some exporters write.
            if let Ok(parsed) = chrono_like_parse(text) {
                return parsed;
            }
            0
        }
        _ => 0,
    }
}

/// Bring a timestamp into seconds.
///
/// The threshold is the year 5138 in seconds, which is where the millisecond
/// range crosses over. Anything above it is milliseconds.
fn normalise_epoch(value: f64) -> i64 {
    if value <= 0.0 {
        return 0;
    }
    if value > 1e11 {
        return (value / 1000.0) as i64;
    }
    value as i64
}

/// Parse an ISO 8601 timestamp without pulling in a date library.
///
/// The format is fixed: `YYYY-MM-DDTHH:MM:SS` with an optional `Z` and
/// optional fractional seconds. Hand-parsing keeps the dependency list of the
/// whole project unchanged for one field of a cookie file.
fn chrono_like_parse(text: &str) -> Result<i64> {
    let text = text.trim_end_matches('Z');
    let (date, time) = text
        .split_once(['T', ' '])
        .ok_or_else(|| anyhow::anyhow!("not an ISO timestamp"))?;

    let mut date_parts = date.split('-');
    let year: i64 = date_parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("not a date"))?
        .parse()?;
    let month: i64 = date_parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("not a date"))?
        .parse()?;
    let day: i64 = date_parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("not a date"))?
        .parse()?;

    let time = time.split('.').next().unwrap_or(time);
    let mut time_parts = time.split(':');
    let hour: i64 = time_parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("not a time"))?
        .parse()?;
    let minute: i64 = time_parts.next().unwrap_or("0").parse()?;
    let second: i64 = time_parts.next().unwrap_or("0").parse()?;

    Ok(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// Days since the Unix epoch for a civil date.
///
/// Howard Hinnant's algorithm: it works for every date the Gregorian calendar
/// has, with no lookup table and no iteration.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_prime = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_of_cookies_becomes_a_netscape_file() {
        let json = r#"[
            {"name":"SID","value":"abc","domain":".youtube.com","path":"/",
             "expires":1799999999,"httpOnly":true,"secure":true}
        ]"#;
        let cookies = parse_json(json).unwrap();
        assert_eq!(cookies.len(), 1);
        let text = render_netscape(&cookies);
        assert!(text.starts_with(NETSCAPE_HEADER));
        assert!(text.contains("#HttpOnly_.youtube.com\tTRUE\t/\tTRUE\t1799999999\tSID\tabc"));
    }

    #[test]
    fn a_wrapped_export_is_accepted() {
        let json = r#"{"cookies":[
            {"name":"A","value":"1","domain":".youtube.com","path":"/","expires":1}
        ]}"#;
        assert_eq!(parse_json(json).unwrap().len(), 1);
    }

    #[test]
    fn a_millisecond_expiry_is_brought_down_to_seconds() {
        let json = r#"[{"name":"A","value":"1","domain":"youtube.com",
            "expires":1799999999000}]"#;
        let cookies = parse_json(json).unwrap();
        assert_eq!(cookies[0].expires, 1_799_999_999);
    }

    #[test]
    fn an_iso_expiry_is_parsed() {
        let json = r#"[{"name":"A","value":"1","domain":"youtube.com",
            "expires":"2026-10-24T12:41:15Z"}]"#;
        let cookies = parse_json(json).unwrap();
        // 2026-10-24T12:41:15Z is 1792845675 seconds after the epoch.
        assert_eq!(cookies[0].expires, 1_792_845_675);
    }

    #[test]
    fn a_session_cookie_has_no_expiry() {
        let json = r#"[{"name":"A","value":"1","domain":"youtube.com"}]"#;
        let cookies = parse_json(json).unwrap();
        assert_eq!(cookies[0].expires, 0);
    }

    #[test]
    fn a_signed_in_session_is_recognised() {
        let cookies = parse_netscape(
            ".youtube.com\tTRUE\t/\tTRUE\t0\tSID\tvalue\n\
             .youtube.com\tTRUE\t/\tTRUE\t0\t__Secure-3PAPISID\tother\n",
        );
        let status = status_for(Path::new("/tmp/x.txt"), &cookies);
        assert!(status.signed_in);
        assert_eq!(status.count, 2);
    }

    #[test]
    fn a_signed_out_browser_is_not_a_session() {
        let cookies = parse_netscape(".youtube.com\tTRUE\t/\tTRUE\t0\t__Secure-3PAPISID\tv\n");
        let status = status_for(Path::new("/tmp/x.txt"), &cookies);
        assert!(!status.signed_in);
    }

    #[test]
    fn an_expired_cookie_is_counted() {
        let cookies = parse_netscape(".youtube.com\tTRUE\t/\tTRUE\t1\tSID\tv\n");
        let status = status_for(Path::new("/tmp/x.txt"), &cookies);
        assert_eq!(status.expired, 1);
    }

    #[test]
    fn the_netscape_file_is_written_beside_the_json() {
        let dir = std::env::temp_dir().join("singalong-cookie-test");
        std::fs::create_dir_all(&dir).unwrap();
        let json_path = dir.join("www.youtube.com.cookies.json");
        std::fs::write(
            &json_path,
            r#"[{"name":"SID","value":"v","domain":".youtube.com","path":"/","expires":0}]"#,
        )
        .unwrap();

        let status = convert_json_file(&json_path).unwrap();
        assert_eq!(status.count, 1);
        assert!(status.path.ends_with("www.youtube.com.cookies.txt"));

        // And it reads back as the same cookie.
        let cookies = parse_netscape(
            &std::fs::read_to_string(&status.path).unwrap(),
        );
        assert_eq!(cookies[0].name, "SID");
        assert_eq!(cookies[0].value, "v");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_host_only_domain_keeps_its_shape() {
        let json = r#"[{"name":"A","value":"1","domain":"music.youtube.com",
            "hostOnly":true}]"#;
        let cookies = parse_json(json).unwrap();
        assert_eq!(cookies[0].domain, "music.youtube.com");
    }

    #[test]
    fn a_plain_domain_gains_the_leading_dot() {
        let json = r#"[{"name":"A","value":"1","domain":"youtube.com"}]"#;
        let cookies = parse_json(json).unwrap();
        assert_eq!(cookies[0].domain, ".youtube.com");
    }
}
