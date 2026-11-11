// The internet side. The rules: an honest User-Agent, the Referer
// each site's image server expects, only http(s), and never the request URL in an error (Rule34's holds the API key).
use crate::store::Store;
use reqwest::{header, Client, Url};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

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

/// How far apart a site's API requests go: the fastest each site allows (checked 11.11.2026). Danbooru doesn't
/// limit reads (10 a second is what others report), e621's hard limit is 2 a second, Rule34 allows 60 a minute per
/// key. The rest give no number: half a second, which has been fine.
fn gap(host: &str) -> Duration {
    if host == "danbooru.donmai.us" {
        Duration::from_millis(100)
    } else if host.ends_with("rule34.xxx") {
        Duration::from_secs(1)
    } else {
        Duration::from_millis(500)
    }
}

/// When a site says it is busy or that we ask too often (421 Moebooru, 429, 503), it gets nothing for this long.
const HOLD_BACK: Duration = Duration::from_secs(30);

// Each site's next free turn.
static NEXT: LazyLock<Mutex<HashMap<String, Instant>>> = LazyLock::new(Default::default);

/// The time of `host`'s next turn, taken: the one after it is `gap` later.
fn reserve(next: &mut HashMap<String, Instant>, host: &str, now: Instant) -> Instant {
    let at = next.get(host).map_or(now, |t| (*t).max(now));
    next.insert(host.to_string(), at + gap(host));
    at
}

/// Waits for the site's turn. Every API request comes through here, so the features together stay under the limit.
/// Pictures don't: they are a few at once (media.rs, downloads.rs).
pub async fn turn(url: &str) {
    let Some(host) = web_url(url).and_then(|u| u.host_str().map(String::from)) else { return };
    let at = reserve(&mut NEXT.lock().unwrap(), &host, Instant::now());
    tokio::time::sleep(at.saturating_duration_since(Instant::now())).await;
}

fn hold_back(next: &mut HashMap<String, Instant>, host: &str, now: Instant) {
    let until = now + HOLD_BACK;
    next.entry(host.to_string()).and_modify(|t| *t = (*t).max(until)).or_insert(until);
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
    // e621's login as basic auth: reqwest drops the header on a redirect to another host, unlike a key in the address.
    if url.host_str() == Some("e621.net") {
        if let (Some(login), Some(key)) = (store.string("e621Login"), store.string("e621ApiKey")) {
            request = request.basic_auth(login, Some(key));
        }
    }
    let response = request.send().await.map_err(describe)?;
    let status = response.status();
    if !status.is_success() {
        if matches!(status.as_u16(), 421 | 429 | 503) {
            if let Some(host) = url.host_str() {
                hold_back(&mut NEXT.lock().unwrap(), host, Instant::now());
            }
        }
        return Err(format!("HTTP {}", status.as_u16()));
    }
    Ok(response)
}

/// An API request, in the site's turn.
pub async fn get_text(client: &Client, store: &Store, url: &str) -> Result<String, String> {
    turn(url).await;
    get(client, store, url, Duration::from_secs(30)).await?.text().await.map_err(describe)
}

pub async fn get_bytes(client: &Client, store: &Store, url: &str) -> Result<Vec<u8>, String> {
    get(client, store, url, Duration::from_secs(60)).await?.bytes().await.map(|b| b.to_vec()).map_err(describe)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turns_are_a_gap_apart_per_site() {
        let (mut next, now) = (HashMap::new(), Instant::now());
        let half = Duration::from_millis(500);
        assert_eq!(reserve(&mut next, "e621.net", now), now);
        assert_eq!(reserve(&mut next, "e621.net", now), now + half);
        assert_eq!(reserve(&mut next, "e621.net", now + Duration::from_secs(5)), now + Duration::from_secs(5));
        // Another site doesn't wait for e621.
        assert_eq!(reserve(&mut next, "api.rule34.xxx", now), now);
        assert_eq!(reserve(&mut next, "api.rule34.xxx", now), now + Duration::from_secs(1));
        assert_eq!(reserve(&mut next, "danbooru.donmai.us", now), now);
        assert_eq!(reserve(&mut next, "danbooru.donmai.us", now), now + Duration::from_millis(100));
        // A "too many" answer holds the site back, and a hold doesn't shorten a later turn.
        hold_back(&mut next, "yande.re", now);
        assert_eq!(reserve(&mut next, "yande.re", now), now + HOLD_BACK);
        hold_back(&mut next, "yande.re", now);
        assert_eq!(reserve(&mut next, "yande.re", now), now + HOLD_BACK + half);
    }
}
