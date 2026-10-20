use std::time::Duration;

use anyhow::Result;
use reqwest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchItem {
    id: i64,
    name: Option<String>,
    artist_name: Option<String>,
    album_name: Option<String>,
    duration: Option<f64>,
    instrumental: bool,
    plain_lyrics: Option<String>,
    synced_lyrics: Option<String>,
    lyricsfile: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct Response(Vec<SearchItem>);

#[derive(Error, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[error("{error}: {message}")]
pub struct ResponseError {
    status_code: Option<u16>,
    error: String,
    message: String,
}

pub async fn request(
    title: &str,
    album_name: &str,
    artist_name: &str,
    q: &str,
    lrclib_instance: &str,
    proxy_url: &str,
) -> Result<Response> {
    // Only send parameters that carry a value. Empty fields are still treated as
    // filters by the endpoint, and a request with three empty filters next to a
    // real `q` is rejected often enough to look like a flaky network.
    let params: Vec<(String, String)> = [
        ("track_name", title),
        ("artist_name", artist_name),
        ("album_name", album_name),
        ("q", q),
    ]
    .into_iter()
    .filter(|(_, value)| !value.trim().is_empty())
    .map(|(key, value)| (key.to_owned(), value.to_owned()))
    .collect();

    let version = env!("CARGO_PKG_VERSION");
    let user_agent = format!(
        "Sing-Along v{} (https://github.com/LeisureLinux/Sing-Along)",
        version
    );
    // reqwest reads HTTPS_PROXY/HTTP_PROXY from the environment by default. On a
    // machine where those are set for other tooling, every LRCLIB request then
    // goes through a proxy that may not reach it, and the failure surfaces as
    // "error sending request" — indistinguishable from LRCLIB being down. The
    // instance URL is configured by the user, so a proxy meant for something else
    // should not be silently in the path.
    let client = crate::http::client_with_agent(proxy_url, 10, &user_agent)?;
    let api_endpoint = format!("{}/api/search", lrclib_instance.trim_end_matches('/'));
    let url = reqwest::Url::parse_with_params(&api_endpoint, &params)?;
    let res = client.get(url).send().await?;

    match res.status() {
        reqwest::StatusCode::OK => {
            let lrclib_response = res.json::<Response>().await?;
            Ok(lrclib_response)
        }

        reqwest::StatusCode::BAD_REQUEST
        | reqwest::StatusCode::SERVICE_UNAVAILABLE
        | reqwest::StatusCode::INTERNAL_SERVER_ERROR => {
            let error = res.json::<ResponseError>().await?;
            Err(error.into())
        }

        _ => Err(ResponseError {
            status_code: None,
            error: "UnknownError".to_string(),
            message: "Unknown error happened".to_string(),
        }
        .into()),
    }
}
