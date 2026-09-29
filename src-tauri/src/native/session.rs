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
#[derive(Clone)]
pub struct Engine {
    pub rt: tokio::runtime::Handle,
    pub client: Client,
    pub store: Arc<Store>,
    pub ctx: eframe::egui::Context,
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
    cfg: Config,
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
            engine.ctx.request_repaint();
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
}
