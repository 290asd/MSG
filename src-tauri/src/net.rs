// The internet side. Electron's webSecurity:false and its Referer hook are replaced by two things here:
//  - `http_request`: the pages' XHR/fetch calls go through this, so no CORS applies and API keys never
//    show in a webview error.
//  - the `msg-proxy` scheme: remote images and videos are loaded through it, so the sites that reject
//    requests without their own Referer (e621, gelbooru, rule34, realbooru) get one, and the pictures
//    are same-origin enough for the Refract canvas.
use crate::App;
use reqwest::{header, Client, Url};
use serde::Serialize;
use std::collections::HashMap;
use std::time::Duration;
use tauri::http::{Request, Response};

// An honest User-Agent: e621 asks for one, and Cloudflare (Danbooru) turns away a browser's User-Agent that
// doesn't come with a browser's TLS handshake.
const USER_AGENT: &str = concat!("MSG/", env!("CARGO_PKG_VERSION"), " (booru slideshow)");
// A video is asked for as "bytes=N-"; answering with a chunk (instead of the whole file) keeps memory small.
const RANGE_CHUNK: u64 = 8 * 1024 * 1024;

pub fn client() -> Client {
    Client::builder().user_agent(USER_AGENT).build().expect("http client")
}

/// Only http(s) addresses are ever fetched.
pub fn web_url(url: &str) -> Option<Url> {
    Url::parse(url).ok().filter(|u| matches!(u.scheme(), "http" | "https"))
}

/// The Referer each site's image server expects.
pub fn referer_for(url: &Url) -> Option<&'static str> {
    let host = url.host_str()?;
    if host.ends_with("e621.net") {
        Some("https://e621.net/")
    } else if host.ends_with("gelbooru.com") {
        Some("https://gelbooru.com")
    } else if host.ends_with("rule34.xxx") {
        Some("https://rule34.xxx/")
    } else if host.ends_with("realbooru.com") {
        Some("https://realbooru.com/")
    } else {
        None
    }
}

// Never the request URL: a Rule34 URL contains the API key.
pub fn describe(e: reqwest::Error) -> String {
    if e.is_timeout() {
        "timeout".into()
    } else if e.is_connect() {
        "connection failed".into()
    } else {
        "network error".into()
    }
}

/// A GET with the site's Referer, for downloads and background images.
pub async fn get(state: &App, url: &str) -> Result<reqwest::Response, String> {
    let url = web_url(url).ok_or("Not a web address")?;
    let mut request = state.client.get(url.clone());
    if let Some(referer) = referer_for(&url) {
        request = request.header(header::REFERER, referer);
    }
    let response = request.send().await.map_err(describe)?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status().as_u16()));
    }
    Ok(response)
}

#[derive(Serialize)]
pub struct HttpResponse {
    status: u16,
    body: String,
    headers: Vec<(String, String)>,
}

#[tauri::command]
pub async fn http_request(
    state: tauri::State<'_, App>,
    url: String,
    method: Option<String>,
    headers: Option<HashMap<String, String>>,
    body: Option<String>,
    timeout_ms: Option<u64>,
) -> Result<HttpResponse, String> {
    if state.store.flag("offlineMode") {
        return Err("offline".into());
    }
    let url = web_url(&url).ok_or("Not a web address")?;
    let method = reqwest::Method::from_bytes(method.unwrap_or_else(|| "GET".into()).as_bytes()).map_err(|_| "bad method")?;
    let mut request = state.client.request(method, url.clone()).timeout(Duration::from_millis(timeout_ms.filter(|t| *t > 0).unwrap_or(30_000)));
    if let Some(referer) = referer_for(&url) {
        request = request.header(header::REFERER, referer);
    }
    for (key, value) in headers.unwrap_or_default() {
        request = request.header(key, value);
    }
    if let Some(body) = body {
        request = request.body(body);
    }
    let response = request.send().await.map_err(describe)?;
    let status = response.status().as_u16();
    let headers = response.headers().iter().filter_map(|(k, v)| Some((k.to_string(), v.to_str().ok()?.to_string()))).collect();
    let body = response.text().await.map_err(describe)?;
    Ok(HttpResponse { status, body, headers })
}

fn reply(status: u16, body: Vec<u8>) -> Response<Vec<u8>> {
    Response::builder().status(status).header("access-control-allow-origin", "*").body(body).unwrap()
}

/// msg-proxy://localhost/<encoded address>: fetches it with the site's Referer and hands it to the page.
pub async fn proxy(state: &App, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    if state.store.flag("offlineMode") {
        return reply(403, vec![]);
    }
    let Some(url) = urlencoding::decode(request.uri().path().trim_start_matches('/')).ok().and_then(|u| web_url(&u)) else {
        return reply(400, vec![]);
    };

    let mut upstream = state.client.get(url.clone()).timeout(Duration::from_secs(60));
    if let Some(referer) = referer_for(&url) {
        upstream = upstream.header(header::REFERER, referer);
    }
    if let Some(range) = request.headers().get(header::RANGE).and_then(|v| v.to_str().ok()) {
        // "bytes=START-END" or "bytes=START-": the second form is capped to a chunk.
        if let Some((start, end)) = range.strip_prefix("bytes=").and_then(|r| r.split_once('-')) {
            if let Ok(start) = start.parse::<u64>() {
                let end = end.parse::<u64>().unwrap_or(start + RANGE_CHUNK - 1).min(start + RANGE_CHUNK - 1);
                upstream = upstream.header(header::RANGE, format!("bytes={start}-{end}"));
            }
        }
    }

    let Ok(response) = upstream.send().await else { return reply(502, vec![]) };
    let status = response.status().as_u16();
    let mut builder = Response::builder().status(status).header("access-control-allow-origin", "*").header("cross-origin-resource-policy", "cross-origin");
    for name in [header::CONTENT_TYPE, header::CONTENT_RANGE, header::ACCEPT_RANGES, header::ETAG, header::LAST_MODIFIED, header::CACHE_CONTROL] {
        if let Some(value) = response.headers().get(&name) {
            builder = builder.header(name, value);
        }
    }
    match response.bytes().await {
        Ok(bytes) => builder.body(bytes.to_vec()).unwrap(),
        Err(_) => reply(502, vec![]),
    }
}
