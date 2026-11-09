// The favorites list (settings.json → personalListItems): fave and unfave, the filter, random order,
// mirroring to e621 and the import from the sites. Ported from js/objects/personal_list.js,
// remote_favorites.js and favorites_importer.js.
use super::net;
use super::session::{Engine, Msg};
use super::sites::{self, Config, Cursor, Site};
use super::slide::Slide;
use crate::store::Store;
use rand::seq::SliceRandom;
use serde_json::{Map, Value};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::{mpsc::Sender, Arc};
use std::time::Duration;

pub struct Favorites {
    pub items: Vec<Slide>,
    keys: HashSet<(String, String)>,
    md5s: HashSet<String>,
    /// What the favorites page shows: the items after the site menu and the tag filter (and random order).
    pub view: Vec<Slide>,
    pub site: Option<String>,
    pub filter: String,
    pub random: bool,
    pub order: Order,
}

/// The order of the favorites when they aren't random: as saved, or by the post's date.
#[derive(Clone, Copy, PartialEq)]
pub enum Order {
    Saved,
    Newest,
    Oldest,
}

impl Favorites {
    pub fn load(store: &Store) -> Favorites {
        let items: Vec<Slide> = match store.get("personalListItems") {
            Some(Value::Array(list)) => list.into_iter().filter_map(|v| serde_json::from_value(v).ok()).collect(),
            _ => vec![],
        };
        let mut favorites = Favorites { items, keys: HashSet::new(), md5s: HashSet::new(), view: vec![], site: None, filter: String::new(), random: false, order: Order::Saved };
        favorites.reindex();
        favorites.rebuild(&|_| true);
        favorites
    }

    fn reindex(&mut self) {
        self.keys = self.items.iter().map(Slide::key).collect();
        self.md5s = self.items.iter().filter(|s| !s.md5.is_empty()).map(|s| s.md5.clone()).collect();
    }

    pub fn save(&self, store: &Arc<Store>) {
        let list: Vec<Value> = self.items.iter().filter_map(|s| serde_json::to_value(s).ok()).collect();
        store.data.lock().unwrap().insert("personalListItems".into(), Value::Array(list));
        store.save();
    }

    pub fn contains(&self, slide: &Slide) -> bool {
        self.keys.contains(&slide.key()) || (!slide.md5.is_empty() && self.md5s.contains(&slide.md5))
    }

    /// Faves the slide, or unfaves it when it already is one. Returns whether it is a favorite now.
    pub fn toggle(&mut self, slide: &Slide) -> bool {
        if self.contains(slide) {
            self.items.retain(|i| i.key() != slide.key() && (slide.md5.is_empty() || i.md5 != slide.md5));
            self.view.retain(|i| i.key() != slide.key());
            self.reindex();
            false
        } else {
            self.items.push(slide.clone());
            self.view.push(slide.clone());
            self.reindex();
            true
        }
    }

    /// Adds without duplicates; returns how many were new.
    pub fn add_many(&mut self, slides: Vec<Slide>) -> usize {
        let before = self.items.len();
        for slide in slides {
            if !self.contains(&slide) {
                self.items.push(slide);
                self.reindex();
            }
        }
        self.items.len() - before
    }

    /// Puts in the tag categories read from e621 (post id → categories).
    pub fn set_tag_groups(&mut self, found: HashMap<String, Map<String, Value>>) {
        for item in self.items.iter_mut().filter(|i| i.site_id == "E621") {
            if let Some(groups) = found.get(&super::slide::id_text(&item.id)) {
                item.tag_groups = Some(groups.clone());
            }
        }
    }

    /// (site id, how many) for the site menu.
    pub fn site_counts(&self) -> Vec<(String, usize)> {
        let mut counts: Vec<(String, usize)> = vec![];
        for item in &self.items {
            match counts.iter_mut().find(|(s, _)| *s == item.site_id) {
                Some(entry) => entry.1 += 1,
                None => counts.push((item.site_id.clone(), 1)),
            }
        }
        counts.sort();
        counts
    }

