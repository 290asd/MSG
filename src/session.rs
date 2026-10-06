// A search across the ticked sites: reads a page from each, filters, drops duplicates, sorts what came in
// and keeps the list. Ported from js/objects/sites_manager.js. The window asks for more pages as the
// slideshow nears the end of what is loaded.
use super::files;
use super::net;
use super::sites::{self, Config, Cursor, Pool, Site};
use super::slide::Slide;
use crate::store::Store;
use regex::RegexBuilder;
use reqwest::Client;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::{mpsc::Sender, Arc};

/// Shared by everything that talks to the network: the runtime the work runs on, the client and the settings.
/// `wake` tells the window (or the terminal) that something arrived; the shared code knows nothing of egui.
#[derive(Clone)]
pub struct Engine {
    pub rt: tokio::runtime::Handle,
    pub client: Client,
    pub store: Arc<Store>,
    pub wake: Arc<dyn Fn() + Send + Sync>,
}

impl Engine {
    pub fn wake(&self) {
        (self.wake)()
    }
}

pub fn config(store: &Store) -> Config {
    let flag = |key: &str, default: bool| store.get(key).and_then(|v| v.as_bool()).unwrap_or(default);
    let text = |key: &str| store.string(key).unwrap_or_default();
    Config {
        images: flag("includeImages", true),
        gifs: flag("includeGifs", true),
        videos: flag("includeWebms", true),
        explicit: flag("includeExplicit", false),
        questionable: flag("includeQuestionable", false),
        safe: flag("includeSafe", true),
        blacklist: text("blacklist").split_whitespace().map(String::from).collect(),
        derpibooru_key: text("derpibooruApiKey"),
        tantabus_key: text("tantabusApiKey"),
        e621_login: text("e621Login"),
        e621_key: text("e621ApiKey"),
        gelbooru_user: text("gelbUserId"),
        gelbooru_key: text("gelbApiKey"),
        rule34_user: text("rule34UserId"),
        rule34_key: text("rule34ApiKey"),
    }
}

/// The sites ticked in `sitesToSearch` (Safebooru when nothing is saved, as in the web version).
pub fn ticked_sites(store: &Store) -> Vec<Site> {
    match store.get("sitesToSearch") {
        Some(Value::Object(map)) => Site::ALL.into_iter().filter(|s| map.get(s.id()).and_then(Value::as_bool) == Some(true)).collect(),
        _ => vec![Site::Safebooru],
    }
}

/// What background work tells the window.
pub enum Msg {
    Batch(Batch),
    /// A short line for the user (errors, results).
    Notice(String),
    /// A favorites import, one site at a time: a progress line and, at the end, the slides found.
    Progress(String),
    Imported { site: String, slides: Vec<Slide> },
    /// A background job changed the settings file (offline copies): reread what depends on it.
    Downloaded(String),
    /// The first result of a quick search, for its card on the front page.
    Cover { index: usize, slide: Slide },
    /// Find pools in favorites: the pools that weren't saved yet.
    Pools(Vec<Value>),
}

pub struct Batch {
    pub generation: u64,
    pub cursors: Vec<Cursor>,
    pub slides: Vec<Slide>,
    pub warnings: Vec<String>,
}

pub struct Search {
    pub generation: u64,
    pub text: String,
    pub slides: Vec<Slide>,
    pub loading: bool,
    // Held here while idle; moved into the task while a page is being read.
    cursors: Vec<Cursor>,
    pub cfg: Config,
    seen: HashSet<String>,
    include_dupes: bool,
}

impl Search {
    pub fn new(generation: u64, text: &str, sites: Vec<Site>, store: &Store) -> Search {
        // A pool is read from e621 only, whatever is ticked.
        let sites = if sites::pool_id(text).is_some() { vec![Site::E621] } else { sites };
        Search {
            generation,
            text: text.trim().to_string(),
            slides: vec![],
            loading: false,
            cursors: sites.into_iter().map(Cursor::new).collect(),
            cfg: config(store),
            seen: HashSet::new(),
            include_dupes: store.flag("includeDupes"),
        }
    }

    /// (id, name, page count) of the e621 pool being read.
    pub fn pool(&self) -> Option<(u64, String, usize)> {
        self.cursors.iter().find_map(|c| c.pool.as_ref()).map(|p| (p.id, p.name.clone(), p.post_ids.len()))
    }

    pub fn has_more(&self) -> bool {
        self.loading || self.cursors.iter().any(|c| !c.exhausted)
    }

