//! Shared HTTP client construction.
//!
//! Every lyric API request goes through `client()`. The proxy is an explicit
//! setting rather than whatever the environment happens to contain: `reqwest`
//! reads `HTTP_PROXY`/`HTTPS_PROXY` by default, so launching the app from a shell
//! that had those set for other tooling silently routed lyric lookups through a
//! proxy that could not reach them. That failure looked exactly like the service
//! being down.

use std::time::Duration;

/// Build a client for talking to the lyric APIs.
///
/// `proxy_url` comes from the user's settings; an empty string means "connect
/// directly, ignoring the environment". When set, it is applied explicitly so the
/// choice is the user's, not the shell's.
pub fn client(proxy_url: &str, timeout_secs: u64) -> reqwest::Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder().timeout(Duration::from_secs(timeout_secs));
    builder = match proxy_url.trim() {
        "" => builder.no_proxy(),
        url => builder.proxy(reqwest::Proxy::all(url)?),
    };
    builder.build()
}

/// Build a client with the user agent the lyric services expect.
pub fn client_with_agent(
    proxy_url: &str,
    timeout_secs: u64,
    user_agent: &str,
) -> reqwest::Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .user_agent(user_agent);
    builder = match proxy_url.trim() {
        "" => builder.no_proxy(),
        url => builder.proxy(reqwest::Proxy::all(url)?),
    };
    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_proxy_builds_a_client() {
        assert!(client("", 5).is_ok());
        assert!(client("   ", 5).is_ok());
    }

    #[test]
    fn valid_proxy_urls_are_accepted() {
        for url in [
            "http://127.0.0.1:8888",
            "http://wpad.lan:8888",
            "socks5://127.0.0.1:1080",
        ] {
            assert!(client(url, 5).is_ok(), "{url} should be accepted");
        }
    }

    #[test]
    fn a_malformed_proxy_url_is_reported() {
        // Better to fail loudly than to silently fall back to a direct
        // connection the user did not ask for.
        assert!(client("not a url", 5).is_err());
    }
}
