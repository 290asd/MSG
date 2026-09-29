// The internet side of the native window. Same rules as src/net.rs: an honest User-Agent, the Referer
// each site's image server expects, only http(s), and never the request URL in an error (Rule34's holds the API key).
use crate::store::Store;
use reqwest::{header, Client, Url};
use std::time::Duration;

// e621 asks for one, and Cloudflare (Danbooru) turns away a browser's User-Agent that doesn't come with a browser's TLS handshake.
pub const USER_AGENT: &str = concat!("MSG/", env!("CARGO_PKG_VERSION"), " (booru slideshow)");

pub fn client() -> Client {
    Client::builder().user_agent(USER_AGENT).connect_timeout(Duration::from_secs(15)).build().expect("http client")
}

pub fn web_url(url: &str) -> Option<Url> {
    Url::parse(url).ok().filter(|u| matches!(u.scheme(), "http" | "https"))
}

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

fn describe(e: reqwest::Error) -> String {
    if e.is_timeout() {
        "timeout".into()
    } else if e.is_connect() {
        "connection failed".into()
    } else {
        "network error".into()
    }
}

/// A GET with the site's Referer. Fails with "offline" in offline mode.
pub async fn get(client: &Client, store: &Store, url: &str, timeout: Duration) -> Result<reqwest::Response, String> {
    if store.flag("offlineMode") {
        return Err("offline".into());
    }
    let url = web_url(url).ok_or("Not a web address")?;
    let mut request = client.get(url.clone()).timeout(timeout);
    if let Some(referer) = referer_for(&url) {
        request = request.header(header::REFERER, referer);
    }
    let response = request.send().await.map_err(describe)?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("HTTP {}", status.as_u16()));
    }
    Ok(response)
}

pub async fn get_text(client: &Client, store: &Store, url: &str) -> Result<String, String> {
    get(client, store, url, Duration::from_secs(30)).await?.text().await.map_err(describe)
}

pub async fn get_bytes(client: &Client, store: &Store, url: &str) -> Result<Vec<u8>, String> {
    get(client, store, url, Duration::from_secs(60)).await?.bytes().await.map(|b| b.to_vec()).map_err(describe)
}