    /// Reads the next page of every site that still has results; the answer arrives as a `Batch`.
    pub fn load_more(&mut self, engine: &Engine, tx: Sender<Msg>) {
        if self.loading || !self.cursors.iter().any(|c| !c.exhausted) {
            return;
        }
        self.loading = true;
        let cursors = std::mem::take(&mut self.cursors);
        let (cfg, text, generation, engine) = (self.cfg.clone(), self.text.clone(), self.generation, engine.clone());
        engine.rt.clone().spawn(async move {
            let tasks: Vec<_> = cursors
                .into_iter()
                .map(|cursor| {
                    let (engine, cfg, text) = (engine.clone(), cfg.clone(), text.clone());
                    tokio::spawn(async move { fetch_page(&engine, &cfg, cursor, &text).await })
                })
                .collect();
            let (mut cursors, mut slides, mut warnings) = (vec![], vec![], vec![]);
            for task in tasks {
                if let Ok((cursor, mut got, warning)) = task.await {
                    cursors.push(cursor);
                    slides.append(&mut got);
                    warnings.extend(warning);
                }
            }
            let _ = tx.send(Msg::Batch(Batch { generation, cursors, slides, warnings }));
            engine.wake();
        });
    }

    /// Takes in a page's worth: dupes out (by md5), the rest sorted and appended. Returns how many were added.
    pub fn accept(&mut self, batch: Batch) -> usize {
        self.loading = false;
        self.cursors = batch.cursors;
        let mut fresh: Vec<Slide> = batch
            .slides
            .into_iter()
            .filter(|s| self.include_dupes || s.md5.is_empty() || self.seen.insert(s.md5.clone()))
            .collect();
        // A pool keeps its own order.
        if sites::pool_id(&self.text).is_none() {
            sort_slides(&mut fresh, sort_method(&self.text));
        }
        let added = fresh.len();
        self.slides.append(&mut fresh);
        added
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Sort {
    NewestFirst,
    OldestFirst,
    ScoreDesc,
    ScoreAsc,
}

pub fn sort_method(text: &str) -> Sort {
    const TERMS: [(&str, Sort); 4] = [
        (r"(?:order|sort):(?:id|id_asc)\b", Sort::OldestFirst),
        (r"(?:order|sort):id_desc\b", Sort::NewestFirst),
        (r"(?:order|sort):(?:score|score_desc)\b", Sort::ScoreDesc),
        (r"(?:order|sort):score_asc\b", Sort::ScoreAsc),
    ];
    for (pattern, sort) in TERMS {
        if RegexBuilder::new(pattern).case_insensitive(true).build().unwrap().is_match(text) {
            return sort;
        }
    }
    Sort::NewestFirst
}

pub fn sort_slides(slides: &mut [Slide], sort: Sort) {
    match sort {
        Sort::NewestFirst => slides.sort_by_key(|s| std::cmp::Reverse(s.timestamp())),
        Sort::OldestFirst => slides.sort_by_key(|s| s.timestamp()),
        Sort::ScoreDesc => slides.sort_by_key(|s| std::cmp::Reverse(s.score)),
        Sort::ScoreAsc => slides.sort_by_key(|s| s.score),
    }
}

// One page from one site. A site that fails is finished for this search (its results are missing, the others still show).
async fn fetch_page(engine: &Engine, cfg: &Config, mut cursor: Cursor, text: &str) -> (Cursor, Vec<Slide>, Option<String>) {
    let site = cursor.site;
    let warn = |e: String| {
        let note = if e == "HTTP 429" { " (requests were made too quickly, try again)" } else { "" };
        format!("{}: {e}{note}", site.name())
    };
    if site == Site::Local {
        let (store, cfg, text) = (engine.store.clone(), cfg.clone(), text.to_string());
        let slides = tokio::task::spawn_blocking(move || files::local_slides(&store, &cfg, &text)).await.unwrap_or_default();
        cursor.exhausted = true;
        return (cursor, slides, None);
    }
    if site == Site::E621 && cursor.pool.is_none() {
        if let Some(id) = sites::pool_id(text) {
            match net::get_text(&engine.client, &engine.store, &sites::pool_url(cfg, id)).await {
                Ok(body) => match parse_pool(&body) {
                    Some(pool) => cursor.pool = Some(pool),
                    None => {
                        cursor.exhausted = true;
                        return (cursor, vec![], Some(warn("the pool could not be read".into())));
                    }
                },
                Err(e) => {
                    cursor.exhausted = true;
                    return (cursor, vec![], Some(warn(e)));
                }
            }
        }
    }
    let Some(url) = sites::request_url(cfg, &cursor, text) else {
        cursor.exhausted = true;
        return (cursor, vec![], None);
    };
    match net::get_text(&engine.client, &engine.store, &url).await.and_then(|body| sites::parse(cfg, site, &body)) {
        Ok((mut slides, count)) => {
            let limit = site.page_limit();
            cursor.page += 1;
            cursor.exhausted = match &cursor.pool {
                Some(pool) => cursor.page * limit >= pool.post_ids.len(),
                None => count < limit,
            };
            if let Some(pool) = &cursor.pool {
                let position = |s: &Slide| pool.post_ids.iter().position(|id| s.id == Value::from(*id)).unwrap_or(usize::MAX);
                slides.sort_by_key(position);
            }
            (cursor, slides, None)
        }
        Err(e) => {
            cursor.exhausted = true;
            (cursor, vec![], Some(warn(e)))
        }
    }
}

/// The first post the sites give for a search (a quick search's card).
pub async fn first_slide(engine: &Engine, sites: Vec<Site>, text: &str) -> Option<Slide> {
    let cfg = config(&engine.store);
    for site in sites.into_iter().filter(|s| *s != Site::Local) {
        let (_, slides, _) = fetch_page(engine, &cfg, Cursor::new(site), text).await;
        if let Some(slide) = slides.into_iter().find(|s| s.media_type != super::slide::MediaType::Video) {
            return Some(slide);
        }
    }
    None
}

/// Adds the pool to settings.json → savedPools (last, as the newest; once).
pub fn save_pool(store: &Arc<Store>, id: u64, name: &str, count: usize, cover: &str) {
    let mut pools: Vec<Value> = store.get("savedPools").and_then(|v| v.as_array().cloned()).unwrap_or_default();
    pools.retain(|p| p["id"].as_u64() != Some(id));
    pools.push(serde_json::json!({"id": id, "name": name, "count": count, "cover": cover}));
    store.data.lock().unwrap().insert("savedPools".into(), Value::Array(pools));
    store.save();
}

/// The saved pools in the order chosen (poolsSort): newest saved first, by name or most pages first.
pub fn saved_pools(store: &Store) -> Vec<Value> {
    let mut pools: Vec<Value> = store.get("savedPools").and_then(|v| v.as_array().cloned()).unwrap_or_default();
    match store.string("poolsSort").as_deref() {
        Some("name") => pools.sort_by_key(|p| p["name"].as_str().unwrap_or("").to_lowercase()),
        Some("pages") => pools.sort_by_key(|p| std::cmp::Reverse(p["count"].as_u64().unwrap_or(0))),
        _ => pools.reverse(),
    }
    pools
}

/// Find pools in favorites: the e621 pools that the e621 favorites (`favorites`, post ids) are in and that
/// aren't in `saved`. Read-only on e621; the window adds them (Msg::Pools).
pub fn find_pools(engine: &Engine, tx: Sender<Msg>, favorites: Vec<String>, saved: Vec<u64>) {
    if engine.store.flag("offlineMode") {
        let _ = tx.send(Msg::Notice("Offline mode is on.".into()));
        return;
    }
    if favorites.is_empty() {
        let _ = tx.send(Msg::Notice("There are no e621 favorites to look in.".into()));
        return;
    }
    // At once, so the button is off before the first answer.
    let _ = tx.send(Msg::Progress("Checking favorites…".into()));
    let engine = engine.clone();
    engine.rt.clone().spawn(async move {
        let text = match pools_of(&engine, &tx, &favorites, &saved).await {
            Ok((found, pools)) => {
                let text = format!("Checked {} e621 favorites: {found} pools, {} new.", favorites.len(), pools.len());
                let _ = tx.send(Msg::Pools(pools));
                text
            }
            Err(e) => format!("Finding pools failed: {e}"),
        };
        let _ = tx.send(Msg::Notice(text));
        let _ = tx.send(Msg::Progress(String::new()));
        engine.wake();
    });
}

/// The posts 100 at a time (their "pools"), then the new pools' names and first pages (the covers).
async fn pools_of(engine: &Engine, tx: &Sender<Msg>, favorites: &[String], saved: &[u64]) -> Result<(usize, Vec<Value>), String> {
    use super::downloads::e621_json;
    let say = |text: String| {
        let _ = tx.send(Msg::Progress(text));
        engine.wake();
    };
    let posts = |ids: &[String]| format!("/posts.json?limit=100&tags=id:{}", ids.join(","));
    let mut found = std::collections::BTreeSet::new();
    let mut checked = 0;
    for ids in favorites.chunks(100) {
        let data = e621_json(engine, &posts(ids)).await?;
        for post in data["posts"].as_array().into_iter().flatten() {
            found.extend(post["pools"].as_array().into_iter().flatten().filter_map(Value::as_u64));
        }
        checked += ids.len();
        say(format!("Checking favorites: {checked} / {} · {} pools", favorites.len(), found.len()));
    }
    let new: Vec<String> = found.iter().filter(|id| !saved.contains(id)).map(u64::to_string).collect();
    // (pool, its first page)
    let mut pools: Vec<(Value, Option<u64>)> = vec![];
    for ids in new.chunks(100) {
        say(format!("Getting pool names: {} / {}", pools.len(), new.len()));
        let data = e621_json(engine, &format!("/pools.json?limit=100&search%5Bid%5D={}", ids.join(","))).await?;
        for pool in data.as_array().into_iter().flatten() {
            let name = pool["name"].as_str().unwrap_or("pool").replace('_', " ");
            pools.push((serde_json::json!({"id": pool["id"], "name": name, "count": pool["post_count"], "cover": ""}), pool["post_ids"][0].as_u64()));
        }
    }
    let firsts: Vec<String> = pools.iter().filter_map(|(_, first)| first.map(|id| id.to_string())).collect();
    for ids in firsts.chunks(100) {
        say("Getting covers…".into());
        let data = e621_json(engine, &posts(ids)).await?;
        for post in data["posts"].as_array().into_iter().flatten() {
            if let Some(url) = post["preview"]["url"].as_str() {
                for (pool, _) in pools.iter_mut().filter(|(_, first)| *first == post["id"].as_u64()) {
                    pool["cover"] = Value::from(url);
                }
            }
        }
    }
    let mut pools: Vec<Value> = pools.into_iter().map(|(pool, _)| pool).collect();
    pools.sort_by_key(|p| p["name"].as_str().unwrap_or("").to_lowercase());
    Ok((found.len(), pools))
}

fn parse_pool(body: &str) -> Option<Pool> {
    let v: Value = serde_json::from_str(body).ok()?;
    Some(Pool {
        id: v.get("id")?.as_u64()?,
        name: v.get("name")?.as_str()?.replace('_', " "),
        post_ids: v.get("post_ids")?.as_array()?.iter().filter_map(Value::as_u64).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slide(md5: &str, date: &str, score: i64) -> Slide {
        Slide { md5: md5.into(), date: date.into(), score, ..Default::default() }
    }

    #[test]
    fn sort_terms() {
        assert_eq!(sort_method("cat order:score_asc"), Sort::ScoreAsc);
        assert_eq!(sort_method("cat sort:score"), Sort::ScoreDesc);
        assert_eq!(sort_method("cat ORDER:id"), Sort::OldestFirst);
        assert_eq!(sort_method("cat"), Sort::NewestFirst);
    }

    #[test]
    fn sorts_by_date_and_score() {
        let mut list = vec![slide("a", "2024-01-01T00:00:00Z", 1), slide("b", "2024-03-01T00:00:00Z", 9), slide("c", "2024-02-01T00:00:00Z", 5)];
        sort_slides(&mut list, Sort::NewestFirst);
        assert_eq!(list.iter().map(|s| s.md5.as_str()).collect::<String>(), "bca");
        sort_slides(&mut list, Sort::ScoreAsc);
        assert_eq!(list.iter().map(|s| s.md5.as_str()).collect::<String>(), "acb");
    }

    #[test]
    fn pool_json() {
        let pool = parse_pool(r#"{"id":5,"name":"a_b","post_ids":[3,1,2]}"#).unwrap();
        assert_eq!((pool.name.as_str(), pool.post_ids), ("a b", vec![3, 1, 2]));
    }

    #[test]
    fn saved_pools_order() {
        let store = Store::load(std::env::temp_dir().join("msg-native-pools"), std::env::temp_dir().join("msg-native-pools-legacy"));
        let pools = serde_json::json!([{"id": 1, "name": "b", "count": 9}, {"id": 2, "name": "A", "count": 5}, {"id": 3, "name": "c", "count": 7}]);
        store.data.lock().unwrap().insert("savedPools".into(), pools);
        let order = |sort: &str| {
            store.data.lock().unwrap().insert("poolsSort".into(), Value::from(sort));
            saved_pools(&store).iter().filter_map(|p| p["id"].as_u64()).collect::<Vec<_>>()
        };
        assert_eq!(order("added"), [3, 2, 1]);
        assert_eq!(order("name"), [2, 1, 3]);
        assert_eq!(order("pages"), [1, 3, 2]);
    }
}