    /// Filters by the site menu and the tag filter; `keep` can drop more (offline: only downloaded ones).
    pub fn rebuild(&mut self, keep: &dyn Fn(&Slide) -> bool) {
        let (site, filter, random) = (self.site.clone(), self.filter.clone(), self.random);
        self.view = self.items.iter().filter(|s| site.as_ref().map_or(true, |x| *x == s.site_id) && keep(s) && matches_filter(&s.tags, &filter)).cloned().collect();
        if random {
            self.view.shuffle(&mut rand::thread_rng());
        } else if self.order == Order::Newest {
            self.view.sort_by_key(|s| std::cmp::Reverse(s.timestamp()));
        } else if self.order == Order::Oldest {
            self.view.sort_by_key(Slide::timestamp);
        }
    }
}

/// Words all have to be tags; `-word` must not be; `~word` at least one of those; `word*` a tag starting with it.
pub fn matches_filter(tags: &str, filter: &str) -> bool {
    let filter = filter.trim();
    if filter.is_empty() {
        return true;
    }
    if tags.is_empty() {
        return false;
    }
    let tags: Vec<String> = tags.split_whitespace().map(str::to_lowercase).collect();
    let has = |word: &str| match word.strip_suffix('*') {
        Some(prefix) => tags.iter().any(|t| t.starts_with(prefix)),
        None => tags.iter().any(|t| t == word),
    };
    let mut any_of: Vec<String> = vec![];
    for word in filter.split_whitespace().map(str::to_lowercase) {
        if let Some(word) = word.strip_prefix('-') {
            if has(word) {
                return false;
            }
        } else if let Some(word) = word.strip_prefix('~') {
            any_of.push(word.to_string());
        } else if !has(&word) {
            return false;
        }
    }
    any_of.is_empty() || any_of.iter().any(|w| has(w))
}

// e621 and Danbooru file these notices under "artist" too (and every *_warning).
const NOT_ARTISTS: [&str; 8] = ["conditional_dnp", "avoid_posting", "third-party_edit", "unknown_artist", "unknown_artist_signature", "anonymous_artist", "banned_artist", "creator_unknown"];

/// Whether a tag filed under "artist" is one.
pub fn is_artist(tag: &str) -> bool {
    !tag.ends_with("_warning") && !NOT_ARTISTS.contains(&tag)
}

/// The artists with the most favorites, most first (ties by name), as (tag, the sites the favorites came from): an
/// artist's name only means something on the sites it was seen on. Sites that don't say which tags are artists don't
/// count, and one favorite isn't enough.
pub fn top_artists(items: &[Slide], skip: &HashSet<String>, limit: usize) -> Vec<(String, Vec<String>)> {
    let mut counts: HashMap<&str, (usize, BTreeSet<&str>)> = HashMap::new();
    for item in items {
        let artists = item.tag_groups.as_ref().and_then(|g| g.get("artist")?.as_array());
        for artist in artists.into_iter().flatten().filter_map(Value::as_str) {
            if is_artist(artist) && !skip.contains(artist) {
                let (count, sites) = counts.entry(artist).or_default();
                *count += 1;
                sites.insert(&item.site_id);
            }
        }
    }
    let mut top: Vec<_> = counts.into_iter().filter(|(_, (count, _))| *count >= 2).collect();
    top.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.0.cmp(b.0)));
    top.truncate(limit);
    top.into_iter().map(|(artist, (_, sites))| (artist.to_string(), sites.into_iter().map(String::from).collect())).collect()
}

