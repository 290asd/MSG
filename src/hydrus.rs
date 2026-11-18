// Hydrus Network through its Client API (a client on your own server or this computer): a site to search, fast
// because nothing goes to the boorus, and a place that keeps the favorites from every site with their tags.
// The access key goes in a header (net::get adds it), never in an address. Hydrus is your own server, so its
// requests don't wait for a turn (net::turn).
use super::net;
use super::session::{Engine, Msg};
use super::sites::{Config, Cursor};
use super::slide::{self, MediaType, Slide};
use crate::store::Store;
use serde_json::{json, Map, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::Duration;

pub const KEY_HEADER: &str = "Hydrus-Client-API-Access-Key";
/// The service keys Hydrus gives its own "my tags" and "all known tags" in every client.
const MY_TAGS: &str = "6c6f63616c2074616773";
const ALL_KNOWN_TAGS: &str = "616c6c206b6e6f776e2074616773";
/// The like/dislike rating every client has, "favourites": a favorite is liked there.
const FAVOURITES: &str = "6661766f757269746573";
/// The favorite mark where "favourites" has been removed.
const FAVORITE_TAG: &str = "msg:favorite";
const PAGE: usize = 100;

/// Booru tag categories and the Hydrus namespaces they become (general has none).
const NAMESPACES: [(&str, &str); 7] = [
    ("artist", "creator"), ("copyright", "series"), ("character", "character"), ("species", "species"),
    ("meta", "meta"), ("lore", "lore"), ("contributor", "contributor"),
];

/// The server's address without the slash at the end, when one is set.
pub fn base(store: &Store) -> Option<String> {
    store.string("hydrusUrl").map(|u| u.trim().trim_end_matches('/').to_string()).filter(|u| net::web_url(u).is_some())
}

/// The access key, for an address on the Hydrus server only.
pub fn key_for(store: &Store, url: &str) -> Option<String> {
    let base = base(store)?;
    (url == base || url.starts_with(&format!("{base}/"))).then(|| store.string("hydrusKey")).flatten()
}

fn enc(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

async fn get_json(engine: &Engine, url: &str) -> Result<Value, String> {
    let response = net::get(&engine.client, &engine.store, url, Duration::from_secs(60)).await.map_err(explain)?;
    let bytes = response.bytes().await.map_err(|_| "Hydrus didn't answer".to_string())?;
    serde_json::from_slice(&bytes).map_err(|_| "Hydrus sent something that isn't JSON".to_string())
}

async fn post_json(engine: &Engine, path: &str, body: Value) -> Result<Value, String> {
    let base = base(&engine.store).ok_or("no address")?;
    let key = engine.store.string("hydrusKey").unwrap_or_default();
    let response = engine.client.post(format!("{base}{path}")).header(KEY_HEADER, key).header(reqwest::header::CONTENT_TYPE, "application/json").body(body.to_string()).timeout(Duration::from_secs(60)).send().await.map_err(|_| "Hydrus didn't answer".to_string())?;
    answer(response).await
}

async fn answer(response: reqwest::Response) -> Result<Value, String> {
    let status = response.status().as_u16();
    if !response.status().is_success() {
        return Err(explain(format!("HTTP {status}")));
    }
    let bytes = response.bytes().await.map_err(|_| "Hydrus didn't answer".to_string())?;
    Ok(serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

fn explain(e: String) -> String {
    match e.as_str() {
        "HTTP 401" | "HTTP 403" | "HTTP 419" => "the access key wasn't accepted or lacks a permission (Settings → Sites & accounts → Test)".into(),
        "connection failed" | "timeout" => "the server didn't answer (is the Client API on?)".into(),
        _ => e,
    }
}

/// Tests the address and key: Ok with what the key may do, or what is wrong. Needs permissions 0–3.
pub async fn test(engine: &Engine) -> String {
    let Some(base) = base(&engine.store) else { return "Hydrus: write the server's address, like http://192.168.1.10:45869.".into() };
    match get_json(engine, &format!("{base}/verify_access_key")).await {
        Ok(v) => {
            let has: Vec<u64> = v["basic_permissions"].as_array().into_iter().flatten().filter_map(Value::as_u64).collect();
            let missing: Vec<&str> = [(0, "import URLs"), (1, "import files"), (2, "edit tags"), (3, "search files"), (9, "edit ratings")]
                .iter()
                .filter(|(n, _)| !has.contains(n) && v["permits_everything"] != true)
                .map(|(_, name)| *name)
                .collect();
            if missing.is_empty() { "Hydrus: connected.".into() } else { format!("Hydrus: connected, but the key can't {}.", missing.join(", ")) }
        }
        Err(e) => format!("Hydrus: {e}"),
    }
}

/// The Test button: what `test` found, as a notice.
pub fn check(engine: &Engine, tx: &Sender<Msg>) {
    let (engine, tx) = (engine.clone(), tx.clone());
    engine.rt.clone().spawn(async move {
        let _ = tx.send(Msg::Notice(test(&engine).await));
        engine.wake();
    });
}

/// A booru word as Hydrus tags: spaces for underscores, lower case.
fn hydrus_tag(word: &str) -> String {
    word.replace('_', " ").to_lowercase()
}

/// The search as Hydrus predicates. A plain word matches its tag in any namespace (creator:, character: …), as
/// Hydrus's own tag list finds them; a word with a colon (creator:x, system:inbox) goes as it is. Sort words and the
/// blacklist are handled here too. `tags_like` gives the tags Hydrus knows for a word.
async fn predicates(engine: &Engine, base: &str, text: &str, cfg: &Config) -> Vec<Value> {
    let mut out = vec![];
    let words = text.split_whitespace().filter(|w| !w.contains("order:") && !w.contains("sort:") && *w != "*");
    let blacklist = cfg.blacklist.iter().map(|t| format!("-{t}"));
    for word in words.map(String::from).chain(blacklist) {
        let (negated, word) = match word.strip_prefix('-') {
            Some(w) => (true, w.to_string()),
            None => (false, word),
        };
        let tag = hydrus_tag(&word);
        let tags = match tag.as_str() {
            // MSG's favorites (liked on "favourites"); other system: words go as they are, underscores for spaces.
            "system:favorites" | "system:favourites" => vec!["system:rating for favourites = like".to_string()],
            _ if tag.contains(':') => vec![tag],
            _ => tags_like(engine, base, &tag).await,
        };
        if negated {
            out.extend(tags.into_iter().map(|t| json!(format!("-{t}"))));
        } else {
            out.push(if tags.len() == 1 { json!(tags[0]) } else { json!(tags) });
        }
    }
    if out.iter().all(|p| p.as_str().is_some_and(|s| s.starts_with('-'))) {
        out.insert(0, json!("system:everything"));
    }
    out
}

/// `tag` and the namespaced tags Hydrus has with that same name.
async fn tags_like(engine: &Engine, base: &str, tag: &str) -> Vec<String> {
    let url = format!("{base}/add_tags/search_tags?search={}&tag_display_type=display", enc(tag));
    let found = get_json(engine, &url).await.unwrap_or(Value::Null);
    let mut tags: Vec<String> = found["tags"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|t| t["value"].as_str())
        .filter(|t| t.rsplit_once(':').map_or(*t, |(_, sub)| sub) == tag)
        .map(String::from)
        .collect();
    if !tags.iter().any(|t| t == tag) {
        tags.insert(0, tag.to_string());
    }
    tags
}

/// Hydrus's sort for MSG's sort words: newest import first, unless asked for the oldest or random.
fn sort(text: &str) -> (u32, bool) {
    let text = text.to_lowercase();
    if text.contains("order:random") || text.contains("sort:random") {
        (4, false)
    } else if ["order:id_asc", "sort:id_asc", "order:id ", "sort:id "].iter().any(|w| format!("{text} ").contains(w)) {
        (2, true)
    } else {
        (2, false)
    }
}

/// A page of a Hydrus search. The first asks for the matching file ids (all of them, Hydrus is quick); each page
/// then reads the details of the next hundred.
pub async fn fetch_page(engine: &Engine, cfg: &Config, mut cursor: Cursor, text: &str) -> (Cursor, Vec<Slide>, Option<String>) {
    let warn = |e: String| Some(format!("Hydrus: {e}"));
    let Some(base) = base(&engine.store) else {
        cursor.exhausted = true;
        return (cursor, vec![], warn("add the server's address and access key in Settings → Sites & accounts".into()));
    };
    if cursor.ids.is_none() {
        let tags = predicates(engine, &base, text, cfg).await;
        let (sort, ascending) = sort(text);
        let url = format!("{base}/get_files/search_files?tags={}&file_sort_type={sort}&file_sort_asc={ascending}", enc(&json!(tags).to_string()));
        match get_json(engine, &url).await {
            Ok(v) => cursor.ids = Some(v["file_ids"].as_array().into_iter().flatten().filter_map(Value::as_u64).collect()),
            Err(e) => {
                cursor.exhausted = true;
                return (cursor, vec![], warn(e));
            }
        }
    }
    let ids: Vec<u64> = cursor.ids.as_deref().unwrap_or_default().iter().skip(cursor.page * PAGE).take(PAGE).copied().collect();
    cursor.page += 1;
    cursor.exhausted = cursor.page * PAGE >= cursor.ids.as_ref().map_or(0, Vec::len);
    if ids.is_empty() {
        return (cursor, vec![], None);
    }
    let url = format!("{base}/get_files/file_metadata?file_ids={}&include_services_object=false", enc(&json!(ids).to_string()));
    match get_json(engine, &url).await {
        Ok(v) => {
            let mut slides: Vec<Slide> = v["metadata"].as_array().into_iter().flatten().filter_map(|m| slide_of(cfg, &base, m)).collect();
            // In the order the search gave.
            slides.sort_by_key(|s| ids.iter().position(|id| s.id.as_u64() == Some(*id)));
            (cursor, slides, None)
        }
        Err(e) => {
            cursor.exhausted = true;
            (cursor, vec![], warn(e))
        }
    }
}

/// A post page among a file's URLs (the boorus' own address for it), before any file address.
fn is_post_page(url: &str) -> bool {
    ["/posts/", "/post/show/", "page=post", "/images/"].iter().any(|p| url.contains(p))
}

fn slide_of(cfg: &Config, base: &str, m: &Value) -> Option<Slide> {
    let id = m["file_id"].as_u64()?;
    let mime = m["mime"].as_str().unwrap_or("");
    let kind = if mime.starts_with("video/") {
        MediaType::Video
    } else if mime == "image/gif" {
        MediaType::Gif
    } else if mime.starts_with("image/") {
        MediaType::Image
    } else {
        return None;
    };
    let all: Vec<&str> = m["tags"][ALL_KNOWN_TAGS]["display_tags"]["0"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    let (tags, groups, rating) = booru_tags(&all);
    let file_url = format!("{base}/get_files/file?file_id={id}");
    // The filters as on the sites (the kind of file goes by the name's end), by the rating tag Hydrus's downloaders give.
    let ext = match kind {
        MediaType::Video => "webm",
        MediaType::Gif => "gif",
        _ => "png",
    };
    cfg.accepts(&format!("x.{ext}"), rating.as_deref(), &tags)?;
    let urls: Vec<String> = m["known_urls"].as_array().into_iter().flatten().filter_map(Value::as_str).filter(|u| net::web_url(u).is_some()).map(String::from).collect();
    let post = urls.iter().find(|u| is_post_page(u)).cloned().unwrap_or_default();
    let imported = m["file_services"]["current"].as_object().into_iter().flatten().filter_map(|(_, s)| s["time_imported"].as_i64()).min();
    let seconds = imported.or_else(|| m["time_modified"].as_i64()).unwrap_or(0);
    Some(Slide {
        site_id: "HYDR".into(),
        id: json!(id),
        preview_file_url: format!("{base}/get_files/thumbnail?file_id={id}"),
        viewable_website_post_url: post.clone(),
        width: m["width"].as_i64().unwrap_or(0),
        height: m["height"].as_i64().unwrap_or(0),
        date: slide::iso_date(&seconds.to_string()),
        media_type: kind,
        md5: m["hash"].as_str().unwrap_or("").into(),
        tags,
        tag_groups: Some(groups),
        file_url,
        sources: urls.into_iter().filter(|u| *u != post && !u.contains("/data/")).collect(),
        ..Default::default()
    })
}

/// Hydrus tags as MSG's: underscores for spaces, the namespaces as categories (creator: → artist), the others kept
/// in the tag (page:3). Also the rating, from a rating: tag.
fn booru_tags(all: &[&str]) -> (String, Map<String, Value>, Option<String>) {
    let (mut words, mut groups, mut rating) = (vec![], Map::new(), None);
    for tag in all {
        let (category, word) = match tag.split_once(':') {
            Some(("rating", r)) => {
                rating = Some(r.to_string());
                continue;
            }
            Some((ns, sub)) => match NAMESPACES.iter().find(|(_, n)| *n == ns) {
                Some((category, _)) => (*category, sub.replace(' ', "_")),
                None => ("general", tag.replace(' ', "_")),
            },
            None => ("general", tag.replace(' ', "_")),
        };
        if word == FAVORITE_TAG {
            continue;
        }
        let list = groups.entry(category).or_insert_with(|| json!([]));
        if let Some(list) = list.as_array_mut() {
            list.push(json!(word));
        }
        words.push(word);
    }
    (words.join(" "), groups, rating)
}

/// A post's tags as Hydrus wants them: spaces, and namespaces for the categories.
fn hydrus_tags(slide: &Slide) -> Vec<String> {
    slide::tag_groups(slide)
        .into_iter()
        .flat_map(|(category, words)| {
            let ns = NAMESPACES.iter().find(|(c, _)| *c == category).map(|(_, n)| format!("{n}:")).unwrap_or_default();
            words.into_iter().map(move |w| format!("{ns}{}", hydrus_tag(&w)))
        })
        .collect()
}

/// The hash of the file Hydrus already has for this post, by the post's address.
async fn known_hash(engine: &Engine, base: &str, slide: &Slide) -> Option<String> {
    if slide.site_id == "HYDR" {
        return Some(slide.md5.clone()).filter(|h| !h.is_empty());
    }
    if slide.viewable_website_post_url.is_empty() {
        return None;
    }
    let url = format!("{base}/add_urls/get_url_files?url={}", enc(&slide.viewable_website_post_url));
    let v = get_json(engine, &url).await.ok()?;
    v["url_file_statuses"].as_array()?.iter().find(|s| s["status"] == 2).and_then(|s| s["hash"].as_str()).map(String::from)
}

/// Marks the file liked (or not) on Hydrus's own "favourites" rating; where that has been removed, with the tag
/// msg:favorite instead.
async fn mark(engine: &Engine, hash: &str, faved: bool) -> Result<(), String> {
    let rating = post_json(engine, "/edit_ratings/set_rating", json!({ "hash": hash, "rating_service_key": FAVOURITES, "rating": if faved { json!(true) } else { Value::Null } })).await;
    if rating.is_ok() {
        return Ok(());
    }
    let action = if faved { "0" } else { "1" };
    post_json(engine, "/add_tags/add_tags", json!({ "hash": hash, "service_keys_to_actions_to_tags": { MY_TAGS: { action: [FAVORITE_TAG] } } })).await.map(|_| ())
}

/// Keeps a favorite in Hydrus: the file (sent unless Hydrus has it already), its tags and the post's addresses, and
/// marks it liked on "favourites". Unfaving only takes the like away; the file stays.
pub async fn keep(engine: &Engine, slide: &Slide, faved: bool) -> Result<(), String> {
    let base = base(&engine.store).ok_or("no address")?;
    let hash = match known_hash(engine, &base, slide).await {
        Some(hash) => hash,
        None if !faved => return Ok(()),
        None => upload(engine, &base, slide).await?,
    };
    mark(engine, &hash, faved).await?;
    if !faved || slide.site_id == "HYDR" {
        return Ok(());
    }
    post_json(engine, "/add_tags/add_tags", json!({ "hash": hash, "service_keys_to_tags": { MY_TAGS: hydrus_tags(slide) } })).await?;
    let urls: Vec<&String> = std::iter::once(&slide.viewable_website_post_url).chain(&slide.sources).filter(|u| net::web_url(u).is_some()).collect();
    if !urls.is_empty() {
        post_json(engine, "/add_urls/associate_url", json!({ "hash": hash, "urls_to_add": urls })).await?;
    }
    Ok(())
}

async fn upload(engine: &Engine, base: &str, slide: &Slide) -> Result<String, String> {
    let bytes = net::get_bytes(&engine.client, &engine.store, &slide.file_url).await.map_err(|e| format!("couldn't get the file ({e})"))?;
    let key = engine.store.string("hydrusKey").unwrap_or_default();
    let response = engine.client.post(format!("{base}/add_files/add_file")).header(KEY_HEADER, key).header(reqwest::header::CONTENT_TYPE, "application/octet-stream").body(bytes).timeout(Duration::from_secs(300)).send().await.map_err(|_| "Hydrus didn't answer".to_string())?;
    let v = answer(response).await?;
    match (v["status"].as_u64(), v["hash"].as_str()) {
        (Some(1 | 2), Some(hash)) => Ok(hash.to_string()),
        (Some(3), _) => Err("Hydrus has deleted this file before and won't take it again".into()),
        _ => Err("Hydrus didn't take the file".into()),
    }
}

/// After faving or unfaving in MSG, the same in Hydrus (when it is set up and `hydrusFavorites` isn't off).
/// Only failures are told.
pub fn sync(engine: &Engine, tx: &Sender<Msg>, slide: &Slide, faved: bool) {
    if base(&engine.store).is_none() || engine.store.get("hydrusFavorites").and_then(|v| v.as_bool()) == Some(false) {
        return;
    }
    let (engine, tx, slide) = (engine.clone(), tx.clone(), slide.clone());
    engine.rt.clone().spawn(async move {
        if let Err(e) = keep(&engine, &slide, faved).await {
            let _ = tx.send(Msg::Notice(format!("Hydrus: couldn't {} it: {e}", if faved { "keep" } else { "unfave" })));
            engine.wake();
        }
    });
}

/// Set to end `send_all` early.
pub static STOP: AtomicBool = AtomicBool::new(false);

/// Sends every favorite to Hydrus, one at a time, telling how far it is. `STOP` ends it early.
pub fn send_all(engine: &Engine, tx: &Sender<Msg>, slides: Vec<Slide>) {
    let (engine, tx, stop) = (engine.clone(), tx.clone(), &STOP);
    stop.store(false, Ordering::SeqCst);
    engine.rt.clone().spawn(async move {
        let say = |text: String| {
            let _ = tx.send(Msg::Notice(text));
            engine.wake();
        };
        let (mut done, mut failed) = (0, 0);
        for (n, slide) in slides.iter().enumerate() {
            if stop.load(Ordering::SeqCst) {
                break;
            }
            match keep(&engine, slide, true).await {
                Ok(()) => done += 1,
                Err(_) => failed += 1,
            }
            if (n + 1) % 10 == 0 {
                say(format!("Sending favorites to Hydrus: {} / {}", n + 1, slides.len()));
            }
        }
        let stopped = if stop.load(Ordering::SeqCst) { " (stopped)" } else { "" };
        say(format!("Hydrus has {done} of {} favorites, {failed} failed{stopped}.", slides.len()));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hydrus_tags_and_back() {
        let mut groups = Map::new();
        groups.insert("artist".into(), json!(["some_artist"]));
        groups.insert("general".into(), json!(["red_fox", "snow"]));
        let slide = Slide { tags: "some_artist red_fox snow".into(), tag_groups: Some(groups), ..Default::default() };
        let tags = hydrus_tags(&slide);
        assert_eq!(tags, ["creator:some artist", "red fox", "snow"]);
        let back: Vec<&str> = tags.iter().map(String::as_str).chain(["page:3", "rating:explicit", FAVORITE_TAG]).collect();
        let (words, groups, rating) = booru_tags(&back);
        assert_eq!(words, "some_artist red_fox snow page:3");
        assert_eq!(groups["artist"], json!(["some_artist"]));
        assert_eq!(rating.as_deref(), Some("explicit"));
    }

    #[test]
    fn metadata_becomes_a_slide() {
        let m = json!({"file_id": 7, "hash": "ab", "mime": "image/png", "width": 3, "height": 4,
            "known_urls": ["https://static1.e621.net/data/x.png", "https://e621.net/posts/5", "https://bsky.app/a"],
            "file_services": {"current": {"k": {"time_imported": 1700000000}}},
            "tags": {ALL_KNOWN_TAGS: {"display_tags": {"0": ["creator:a b", "cat", "rating:safe"]}}}});
        let cfg = Config { images: true, gifs: true, videos: true, ..Default::default() };
        let s = slide_of(&cfg, "http://h:45869", &m).unwrap();
        assert_eq!((s.file_url.as_str(), s.preview_file_url.as_str()), ("http://h:45869/get_files/file?file_id=7", "http://h:45869/get_files/thumbnail?file_id=7"));
        assert_eq!((s.viewable_website_post_url.as_str(), s.sources.as_slice()), ("https://e621.net/posts/5", &["https://bsky.app/a".to_string()][..]));
        assert_eq!((s.tags.as_str(), s.media_type, s.md5.as_str()), ("a_b cat", MediaType::Image, "ab"));
        assert!(s.date.starts_with("2023-11-14"));
        let explicit_only = Config { explicit: true, ..cfg };
        assert!(slide_of(&explicit_only, "http://h:45869", &m).is_none());
    }

    #[test]
    fn sort_words() {
        assert_eq!(sort("cat"), (2, false));
        assert_eq!(sort("cat order:random"), (4, false));
        assert_eq!(sort("cat order:id_asc"), (2, true));
    }
}