/// Faves or unfaves on e621 too, when the app does. Only failures are told.
pub fn sync_e621(engine: &Engine, tx: &Sender<Msg>, slide: &Slide, faved: bool) {
    let store = &engine.store;
    if slide.site_id != "E621" || store.get("syncE621Favorites").and_then(|v| v.as_bool()) == Some(false) {
        return;
    }
    let (Some(login), Some(key)) = (store.string("e621Login"), store.string("e621ApiKey")) else { return };
    let id = super::slide::id_text(&slide.id);
    let (client, tx, engine) = (engine.client.clone(), tx.clone(), engine.clone());
    engine.rt.clone().spawn(async move {
        let request = if faved {
            client.post("https://e621.net/favorites.json").form(&[("post_id", id.as_str())])
        } else {
            client.delete(format!("https://e621.net/favorites/{id}.json"))
        };
        let message = match request.basic_auth(login, Some(key)).timeout(Duration::from_secs(30)).send().await {
            // 422: already a favorite; 404: wasn't one. Both mean e621 is as wanted.
            Ok(r) if r.status().is_success() || r.status().as_u16() == 422 || (!faved && r.status().as_u16() == 404) => return,
            Ok(r) if matches!(r.status().as_u16(), 401 | 403) => format!("e621 didn't accept the username and API key ({}). Check them in Settings → Sites & accounts.", r.status().as_u16()),
            Ok(r) => format!("Couldn't {} on e621 (error {}).", if faved { "fave" } else { "unfave" }, r.status().as_u16()),
            Err(_) => format!("Couldn't reach e621 to {} the post.", if faved { "fave" } else { "unfave" }),
        };
        let _ = tx.send(Msg::Notice(message));
        engine.wake();
    });
}

/// Favorites ignore the search filters.
fn unfiltered(store: &Store) -> Config {
    let mut cfg = super::session::config(store);
    (cfg.images, cfg.gifs, cfg.videos) = (true, true, true);
    (cfg.explicit, cfg.questionable, cfg.safe) = (false, false, false);
    cfg.blacklist.clear();
    cfg
}

/// The old versions saved e621 favorites without tag categories, so their artists aren't known. Reads those from
/// e621, a page of ids at a time, once: a post the site no longer gives is kept with no categories. A failure ends it
/// quietly and the next start reads the rest.
pub fn fetch_tag_groups(engine: &Engine, tx: Sender<Msg>, items: &[Slide]) {
    let ids: Vec<String> = items.iter().filter(|s| s.site_id == "E621" && s.tag_groups.is_none()).map(|s| super::slide::id_text(&s.id)).collect();
    if ids.is_empty() {
        return;
    }
    let engine = engine.clone();
    engine.rt.clone().spawn(async move {
        let cfg = unfiltered(&engine.store);
        let mut found = HashMap::new();
        for chunk in ids.chunks(Site::E621.page_limit()) {
            let Some(url) = sites::request_url(&cfg, &Cursor::new(Site::E621), &format!("id:{}", chunk.join(","))) else { break };
            let Ok((slides, _)) = net::get_text(&engine.client, &engine.store, &url).await.and_then(|body| sites::parse(&cfg, Site::E621, &body)) else { break };
            found.extend(chunk.iter().map(|id| (id.clone(), Map::new())));
            found.extend(slides.into_iter().filter_map(|s| Some((super::slide::id_text(&s.id), s.tag_groups?))));
            // Stay under e621's rate limit, as the import does.
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        let _ = tx.send(Msg::TagGroups(found));
        engine.wake();
    });
}

/// Reads the favorites of every site that has login details in the settings, one site after another.
pub fn import(engine: &Engine, tx: Sender<Msg>) {
    let store = engine.store.clone();
    let mut sources: Vec<(Site, String)> = vec![];
    if let Some(login) = store.string("e621Login") {
        sources.push((Site::E621, format!("fav:{login}")));
    }
    if store.string("derpibooruApiKey").is_some() {
        sources.push((Site::Derpibooru, "my:faves".into()));
    }
    if let (Some(user), Some(_)) = (store.string("gelbUserId"), store.string("gelbApiKey")) {
        sources.push((Site::Gelbooru, format!("fav:{user}")));
    }
    if sources.is_empty() {
        let _ = tx.send(Msg::Notice("No site has login details saved. Add your e621 username, Derpibooru API key or Gelbooru user ID and API key in Settings → Sites & accounts.".into()));
        return;
    }
    let engine = engine.clone();
    engine.rt.clone().spawn(async move {
        let cfg = unfiltered(&store);
        for (site, query) in sources {
            let mut cursor = Cursor::new(site);
            let mut all: Vec<Slide> = vec![];
            while !cursor.exhausted {
                let _ = tx.send(Msg::Progress(format!("{}: loading page {} ({} favorites so far)", site.name(), cursor.page + 1, all.len())));
                engine.wake();
                let Some(url) = sites::request_url(&cfg, &cursor, &query) else { break };
                match net::get_text(&engine.client, &store, &url).await.and_then(|body| sites::parse(&cfg, site, &body)) {
                    Ok((mut slides, count)) => {
                        cursor.page += 1;
                        cursor.exhausted = count < site.page_limit();
                        all.append(&mut slides);
                    }
                    Err(e) => {
                        let _ = tx.send(Msg::Notice(format!("{} returned an error ({e}). Check your login details.", site.name())));
                        break;
                    }
                }
                // Stay under the sites' rate limits (e621 allows about 2 requests per second).
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            // Sites list favorites newest first; the list keeps the newest last.
            all.reverse();
            let _ = tx.send(Msg::Imported { site: site.name().into(), slides: all });
            engine.wake();
        }
        let _ = tx.send(Msg::Progress(String::new()));
        engine.wake();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_filter() {
        let tags = "cat blue_eyes solo";
        assert!(matches_filter(tags, ""));
        assert!(matches_filter(tags, "cat solo"));
        assert!(!matches_filter(tags, "cat dog"));
        assert!(!matches_filter(tags, "cat -solo"));
        assert!(matches_filter(tags, "-dog"));
        assert!(matches_filter(tags, "blue*"));
        assert!(!matches_filter(tags, "-blue*"));
        assert!(matches_filter(tags, "~dog ~cat"));
        assert!(!matches_filter(tags, "~dog ~bird"));
        assert!(!matches_filter("", "cat"));
    }

    #[test]
    fn top_artists_ranks_the_favorites() {
        let fav = |site: &str, artists: &[&str]| Slide { site_id: site.into(), tag_groups: serde_json::json!({ "artist": artists }).as_object().cloned(), ..Default::default() };
        let items = vec![
            fav("E621", &["b", "conditional_dnp"]),
            fav("E621", &["b", "conditional_dnp"]),
            fav("DANB", &["b"]),
            fav("E621", &["c", "sound_warning"]),
            fav("E621", &["c", "sound_warning"]),
            fav("E621", &["a"]),
            fav("E621", &["a"]),
            fav("E621", &["once"]),
            Slide { site_id: "RULE".into(), tags: "a a a".into(), ..Default::default() },
        ];
        let entry = |artist: &str, sites: &[&str]| (artist.to_string(), sites.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        let none = HashSet::new();
        assert_eq!(top_artists(&items, &none, 10), [entry("b", &["DANB", "E621"]), entry("a", &["E621"]), entry("c", &["E621"])]);
        assert_eq!(top_artists(&items, &none, 1), [entry("b", &["DANB", "E621"])]);
        assert!(top_artists(&items, &none, 0).is_empty());
        assert_eq!(top_artists(&items, &HashSet::from(["b".to_string()]), 1), [entry("a", &["E621"])]);
    }

    #[test]
    fn toggle_and_dupes() {
        let store = Store::load(std::env::temp_dir().join("msg-native-fav"), std::env::temp_dir().join("msg-native-fav-legacy"));
        let mut list = Favorites::load(&store);
        let slide = Slide { site_id: "E621".into(), id: Value::from(1), md5: "m".into(), ..Default::default() };
        assert!(list.toggle(&slide));
        assert!(list.contains(&Slide { site_id: "DANB".into(), id: Value::from(9), md5: "m".into(), ..Default::default() }));
        assert_eq!(list.add_many(vec![slide.clone()]), 0);
        assert!(!list.toggle(&slide));
        assert!(list.items.is_empty());
    }
}
