// The window: search bar, the picture, the controls, the tags and the thumbnails. The state of the
// slideshow lives here; the settings and pools pages are in settings_ui.rs.
use super::downloads::{self, Downloader};
use super::favorites::{self, Favorites};
use super::files;
use super::glass;
use super::hotkeys::Hotkeys;
use super::theme::{self, icon, tok};
use super::media::{self, MediaCache, State};
use super::session::{self, Engine, Msg, Search};
use super::settings_ui::masonry;
use super::sites::Site;
use super::video::Player;
use super::slide::{MediaType, Slide};
use crate::store::Store;
use eframe::egui::{self, Color32, RichText, Sense, Vec2};
use raw_window_handle::HasWindowHandle;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How many of the next slides are loaded ahead (their thumbnails, and the first few pictures).
const AHEAD: usize = 12;
/// How long a removal from the favorites waits for the second press.
const UNFAVE_WAIT: Duration = Duration::from_secs(4);
/// How long (seconds) the player stays after the pointer stops.
const CHROME_SECS: f32 = 2.5;
/// The drawer of the slideshow: its width, the room it takes when pinned, and how near the window's edge opens it.
const DRAWER_W: f32 = 280.0;
const DRAWER_SPACE: f32 = DRAWER_W + RAIL_MARGIN * 2.0;
const DRAWER_EDGE: f32 = 10.0;
/// The width of the rail on the pages, its distance from the window's edge, and the room it takes.
const RAIL_W: f32 = 72.0;
const RAIL_MARGIN: f32 = 6.0;
const RAIL_SPACE: f32 = RAIL_W + RAIL_MARGIN * 2.0;
/// Where a page's content starts, below the toolbar floating over it.
pub const TOP_INSET: f32 = 60.0;
/// How long (seconds) the picture behind everything takes to change.
const AMBIENT_FADE: f32 = 0.45;

#[derive(Clone, Copy, PartialEq, Default)]
pub enum Mode {
    #[default]
    Slideshow,
    Favorites,
}

/// A screen as far as Esc is concerned: the pages up and the search under them.
#[derive(Clone, PartialEq, Default)]
struct Screen {
    mode: Mode,
    settings: bool,
    pools: bool,
    mosaic: bool,
    reading_list: bool,
    search: Option<String>,
}

/// A search put aside when another took its place, and the slide it was on.
type Left = (Search, usize);

/// Follows the screen: the one left goes on the stack, except that closing a page with its own button is a step back.
fn note<T>(back: &mut Vec<(Screen, Option<T>)>, here: &mut Screen, now: Screen, replaced: Option<T>) {
    if now == *here {
        return;
    }
    let was = std::mem::replace(here, now);
    if replaced.is_none() && back.last().is_some_and(|(s, _)| s == here) {
        back.pop();
    } else {
        back.push((was, replaced));
    }
}

/// The picture being shown and the one it is fading in over.
struct OnScreen {
    url: String,
    rect: egui::Rect,
    was: String,
    was_rect: egui::Rect,
    at: Option<Instant>,
}

impl Default for OnScreen {
    fn default() -> OnScreen {
        OnScreen { url: String::new(), rect: egui::Rect::NOTHING, was: String::new(), was_rect: egui::Rect::NOTHING, at: None }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Tab {
    Appearance,
    Sites,
    Slideshow,
    Filtering,
    Hotkeys,
    Folders,
    Favorites,
    Quick,
    History,
    About,
}

pub struct App {
    pub rt: tokio::runtime::Runtime,
    pub engine: Engine,
    pub store: Arc<Store>,
    pub tx: Sender<Msg>,
    rx: Receiver<Msg>,
    pub downloader: Downloader,

    pub search: Option<Search>,
    generation: u64,
    pub query: String,
    pub mode: Mode,
    pub fav: Favorites,
    pub fav_filter_box: String,
    /// Counts the times the favorites list was made anew: its mosaic then starts from the top.
    pub fav_view: u64,
    cur: [usize; 2],
    pub media: MediaCache,
    pub copies: HashMap<String, String>,
    pub hotkeys: Hotkeys,

    pub playing: bool,
    shown_at: Option<Instant>,
    shown_index: Option<(Mode, usize)>,
    pub show_tags: bool,
    pub show_controls: bool,
    pub show_settings: bool,
    pub tab: Tab,
    pub show_pools: bool,
    /// The mosaic page over the slideshow, and whether `search` is the reading list's.
    pub show_mosaic: bool,
    pub reading_list: bool,
    /// Just back from the slideshow: the mosaic brings up the post it was on.
    pub mosaic_seek: bool,
    /// The screens left behind, for Esc, with the search when the next screen replaced it.
    // ponytail: no cap; a kept search is only its posts' details (the pictures are in `media`). Cap it if a session's memory grows.
    back: Vec<(Screen, Option<Left>)>,
    here: Screen,
    replaced: Option<Left>,
    pub notices: Vec<(String, Instant)>,
    /// The favorite (its file) that ♥ was pressed on once while browsing the favorites, and when.
    unfave: Option<(String, Instant)>,
    pub progress: String,
    pub recording: Option<(String, usize)>,
    logo: egui::TextureHandle,
    /// Where the player is (nothing while hidden), whether the drawer is out, and the test script's order to keep it out.
    player_rect: egui::Rect,
    drawer_open: bool,
    drawer_forced: bool,
    on_screen: OnScreen,
    /// When the screen last changed, for the fade between pages.
    changed_at: Option<Instant>,
    /// The window for Windows' title bar, and the theme it was last painted for.
    window: Option<isize>,
    title_theme: Option<egui::Theme>,
    /// Height ÷ width of pictures whose site doesn't say (your folders), by file.
    pub ratios: HashMap<String, f32>,
    /// The picture blurred behind everything, the one before it, and when it changed.
    ambient: (String, String, Instant),
    pub pool_filter: String,
    pub video: Option<Player>,
    covers: HashMap<usize, Slide>,
    covers_asked: bool,
    pub video_error: Option<String>,
    // Testing aid: MSG_SHOT=<png> takes a screenshot after MSG_SHOT_AFTER seconds (default 12) and quits; MSG_SEARCH=<tags> searches at start.
    hook: Option<(std::path::PathBuf, Instant, bool)>,
    // MSG_SCRIPT="18:shot=a,19:next,21:prev,24:shot=b,26:quit" does these at those seconds (shots go to MSG_SHOT_DIR).
    script: Option<(Instant, Vec<(f64, String)>)>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, rt: tokio::runtime::Runtime, store: Arc<Store>) -> App {
        let (tx, rx) = channel();
        let ctx = cc.egui_ctx.clone();
        let engine = Engine { rt: rt.handle().clone(), client: super::net::client(), store: store.clone(), wake: Arc::new(move || ctx.request_repaint()) };
        let downloader = Downloader::new(engine.clone(), tx.clone());
        let mut app = App {
            rt,
            engine,
            fav: Favorites::load(&store),
            hotkeys: Hotkeys::load(&store),
            copies: HashMap::new(),
            store,
            tx,
            rx,
            downloader,
            search: None,
            generation: 0,
            query: String::new(),
            mode: Mode::Slideshow,
            fav_filter_box: String::new(),
            fav_view: 0,
            cur: [0, 0],
            media: MediaCache::new(),
            playing: false,
            shown_at: None,
            shown_index: None,
            show_tags: false,
            show_controls: true,
            show_settings: false,
            tab: Tab::Appearance,
            show_pools: false,
            show_mosaic: false,
            reading_list: false,
            mosaic_seek: false,
            back: vec![],
            here: Screen::default(),
            replaced: None,
            notices: vec![],
            unfave: None,
            progress: String::new(),
            recording: None,
            logo: theme::logo(&cc.egui_ctx),
            player_rect: egui::Rect::NOTHING,
            drawer_open: false,
            drawer_forced: false,
            on_screen: OnScreen::default(),
            changed_at: None,
            window: match cc.window_handle().map(|h| h.as_raw()) {
                Ok(raw_window_handle::RawWindowHandle::Win32(handle)) => Some(handle.hwnd.get()),
                _ => None,
            },
            title_theme: None,
            ratios: HashMap::new(),
            ambient: (String::new(), String::new(), Instant::now()),
            pool_filter: String::new(),
            video: None,
            covers: HashMap::new(),
            covers_asked: false,
            video_error: None,
            hook: std::env::var_os("MSG_SHOT").map(|p| (p.into(), Instant::now(), false)),
            script: std::env::var("MSG_SCRIPT").ok().map(|text| {
                let steps = text.split(',').filter_map(|s| s.split_once(':')).filter_map(|(t, a)| Some((t.trim().parse().ok()?, a.trim().to_string()))).collect();
                (Instant::now(), steps)
            }),
        };
        match cc.get_proc_address.as_deref().ok_or_else(|| "no OpenGL".to_string()).and_then(|get| Player::new(get, cc.egui_ctx.clone())) {
            Ok(player) => app.video = Some(player),
            Err(e) => app.video_error = Some(e),
        }
        app.show_tags = app.flag("showTags", false);
        super::theme::install(&cc.egui_ctx);
        app.apply_theme(&cc.egui_ctx);
        app.refresh_copies();
        media::clear_video_files();
        app.downloader.schedule(Duration::from_secs(15));
        if app.auto_follow_limit() > 0.0 {
            favorites::fetch_tag_groups(&app.engine, app.tx.clone(), &app.fav.items);
        }
        if let Ok(text) = std::env::var("MSG_SEARCH") {
            app.start_search(&text);
        } else if app.reading_home() {
            app.open_reading();
        }
        app.here = app.screen();
        app
    }

    // ---------- settings ----------

    pub fn flag(&self, key: &str, default: bool) -> bool {
        self.store.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
    }

    pub fn text(&self, key: &str) -> String {
        self.store.get(key).and_then(|v| v.as_str().map(String::from)).unwrap_or_default()
    }

    pub fn number(&self, key: &str, default: f64) -> f64 {
        match self.store.get(key) {
            Some(Value::Number(n)) => n.as_f64().unwrap_or(default),
            Some(Value::String(s)) => s.trim().parse().unwrap_or(default),
            _ => default,
        }
    }

    /// How many of the artists with the most favorites the reading list follows by itself (0 = none).
    pub fn auto_follow_limit(&self) -> f64 {
        self.number("autoFollowArtists", 10.0)
    }

    /// The front page is the reading list's mosaic, unless the quick search cards were chosen.
    pub fn reading_home(&self) -> bool {
        self.text("frontPage") != "search"
    }

    /// Searches open as a mosaic, not straight as a slideshow.
    pub fn mosaic_first(&self) -> bool {
        self.flag("mosaicFirst", true)
    }

    pub fn set(&self, key: &str, value: Value) {
        self.store.data.lock().unwrap().insert(key.to_string(), value);
        self.store.save();
    }

    pub fn notice(&mut self, text: impl Into<String>) {
        self.notices.push((text.into(), Instant::now()));
        if self.notices.len() > 4 {
            self.notices.remove(0);
        }
    }

    pub fn apply_theme(&self, ctx: &egui::Context) {
        ctx.set_theme(match self.text("appTheme").as_str() {
            "light" => egui::ThemePreference::Light,
            "system" => egui::ThemePreference::System,
            _ => egui::ThemePreference::Dark,
        });
    }

    pub fn refresh_copies(&mut self) {
        self.copies = if self.flag("useLocalCopies", true) { downloads::local_copies(&self.store) } else { HashMap::new() };
    }

    /// Settings that change what the background job or the lists depend on.
    pub fn settings_changed(&mut self) {
        self.downloader.schedule(Duration::from_secs(5));
        self.rebuild_favorites();
    }

    pub fn rebuild_favorites(&mut self) {
        let offline = self.flag("offlineMode", false);
        let copies = self.copies.clone();
        self.fav.rebuild(&|s| !offline || downloads::display_url(&copies, &s.file_url) != s.file_url);
    }

    // ---------- the list being shown ----------

    pub fn list(&self) -> &[Slide] {
        match self.mode {
            Mode::Slideshow => self.search.as_ref().map(|s| s.slides.as_slice()).unwrap_or(&[]),
            Mode::Favorites => &self.fav.view,
        }
    }

    pub fn idx(&self) -> usize {
        self.cur[self.mode as usize]
    }

    pub fn set_idx(&mut self, i: usize) {
        let len = self.list().len();
        self.cur[self.mode as usize] = if len == 0 { 0 } else { i.min(len - 1) };
        self.shown_at = None;
        // The new slide's clock starts in the next frame. A frame that came from a timer is followed by no other by itself.
        self.engine.wake();
    }

    pub fn current(&self) -> Option<Slide> {
        self.list().get(self.idx()).cloned()
    }

    /// A page is over the slideshow: no clock, no video, no downloads for it, and no keys that act on its slide.
    pub fn covered(&self) -> bool {
        self.show_pools || self.show_settings || self.show_mosaic
    }

    /// The mosaic is the page on top.
    pub fn on_mosaic(&self) -> bool {
        self.show_mosaic && !self.show_pools && !self.show_settings
    }

    /// The reading list's mosaic is the page on top.
    pub fn on_reading(&self) -> bool {
        self.on_mosaic() && self.reading_list && self.mode == Mode::Slideshow
    }

    /// The front page is showing, in either of its forms.
    pub fn on_home(&self) -> bool {
        self.on_reading() || (!self.covered() && self.mode == Mode::Slideshow && self.search.is_none())
    }

    /// From the mosaic to the slideshow, running from the first post.
    pub fn start_slideshow(&mut self) {
        self.show_mosaic = false;
        self.set_idx(0);
        self.playing = true;
    }

    pub fn move_by(&mut self, delta: i64) {
        let len = self.list().len() as i64;
        if len == 0 {
            return;
        }
        let i = self.idx() as i64 + delta;
        // Past the end: on to the start (favorites), or stay while more is loading.
        let i = if i >= len {
            if self.mode == Mode::Favorites || !self.search.as_ref().is_some_and(|s| s.has_more()) { 0 } else { len - 1 }
        } else {
            i.max(0)
        };
        self.set_idx(i as usize);
    }

    // ---------- going back ----------

    fn screen(&self) -> Screen {
        Screen {
            mode: self.mode,
            settings: self.show_settings,
            pools: self.show_pools,
            mosaic: self.show_mosaic,
            reading_list: self.reading_list,
            search: self.search.as_ref().map(|s| s.text.clone()),
        }
    }

    /// The search is about to be replaced: Esc can bring it back.
    fn set_aside_search(&mut self) {
        self.replaced = self.search.take().map(|s| (s, self.cur[Mode::Slideshow as usize]));
    }

    /// Returns to the screen before, and its search where it was left. False if there is none.
    pub fn go_back(&mut self) -> bool {
        let Some((screen, left)) = self.back.pop() else { return false };
        match left {
            Some((search, i)) => {
                self.query = search.text.clone();
                self.search = Some(search);
                self.cur[Mode::Slideshow as usize] = i;
            }
            None if screen.search.is_none() => self.search = None,
            None => {}
        }
        self.mode = screen.mode;
        self.show_settings = screen.settings;
        self.show_pools = screen.pools;
        self.mosaic_seek = screen.mosaic && !self.show_mosaic;
        self.show_mosaic = screen.mosaic;
        self.reading_list = screen.reading_list;
        self.shown_at = None;
        self.here = screen;
        true
    }

    // ---------- searching ----------

    pub fn start_search(&mut self, text: &str) {
        self.mode = Mode::Slideshow;
        self.show_pools = false;
        self.show_settings = false;
        self.show_mosaic = self.mosaic_first();
        self.generation += 1;
        let text = text.trim();
        self.query = text.to_string();
        let sites = session::ticked_sites(&self.store);
        if sites.is_empty() {
            self.notice("Tick at least one site (Sites ▾).");
            return;
        }
        self.set_aside_search();
        self.search = Some(Search::new(self.generation, text, sites, &self.store));
        self.reading_list = false;
        self.cur[Mode::Slideshow as usize] = 0;
        self.shown_at = None;
        if self.flag("storeHistory", true) && !text.is_empty() {
            let mut history: Vec<Value> = self.store.get("searchHistory").and_then(|v| v.as_array().cloned()).unwrap_or_default();
            history.retain(|h| h.as_str() != Some(text));
            history.push(json!(text));
            let extra = history.len().saturating_sub(100);
            history.drain(..extra);
            self.set("searchHistory", Value::Array(history));
        }
        self.load_more();
    }

    /// The reading list page; its search starts unless it is the one already loaded.
    pub fn open_reading(&mut self) {
        self.mode = Mode::Slideshow;
        self.show_pools = false;
        self.show_settings = false;
        self.show_mosaic = true;
        if self.refresh_auto_follow() || !self.reading_list {
            self.start_reading();
        }
    }

    /// The followed searches together, newest first (not saved in the history).
    pub fn start_reading(&mut self) {
        self.generation += 1;
        self.query.clear();
        self.set_aside_search();
        self.search = Some(Search::many(self.generation, &session::reading_list(&self.store), &self.store));
        self.reading_list = true;
        self.cur[Mode::Slideshow as usize] = 0;
        self.shown_at = None;
        self.load_more();
    }

    /// The artists with the most favorites join the reading list and those no longer among them leave; what is
    /// followed by hand or was unfollowed is left alone. Returns whether the list changed.
    fn refresh_auto_follow(&mut self) -> bool {
        let list: Vec<Value> = self.store.get("readingList").and_then(|v| v.as_array().cloned()).unwrap_or_default();
        let (old, mut kept): (Vec<Value>, Vec<Value>) = list.into_iter().partition(|e| e["auto"] == true);
        let ignored = self.store.get("readingListIgnored").and_then(|v| v.as_array().cloned()).unwrap_or_default();
        let skip: HashSet<String> = ignored.iter().chain(kept.iter().map(|e| &e["tags"])).filter_map(|t| t.as_str().map(String::from)).collect();
        let new: Vec<Value> = favorites::top_artists(&self.fav.items, &skip, self.auto_follow_limit() as usize)
            .into_iter()
            .map(|(tags, sites)| json!({"tags": tags, "sites": sites, "auto": true}))
            .collect();
        if new == old {
            return false;
        }
        kept.extend(new);
        self.set("readingList", Value::Array(kept));
        true
    }

    /// Follows the search (on the ticked sites), or stops following it; what is unfollowed isn't followed automatically again.
    pub fn toggle_follow(&mut self, tags: &str) {
        let mut list: Vec<Value> = self.store.get("readingList").and_then(|v| v.as_array().cloned()).unwrap_or_default();
        let mut ignored: Vec<Value> = self.store.get("readingListIgnored").and_then(|v| v.as_array().cloned()).unwrap_or_default();
        let before = list.len();
        list.retain(|e| e["tags"].as_str() != Some(tags));
        ignored.retain(|t| t.as_str() != Some(tags));
        if list.len() == before {
            let sites: Vec<&str> = session::ticked_sites(&self.store).iter().map(|s| s.id()).collect();
            list.push(json!({"tags": tags, "sites": sites}));
        } else {
            ignored.push(json!(tags));
        }
        self.set("readingList", Value::Array(list));
        self.set("readingListIgnored", Value::Array(ignored));
    }

    pub fn load_more(&mut self) {
        if let Some(search) = &mut self.search {
            search.load_more(&self.engine, self.tx.clone());
        }
    }

    fn handle_messages(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Batch(batch) => {
                    // A search left while it was loading takes its answer all the same, for Esc.
                    if let Some(left) = self.back.iter_mut().filter_map(|(_, left)| left.as_mut()).find(|left| left.0.generation == batch.generation) {
                        left.0.accept(batch);
                        continue;
                    }
                    let Some(search) = self.search.as_mut().filter(|s| s.generation == batch.generation) else { continue };
                    let warnings = batch.warnings.clone();
                    search.accept(batch);
                    let empty = search.slides.is_empty() && !search.has_more();
                    for w in warnings {
                        self.notice(w);
                    }
                    if empty {
                        self.notice("No results.");
                    }
                }
                Msg::Notice(text) => self.notice(text),
                Msg::Cover { index, slide } => {
                    self.covers.insert(index, slide);
                }
                Msg::Progress(text) => self.progress = text,
                Msg::Imported { site, slides } => {
                    let found = slides.len();
                    let added = self.fav.add_many(slides);
                    self.fav.save(&self.store);
                    self.rebuild_favorites();
                    self.notice(format!("{site}: found {found} favorites, {added} new."));
                    self.settings_changed();
                }
                Msg::Downloaded(status) => {
                    self.refresh_copies();
                    self.notice(status);
                }
                Msg::TagGroups(found) => {
                    self.fav.set_tag_groups(found);
                    self.fav.save(&self.store);
                    if self.reading_list && self.refresh_auto_follow() {
                        self.start_reading();
                    }
                }
                Msg::Pools(found) => {
                    let mut pools: Vec<Value> = self.store.get("savedPools").and_then(|v| v.as_array().cloned()).unwrap_or_default();
                    for pool in found {
                        if !pools.iter().any(|p| p["id"] == pool["id"]) {
                            pools.push(pool);
                        }
                    }
                    self.set("savedPools", Value::Array(pools));
                    self.settings_changed();
                }
            }
        }
    }

    // ---------- actions ----------

    pub fn toggle_favorite(&mut self) {
        if let Some(slide) = self.current() {
            self.toggle_favorite_of(slide);
        }
    }

    pub fn toggle_favorite_of(&mut self, slide: Slide) {
        // Browsing the favorites, a removal takes a second press: the post leaves the list, and there is no undoing it.
        if self.mode == Mode::Favorites && self.fav.contains(&slide) && !self.unfave_armed(&slide) {
            self.unfave = Some((slide.file_url.clone(), Instant::now()));
            return self.notice("Press ♥ again to remove it from the favorites.");
        }
        self.unfave = None;
        let faved = self.fav.toggle(&slide);
        self.fav.save(&self.store);
        favorites::sync_e621(&self.engine, &self.tx, &slide, faved);
        if self.mode == Mode::Favorites {
            // The list shrank: stay at the same place.
            let i = self.idx();
            self.set_idx(i);
        }
        self.downloader.schedule(Duration::from_secs(5));
    }

    /// ♥ was pressed once on this favorite, a moment ago.
    fn unfave_armed(&self, slide: &Slide) -> bool {
        self.unfave.as_ref().is_some_and(|(url, at)| *url == slide.file_url && at.elapsed() < UNFAVE_WAIT)
    }

    fn open_source(&self) {
        if let Some(slide) = self.current() {
            if net_ok(&slide.viewable_website_post_url) {
                let _ = open::that(&slide.viewable_website_post_url);
            }
        }
    }

    fn download_current(&mut self) {
        if let Some(slide) = self.current() {
            if slide.site_id == "LOCL" {
                return self.notice("That file is already on your computer.");
            }
            self.downloader.download_one(&slide.file_url);
        }
    }

    pub fn use_as_background(&mut self) {
        let Some(slide) = self.current() else { return };
        let target = self.copies_or(&slide.file_url);
        if !super::net::web_url(&target).is_some() {
            self.set("backgroundImage", json!(target));
            return self.notice("Background set.");
        }
        self.notice("Only pictures that are on the disk (downloaded copies, your folders) can be the background.");
    }

    fn copies_or(&self, url: &str) -> String {
        downloads::display_url(&self.copies, url)
    }

    fn toggle_tag_search(&mut self, tag: &str) {
        let add = self.text("tagClick") == "add";
        if self.mode == Mode::Favorites {
            self.fav_filter_box = if add && !self.fav_filter_box.is_empty() { format!("{} {tag}", self.fav_filter_box) } else { tag.to_string() };
            self.apply_favorite_filter();
            self.show_mosaic = self.mosaic_first();
        } else {
            let text = if add && !self.query.is_empty() { format!("{} {tag}", self.query) } else { tag.to_string() };
            self.start_search(&text);
        }
    }

    pub fn apply_favorite_filter(&mut self) {
        self.fav.filter = self.fav_filter_box.trim().to_string();
        self.fav.random = false;
        self.rebuild_favorites();
        self.fav_view += 1;
        self.cur[Mode::Favorites as usize] = 0;
        self.shown_at = None;
    }

    pub fn open_favorites(&mut self, on: bool) {
        self.mode = if on { Mode::Favorites } else { Mode::Slideshow };
        self.show_pools = false;
        self.show_settings = false;
        self.show_mosaic = self.mosaic_first() && (on || self.search.is_some());
        self.shown_at = None;
        if on {
            self.rebuild_favorites();
        }
    }

    fn run_quick_search(&mut self, n: usize) {
        let Some(quick) = self.store.get("quickSearches").and_then(|v| v.as_array().and_then(|a| a.get(n).cloned())) else { return };
        let ids: Vec<&str> = quick["sites"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        if ids.is_empty() {
            return;
        }
        let map: serde_json::Map<String, Value> = Site::ALL.iter().map(|s| (s.id().to_string(), json!(ids.contains(&s.id())))).collect();
        self.set("sitesToSearch", Value::Object(map));
        let tags = quick["tags"].as_str().unwrap_or("").trim().to_string();
        let text = if tags.is_empty() { self.query.clone() } else { tags };
        self.start_search(&text);
    }

    fn act(&mut self, ctx: &egui::Context, action: &str) {
        match action {
            // On the mosaic Space and Enter start the slideshow.
            "playPause" if self.on_mosaic() => {
                if !self.list().is_empty() {
                    self.start_slideshow();
                }
            }
            // Under a page there is no slide for these to act on.
            "previous" | "next" | "back10" | "forward10" | "playPause" | "autoFit" | "openSource" | "download" | "favorite" | "toggleTags" | "setBackground" if self.covered() => {}
            "previous" => self.move_by(-1),
            "next" => self.move_by(1),
            "back10" => self.move_by(-10),
            "forward10" => self.move_by(10),
            "playPause" => self.playing = !self.playing,
            "autoFit" => {
                let v = !self.flag("autoFitSlide", true);
                self.set("autoFitSlide", json!(v));
            }
            "openSource" => self.open_source(),
            "download" => self.download_current(),
            "favorite" => self.toggle_favorite(),
            "toggleTags" => {
                self.show_tags = !self.show_tags;
                self.set("showTags", json!(self.show_tags));
            }
            // From under a page the button means the favorites, whichever list is under it.
            "openFavorites" => self.open_favorites(self.mode == Mode::Slideshow || self.show_pools || self.show_settings),
            "openSettings" => self.show_settings = !self.show_settings,
            "setBackground" => self.use_as_background(),
            "showInterface" => self.show_controls = !self.show_controls,
            // Back to the front page: the quick search cards (a new search starts from there as before), or the reading list read again.
            "home" if self.reading_home() => {
                self.reading_list = false;
                self.open_reading();
            }
            "home" => {
                self.mode = Mode::Slideshow;
                self.set_aside_search();
                self.reading_list = false;
                self.show_pools = false;
                self.show_settings = false;
                self.show_mosaic = false;
            }
            // From the settings page straight to the pools page.
            "openPools" => {
                self.show_pools = !self.show_pools || self.show_settings;
                self.show_settings = false;
            }
            "openReadingList" if self.on_reading() => {
                self.go_back();
            }
            "openReadingList" => self.open_reading(),
            // Back to the slideshow where it was left.
            "showSlideshow" => {
                self.show_pools = false;
                self.show_settings = false;
                self.show_mosaic = false;
            }
            // Esc: the screen before; with none left, out of fullscreen.
            "back" => {
                if !self.go_back() && ctx.input(|i| i.viewport().fullscreen.unwrap_or(false)) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
                }
            }
            _ => {}
        }
        ctx.request_repaint();
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        if self.recording.is_some() {
            return;
        }
        let (events, wants_text) = (ctx.input(|i| i.events.clone()), ctx.egui_wants_keyboard_input());
        let mouse = !self.covered();
        for action in self.hotkeys.pressed(&events, wants_text, mouse) {
            self.act(ctx, action);
        }
        // Fixed keys: Enter also plays and pauses, F11 is fullscreen, Esc goes back, 1–0 are the quick searches.
        for event in &events {
            if let egui::Event::Key { key, pressed: true, repeat: false, modifiers, .. } = event {
                match key {
                    egui::Key::F11 => {
                        let full = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!full));
                    }
                    // An open menu closes by itself on this press.
                    egui::Key::Escape if !egui::Popup::is_any_open(ctx) => self.act(ctx, "back"),
                    egui::Key::Enter if !wants_text && !modifiers.any() => self.act(ctx, "playPause"),
                    _ if !wants_text && !modifiers.any() && self.mode == Mode::Slideshow => {
                        let digit = match key {
                            egui::Key::Num1 => Some(0), egui::Key::Num2 => Some(1), egui::Key::Num3 => Some(2), egui::Key::Num4 => Some(3),
                            egui::Key::Num5 => Some(4), egui::Key::Num6 => Some(5), egui::Key::Num7 => Some(6), egui::Key::Num8 => Some(7),
                            egui::Key::Num9 => Some(8), egui::Key::Num0 => Some(9), _ => None,
                        };
                        if let Some(n) = digit {
                            self.run_quick_search(n);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // ---------- the slideshow clock ----------

    /// Starts the video of the slide being shown (and stops it when another kind of slide comes up).
    fn sync_video(&mut self, wanted: Option<String>) {
        let (autoplay, to_end, hardware) = (self.flag("videoAutoplay", true), self.flag("playVideosToEnd", false), self.flag("videoHwdec", false));
        let (volume, muted) = (self.number("videoVolume", 0.5), self.flag("videoAutoMute", false) || self.flag("videoMuted", false));
        let Some(player) = self.video.as_mut() else { return };
        match wanted {
            Some(url) if player.url != url => {
                let referer = super::net::web_url(&url).and_then(|u| super::net::referer_for(&u));
                player.set_volume(volume, muted);
                player.set_hwdec(hardware);
                // A video fetched ahead starts at once; any other is read from the site as it plays.
                let file = match self.media.get(&url) {
                    Some(State::File(file)) => file.path(),
                    _ => None,
                };
                player.load(file.as_deref().unwrap_or(&url), referer, !to_end, autoplay);
                player.url = url;
            }
            None => player.stop(),
            _ => {}
        }
    }

    fn tick(&mut self, ctx: &egui::Context) {
        // A page over the slideshow: the video stops and the slide's clock starts over afterwards.
        let current = if self.covered() { None } else { self.current() };
        let wanted = current.as_ref().filter(|s| s.media_type == MediaType::Video).map(|s| self.copies_or(&s.file_url));
        self.sync_video(wanted);
        let Some(slide) = current else {
            self.shown_index = None;
            return;
        };
        let here = (self.mode, self.idx());
        if self.shown_index != Some(here) {
            self.shown_index = Some(here);
            self.shown_at = None;
        }
        let is_video = slide.media_type == MediaType::Video;
        let url = self.copies_or(&slide.file_url);
        let settled = if is_video {
            true
        } else {
            matches!(self.media.get(&url), Some(State::Ready(_) | State::Failed(_)))
        };
        if settled && self.shown_at.is_none() {
            self.shown_at = Some(Instant::now());
        }
        if !self.playing {
            return;
        }
        let Some(at) = self.shown_at else { return };
        // A video that is played to the end moves on when it has ended.
        if is_video && self.flag("playVideosToEnd", false) {
            if let Some(player) = &mut self.video {
                // Right after a load mpv still answers for the previous file (or for none), so the first moment doesn't count.
                if player.status().ended && at.elapsed() >= Duration::from_secs(1) {
                    // Stopped, so a video that comes after itself (the only one in the list) starts over.
                    player.stop();
                    self.move_by(1);
                } else {
                    ctx.request_repaint_after(Duration::from_millis(300));
                }
                return;
            }
        }
        let failed = matches!(self.media.get(&url), Some(State::Failed(_)));
        let wait = if failed { Duration::from_secs(1) } else { Duration::from_secs_f64(self.number("secondsPerSlide", 6.0).max(1.0)) };
        if at.elapsed() >= wait {
            self.move_by(1);
        } else {
            ctx.request_repaint_after(wait - at.elapsed());
        }
    }

    fn preload(&mut self) {
        // What is under a page waits: the mosaic asks for its own pictures, and these would hold them up.
        if self.covered() {
            return;
        }
        let i = self.idx();
        let wanted: Vec<(String, String)> = self
            .list()
            .iter()
            .enumerate()
            .skip(i)
            .take(AHEAD)
            .map(|(n, s)| (if n < i + 4 && s.media_type != MediaType::Video { s.file_url.clone() } else { String::new() }, s.preview_file_url.clone()))
            .collect();
        for (full, preview) in wanted {
            let (full, preview) = (self.copies_or(&full), self.copies_or(&preview));
            self.media.request(&self.engine, &full);
            self.media.request(&self.engine, &thumb_source(&preview));
        }
        // The next videos come ahead of their turn; the one on the screen is mpv's.
        let videos: Vec<String> = self.list().iter().skip(i + 1).take(3).filter(|s| s.media_type == MediaType::Video).map(|s| self.copies_or(&s.file_url)).collect();
        for url in videos.iter().filter(|url| super::net::web_url(url).is_some()) {
            self.media.request_video(&self.engine, url);
        }
        if self.mode == Mode::Slideshow && self.search.as_ref().is_some_and(|s| s.has_more() && s.slides.len() < i + 25) {
            self.load_more();
        }
    }

    // ---------- drawing ----------

    /// One row: the buttons are laid out from the right end, and the search box takes the width that is left.
    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if self.mode == Mode::Slideshow && !self.show_pools && !self.show_settings {
                self.sites_menu(ui);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.show_pools {
                    return self.pools_bar(ui);
                }
                if self.on_home() {
                    self.home_switch(ui);
                }
                // The mosaic keeps the search bar (or the favorites bar).
                if self.show_mosaic {
                    self.mosaic_bar(ui);
                }
                if self.mode == Mode::Slideshow {
                    self.search_extras(ui);
                    self.search_field(ui);
                } else {
                    self.favorites_extras(ui);
                    self.favorites_field(ui);
                }
            });
        });
    }

    /// In a narrow window the toolbar's buttons are icons only.
    pub fn compact(&self, ui: &egui::Ui) -> bool {
        ui.ctx().content_rect().width() < 900.0
    }

    /// The two forms of the front page; the choice is the Front page setting.
    fn home_switch(&mut self, ui: &mut egui::Ui) {
        let chosen = if self.on_reading() { 0 } else { 1 };
        if let Some(n) = theme::segmented(ui, &if self.compact(ui) { [(icon::MOSAIC, ""), (icon::SAVED, "")] } else { [(icon::MOSAIC, "Mosaic"), (icon::SAVED, "Saved searches")] }, chosen) {
            if n != chosen {
                self.set("frontPage", json!(if n == 0 { "reading" } else { "search" }));
                self.act(&ui.ctx().clone(), "home");
            }
        }
    }

    /// Follow or Save pool, and the sort menu. Right to left in the toolbar, left to right in the drawer.
    fn search_extras(&mut self, ui: &mut egui::Ui) {
        if let Some(pool) = self.search.as_ref().and_then(|s| s.pool()) {
            if ui.button((theme::glyph(icon::FOLLOW, 14.0), "Save pool")).clicked() {
                let cover = self.search.as_ref().and_then(|s| s.slides.first().map(|x| x.preview_file_url.clone())).unwrap_or_default();
                self.save_pool(pool.0, pool.1, pool.2, cover);
            }
        } else if let Some(tags) = self.search.as_ref().map(|s| s.text.clone()).filter(|t| !t.is_empty() && !self.reading_list) {
            let following = session::reading_list(&self.store).iter().any(|(t, _)| *t == tags);
            let word = if self.compact(ui) { "" } else if following { "Following" } else { "Follow" };
            let text = (theme::glyph(if following { icon::FOLLOWING } else { icon::FOLLOW }, 14.0), word);
            if ui.button(text).on_hover_text("The reading list shows the newest posts of what you follow.").clicked() {
                self.toggle_follow(&tags);
            }
        }
        if self.flag("searchSortMenu", false) {
            self.sort_menu(ui);
        }
    }

    /// The search box with its buttons, right to left: the box takes the width that is left.
    fn search_field(&mut self, ui: &mut egui::Ui) {
        let search = theme::icon_button(ui, icon::SEARCH, "Search", false).clicked();
        let history: Vec<String> = self.store.get("searchHistory").and_then(|v| v.as_array().cloned()).unwrap_or_default().iter().rev().filter_map(|v| v.as_str().map(String::from)).collect();
        if !history.is_empty() {
            ui.menu_button(theme::glyph(icon::HISTORY, 15.0), |ui| {
                egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                    for item in history.iter().take(40) {
                        if ui.button(item).clicked() {
                            let item = item.clone();
                            ui.close();
                            self.start_search(&item);
                        }
                    }
                });
            });
        }
        let mut query = std::mem::take(&mut self.query);
        let response = ui.add(egui::TextEdit::singleline(&mut query).hint_text("Search tags…").desired_width(f32::INFINITY).margin(egui::Margin::symmetric(10, 7)));
        self.query = query;
        let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if search || enter {
            let text = self.query.clone();
            self.start_search(&text);
        }
    }

    /// The toolbar floating over the top of a page: what scrolls under it shows through the glass.
    fn toolbar(&mut self, ctx: &egui::Context) {
        let rect = ctx.content_rect();
        let width = rect.width() - RAIL_SPACE - theme::S2;
        egui::Area::new(egui::Id::new("toolbar")).order(egui::Order::Middle).fixed_pos(egui::pos2(rect.left() + RAIL_SPACE, rect.top() + theme::S2)).show(ctx, |ui| {
            glass::card(ui, glass::bar(ctx), egui::Margin::same(6), |ui| {
                ui.set_width(width - 12.0);
                ui.set_min_height(32.0);
                self.top_bar(ui);
            });
        });
    }

    /// The pages, on a glass card down the left edge of the window. The one open is marked.
    fn rail(&mut self, ui: &mut egui::Ui) {
        const ITEM: f32 = 56.0;
        let ctx = ui.ctx().clone();
        let t = tok(&ctx);
        let column = ui.max_rect();
        let card = egui::Rect::from_min_max(column.min + Vec2::splat(RAIL_MARGIN), egui::pos2(column.left() + RAIL_MARGIN + RAIL_W, column.bottom() - RAIL_MARGIN));
        glass::panel(ui, card, glass::bar(&ctx));
        let logo = egui::Rect::from_center_size(egui::pos2(card.center().x, card.top() + 24.0), Vec2::splat(26.0));
        ui.painter().image(self.logo.id(), logo, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), t.accent);
        let (settings, pools) = (self.show_settings, self.show_pools && !self.show_settings);
        let favorites = self.mode == Mode::Favorites && !pools && !settings;
        let results = self.on_mosaic() && !self.on_reading() && self.mode == Mode::Slideshow;
        let items = [
            (icon::HOME, "Home", "home", self.on_home(), true),
            (icon::SLIDESHOW, "Slideshow", "showSlideshow", results, !self.list().is_empty()),
            (icon::POOLS, "Pools", "openPools", pools, true),
            (icon::HEART, "Favorites", "openFavorites", favorites, true),
            (icon::SETTINGS, "Settings", "openSettings", settings, true),
        ];
        let place = |n: usize| {
            let top = if n == items.len() - 1 { card.bottom() - ITEM - 6.0 } else { card.top() + 48.0 + n as f32 * ITEM };
            egui::Rect::from_min_size(egui::pos2(card.left() + 4.0, top), Vec2::new(RAIL_W - 8.0, ITEM))
        };
        // The mark slides from page to page.
        if let Some(n) = items.iter().position(|i| i.3) {
            let y = ctx.animate_value_with_time(egui::Id::new("rail mark"), place(n).top(), theme::smooth());
            ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(place(n).left(), y), place(n).size()).shrink(2.0), theme::RADIUS_M, t.press);
        }
        for (n, (glyph, label, action, on, enabled)) in items.into_iter().enumerate() {
            if ui.add_enabled_ui(enabled, |ui| theme::rail_item(ui, place(n), glyph, label, on)).inner.clicked() {
                self.act(&ctx, action);
            }
        }
    }

    /// Behind everything: the picture being looked at, blurred and dimmed, so that the glass has something to show.
    /// Without one, the window's own colour and a faint glow.
    fn ambient(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let ctx = ui.ctx().clone();
        let t = tok(&ctx);
        ui.painter().rect_filled(rect, 0.0, t.base);
        ui.painter().circle_filled(egui::pos2(rect.right() - rect.width() * 0.2, rect.top()), rect.height() * 0.45, t.accent.gamma_multiply(0.16));
        ui.painter().circle_filled(egui::pos2(rect.left() + rect.width() * 0.15, rect.bottom()), rect.height() * 0.4, t.lavender.gamma_multiply(0.10));
        // Under the settings and the pools the last one stays.
        if !self.show_settings && !self.show_pools {
            if let Some(url) = self.current().map(|s| self.copies_or(&s.preview_file_url)) {
                self.media.request(&self.engine, &url);
                if url != self.ambient.0 && matches!(self.media.get(&url), Some(State::Ready(_))) {
                    self.ambient = (url, std::mem::take(&mut self.ambient.0), Instant::now());
                }
            }
        }
        let k = if theme::smooth() > 0.0 { (self.ambient.2.elapsed().as_secs_f32() / AMBIENT_FADE).min(1.0) } else { 1.0 };
        if k < 1.0 {
            ctx.request_repaint();
        }
        let (now, was) = (self.ambient.0.clone(), self.ambient.1.clone());
        for (url, alpha) in [(was, 1.0), (now, k)] {
            if let Some(State::Ready(p)) = self.media.get(&url) {
                let (w, h) = (p.size[0] as f32, p.size[1] as f32);
                let scale = (rect.width() / w).max(rect.height() / h);
                let whole = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                ui.painter().image(p.frames[0].texture.id(), egui::Rect::from_center_size(rect.center(), Vec2::new(w, h) * scale), whole, Color32::WHITE.gamma_multiply(alpha));
            }
        }
        ui.painter().add(glass::frost(&ctx, rect, glass::Look { radius: 0.0, tint: t.ambient, passes: 6 }, 1.0));
    }

    fn sites_menu(&mut self, ui: &mut egui::Ui) {
        let compact = self.compact(ui);
        ui.menu_button((theme::glyph(icon::SITES, 14.0), if compact { "" } else { "Sites" }), |ui| {
            let ticked = session::ticked_sites(&self.store);
            for site in Site::ALL {
                let mut on = ticked.contains(&site);
                if ui.checkbox(&mut on, site.name()).changed() {
                    let map: serde_json::Map<String, Value> = Site::ALL.iter().map(|s| (s.id().to_string(), json!(if *s == site { on } else { ticked.contains(s) }))).collect();
                    self.set("sitesToSearch", Value::Object(map));
                }
            }
        });
        ui.menu_button((theme::glyph(icon::FILTERS, 14.0), if compact { "" } else { "Filters" }), |ui| {
            for (key, default, label) in [
                ("includeImages", true, "Images"), ("includeGifs", true, "GIFs"), ("includeWebms", true, "Videos"),
                ("includeSafe", true, "Safe"), ("includeQuestionable", false, "Questionable"), ("includeExplicit", false, "Explicit"),
                ("includeDupes", false, "Include duplicates"),
            ] {
                let mut on = self.flag(key, default);
                if ui.checkbox(&mut on, label).changed() {
                    self.set(key, json!(on));
                }
            }
            ui.label(RichText::new("Applies to the next search.").weak());
        });
    }

    fn sort_menu(&mut self, ui: &mut egui::Ui) {
        let current = self.text("searchSort");
        let names = [("", "Default"), ("order:id_desc", "Newest"), ("order:id_asc", "Oldest"), ("order:score_desc", "Highest score"), ("order:score_asc", "Lowest score")];
        let label = names.iter().find(|(v, _)| *v == current).map(|(_, l)| *l).unwrap_or("Default");
        egui::ComboBox::from_id_salt("sort").selected_text(label).show_ui(ui, |ui| {
            for (value, name) in names {
                if ui.selectable_label(current == value, name).clicked() {
                    self.set("searchSort", json!(value));
                    // The term is set in the box in place of typing it.
                    let plain: Vec<&str> = self.query.split_whitespace().filter(|w| !w.to_lowercase().starts_with("order:") && !w.to_lowercase().starts_with("sort:")).collect();
                    self.query = if value.is_empty() { plain.join(" ") } else { format!("{} {value}", plain.join(" ")).trim().to_string() };
                }
            }
        });
    }

    /// How many are shown, the site and Random.
    fn favorites_extras(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new(format!("{} shown", self.fav.view.len())).small().weak());
        let counts = self.fav.site_counts();
        let selected = self.fav.site.clone();
        let label = selected.as_ref().and_then(|s| Site::from_id(s)).map(|s| s.name().to_string()).unwrap_or_else(|| format!("All sites ({})", self.fav.items.len()));
        egui::ComboBox::from_id_salt("favsite").selected_text(label).show_ui(ui, |ui| {
            if ui.selectable_label(selected.is_none(), format!("All sites ({})", self.fav.items.len())).clicked() {
                self.fav.site = None;
                self.apply_favorite_filter();
            }
            for (id, n) in counts {
                let name = Site::from_id(&id).map(|s| s.name().to_string()).unwrap_or_else(|| id.clone());
                if ui.selectable_label(selected.as_deref() == Some(id.as_str()), format!("{name} ({n})")).clicked() {
                    self.fav.site = Some(id);
                    self.apply_favorite_filter();
                }
            }
        });
        if ui.button((theme::glyph(icon::RANDOM, 14.0), "Random")).clicked() {
            self.fav.filter = self.fav_filter_box.trim().to_string();
            self.fav.random = true;
            self.rebuild_favorites();
            self.fav_view += 1;
            self.cur[Mode::Favorites as usize] = 0;
            self.shown_at = None;
        }
    }

    /// The filter box with its button, right to left.
    fn favorites_field(&mut self, ui: &mut egui::Ui) {
        let filter_clicked = theme::icon_button(ui, icon::SEARCH, "Filter", false).clicked();
        let mut filter = std::mem::take(&mut self.fav_filter_box);
        let response = ui.add(egui::TextEdit::singleline(&mut filter).hint_text("Filter by tags (-tag, ~tag, tag*)").desired_width(f32::INFINITY).margin(egui::Margin::symmetric(10, 7)));
        self.fav_filter_box = filter;
        let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if filter_clicked || enter {
            self.apply_favorite_filter();
        }
    }

    /// The strip of the next slides: 4:3, as high as its top edge is dragged.
    fn thumb_strip(&mut self, ui: &mut egui::Ui) {
        let i = self.idx();
        let size = Vec2::new(4.0 / 3.0, 1.0) * self.number("thumbHeight", 42.0) as f32;
        // As many as fit (and two more, for the scrolling), so only those are loaded.
        let fit = (ui.available_width() / (size.x + 8.0)).ceil() as usize + 2;
        let thumbs: Vec<(usize, String)> = self.list().iter().enumerate().skip(i + 1).take(fit).map(|(n, s)| (n, s.preview_file_url.clone())).collect();
        if thumbs.is_empty() {
            return;
        }
        // The top edge is dragged like a window's edge; a short line in its middle shows where.
        let (edge, drag) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 6.0), Sense::drag());
        let active = drag.hovered() || drag.dragged();
        if active {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
        let t = tok(ui.ctx());
        ui.painter().hline(edge.center().x - 20.0..=edge.center().x + 20.0, edge.center().y, egui::Stroke::new(2.0, if active { t.accent } else { t.hairline }));
        if drag.dragged() {
            let most = (ui.ctx().content_rect().height() / 2.0).max(24.0);
            self.set("thumbHeight", json!((size.y - drag.drag_delta().y).clamp(24.0, most)));
        }
        let mut jump = None;
        egui::ScrollArea::horizontal().id_salt("thumbs").show(ui, |ui| {
            ui.horizontal(|ui| {
                for (n, url) in &thumbs {
                    let url = thumb_source(&self.copies_or(url));
                    // Also asks for what is missing (the list may have grown); a thumbnail in use is not let go.
                    self.media.request(&self.engine, &url);
                    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
                    ui.painter().rect_filled(rect, theme::RADIUS_S, t.control);
                    if let Some(State::Ready(p)) = self.media.get(&url) {
                        paint_fit(ui, rect, &p.frames[0].texture, p.size, if response.hovered() { 1.0 } else { 0.85 }, theme::RADIUS_S as f32);
                    }
                    if response.clicked() {
                        jump = Some(*n);
                    }
                }
            });
        });
        if let Some(n) = jump {
            self.set_idx(n);
        }
    }

    /// The buttons of the slideshow and of the video on the screen: the one thing floating over the picture. It goes
    /// away a moment after the pointer stops (with the pointer), unless the pointer is on it or the video is paused.
    fn player(&mut self, ctx: &egui::Context) {
        let area = ctx.content_rect();
        let (idle, pointer) = ctx.input(|i| (i.pointer.time_since_last_movement(), i.pointer.hover_pos()));
        let video = self.current().is_some_and(|s| s.media_type == MediaType::Video) && self.video.is_some();
        let status = self.video.as_ref().filter(|_| video).map(|p| p.status());
        let over = pointer.is_some_and(|p| self.player_rect.expand(theme::S3).contains(p));
        let show = over || self.drawer_forced || idle < CHROME_SECS || status.as_ref().is_some_and(|s| s.paused) || egui::Popup::is_any_open(ctx);
        if idle < CHROME_SECS {
            // A timed repaint brings one frame only, and this is the frame that hides the bar.
            ctx.request_repaint_after_secs(CHROME_SECS - idle + 0.02);
        }
        if !show && !self.drawer_open && pointer.is_some() {
            ctx.set_cursor_icon(egui::CursorIcon::None);
        }
        let k = ctx.animate_bool_with_time(egui::Id::new("player"), show && self.show_controls, theme::smooth());
        if k == 0.0 {
            self.player_rect = egui::Rect::NOTHING;
            return;
        }
        let left = area.left() + if self.show_tags { DRAWER_SPACE } else { 0.0 };
        let width = (area.right() - left - 2.0 * theme::S4).min(760.0);
        let wide = status.is_some() || self.flag("showThumbs", true);
        let (mut pause, mut seek, mut audio) = (None, None, false);
        let (mut volume, mut muted) = (self.number("videoVolume", 0.5), self.flag("videoMuted", false));
        let pos = egui::pos2((left + area.right()) / 2.0, area.bottom() - theme::S4 + (1.0 - k) * theme::S3);
        let shown = egui::Area::new(egui::Id::new("player area")).order(egui::Order::Middle).pivot(egui::Align2::CENTER_BOTTOM).fixed_pos(pos).interactable(k > 0.5).show(ctx, |ui| {
            ui.set_opacity(k);
            glass::card(ui, glass::bar(ctx), egui::Margin::same(8), |ui| {
                if wide {
                    ui.set_width(width - 16.0);
                }
                if self.flag("showThumbs", true) {
                    self.thumb_strip(ui);
                }
                let len = self.list().len();
                let t = tok(ctx);
                ui.horizontal(|ui| {
                    if theme::icon_button(ui, icon::PREVIOUS, "Previous", false).clicked() {
                        self.move_by(-1);
                    }
                    if theme::icon_button(ui, if self.playing { icon::PAUSE } else { icon::PLAY }, if self.playing { "Pause the slideshow" } else { "Play the slideshow" }, self.playing).clicked() {
                        self.playing = !self.playing;
                    }
                    if theme::icon_button(ui, icon::NEXT, "Next", false).clicked() {
                        self.move_by(1);
                    }
                    ui.add_space(theme::S1);
                    ui.label(RichText::new(if len == 0 { "0 / 0".to_string() } else { format!("{} / {}{}", self.idx() + 1, len, if self.search.as_ref().is_some_and(|s| s.has_more()) && self.mode == Mode::Slideshow { "+" } else { "" }) }).font(theme::medium(12.5)));
                    let mut secs = self.number("secondsPerSlide", 6.0);
                    if ui.add(egui::DragValue::new(&mut secs).range(1.0..=600.0).suffix(" s")).on_hover_text("Seconds per slide").changed() {
                        self.set("secondsPerSlide", json!(secs));
                    }
                    let what = if self.mode == Mode::Favorites { "Favorites".to_string() } else if self.reading_list { "Reading list".to_string() } else { self.search.as_ref().map_or(String::new(), |s| s.text.chars().take(48).collect()) };
                    ui.add_space(theme::S1);
                    ui.label(RichText::new(what).small().color(t.text2));
                });
                if let Some(status) = &status {
                    ui.horizontal(|ui| {
                        if theme::icon_button(ui, if status.paused { icon::PLAY } else { icon::PAUSE }, if status.paused { "Play the video" } else { "Pause the video" }, false).clicked() {
                            pause = Some(!status.paused);
                        }
                        ui.label(RichText::new(format!("{} / {}", clock(status.position), clock(status.duration))).small().color(t.text2));
                        // Volume at the right end, and the seek bar takes the width between (a slider is slider_width wide).
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spacing_mut().slider_width = 90.0;
                            if ui.add(egui::Slider::new(&mut volume, 0.0..=1.0).show_value(false)).changed() {
                                audio = true;
                            }
                            if theme::icon_button(ui, if muted { icon::MUTED } else { icon::VOLUME }, if muted { "Sound on" } else { "Mute" }, false).clicked() {
                                muted = !muted;
                                audio = true;
                            }
                            ui.spacing_mut().slider_width = ui.available_width() - 8.0;
                            let mut at = status.position;
                            if ui.add(egui::Slider::new(&mut at, 0.0..=status.duration.max(0.1)).show_value(false).trailing_fill(true)).changed() {
                                seek = Some(at);
                            }
                        });
                    });
                }
            });
        });
        self.player_rect = shown.response.rect;
        if let Some(player) = &self.video {
            if let Some(p) = pause {
                player.set_paused(p);
            }
            if let Some(t) = seek {
                player.seek(t);
            }
            if audio {
                player.set_volume(volume, muted);
                self.set("videoVolume", json!(volume));
                self.set("videoMuted", json!(muted));
            }
        }
    }

    /// Everything else of the slideshow, on glass that slides in from the left edge of the window when the pointer
    /// goes there, and over the picture. Tags (the button or its hotkey) pin it, and the picture then makes room.
    fn drawer(&mut self, ctx: &egui::Context) {
        let area = ctx.content_rect();
        let pointer = ctx.input(|i| i.pointer.hover_pos());
        let at_edge = pointer.is_some_and(|p| p.x <= area.left() + DRAWER_EDGE);
        let on_it = self.drawer_open && pointer.is_some_and(|p| p.x <= area.left() + DRAWER_SPACE + theme::S4);
        let held = self.drawer_open && (egui::Popup::is_any_open(ctx) || ctx.egui_wants_keyboard_input());
        self.drawer_open = self.show_tags || self.drawer_forced || at_edge || on_it || held;
        let k = ctx.animate_bool_with_time(egui::Id::new("drawer"), self.drawer_open, theme::smooth());
        let t = tok(ctx);
        if k < 1.0 && self.show_controls && self.player_rect.is_positive() {
            // Where it is: a short mark at the edge, with the player.
            let mark = egui::Rect::from_center_size(egui::pos2(area.left() + 4.0, area.center().y), Vec2::new(3.0, 44.0));
            ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("drawer mark"))).rect_filled(mark, 2, t.hover);
        }
        if k == 0.0 {
            return;
        }
        let x = egui::lerp(-(DRAWER_W + theme::S4)..=RAIL_MARGIN, k);
        let height = area.height() - 2.0 * RAIL_MARGIN;
        egui::Area::new(egui::Id::new("drawer area")).order(egui::Order::Middle).fixed_pos(egui::pos2(area.left() + x, area.top() + RAIL_MARGIN)).constrain(false).show(ctx, |ui| {
            glass::card(ui, glass::bar(ctx), egui::Margin::same(12), |ui| {
                ui.set_width(DRAWER_W - 24.0);
                ui.set_height(height - 24.0);
                self.drawer_content(ui);
            });
        });
    }

    fn drawer_content(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let t = tok(&ctx);
        // The search of what is showing, or the filter of the favorites.
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| if self.mode == Mode::Slideshow { self.search_field(ui) } else { self.favorites_field(ui) });
        });
        ui.horizontal_wrapped(|ui| {
            if self.mode == Mode::Slideshow {
                self.sites_menu(ui);
                self.search_extras(ui);
            } else {
                self.favorites_extras(ui);
            }
        });
        ui.add_space(theme::S1);
        ui.separator();
        // What can be done with the slide.
        ui.horizontal(|ui| {
            let faved = self.current().is_some_and(|s| self.fav.contains(&s));
            let armed = self.current().is_some_and(|s| self.unfave_armed(&s));
            if let Some((_, at)) = self.unfave.as_ref().filter(|_| armed) {
                // Back to ♥ when the wait is over.
                ui.ctx().request_repaint_after(UNFAVE_WAIT.saturating_sub(at.elapsed()));
            }
            let tip = if armed { "Press again to remove it" } else if faved { "Remove from the favorites" } else { "Add to the favorites" };
            if theme::icon_button_tinted(ui, icon::HEART, tip, armed, faved.then_some(t.accent)).clicked() {
                self.toggle_favorite();
            }
            if self.flag("showDownloadButton", true) && theme::icon_button(ui, icon::DOWNLOAD, "Download", false).clicked() {
                self.download_current();
            }
            if theme::icon_button(ui, icon::POST, "Open the post in the browser", false).clicked() {
                self.open_source();
            }
            if theme::icon_button(ui, icon::TAGS, "Keep this panel open beside the picture", self.show_tags).clicked() {
                self.show_tags = !self.show_tags;
                self.set("showTags", json!(self.show_tags));
            }
            let thumbs = self.flag("showThumbs", true);
            if theme::icon_button(ui, icon::THUMBS, "Thumbnails of the next slides", thumbs).clicked() {
                self.set("showThumbs", json!(!thumbs));
            }
            let fit = self.flag("autoFitSlide", true);
            if theme::icon_button(ui, icon::FIT, "Fit the picture to the window", fit).clicked() {
                self.set("autoFitSlide", json!(!fit));
            }
        });
        if !self.progress.is_empty() {
            ui.label(RichText::new(&self.progress).small().color(t.text2));
        }
        ui.separator();
        // The tags take the height the pages leave.
        const PAGES: f32 = 5.0 * 36.0 + 12.0;
        if let Some(slide) = self.current() {
            let room = Vec2::new(ui.available_width(), (ui.available_height() - PAGES).max(0.0));
            ui.allocate_ui(room, |ui| {
                ui.set_min_height(room.y);
                self.tags_panel(ui, &slide);
            });
        }
        ui.separator();
        let (mosaic_ok, favorites) = (!self.list().is_empty(), self.mode == Mode::Favorites);
        for (glyph, label, action, on, enabled) in [
            (icon::HOME, "Home", "home", false, true),
            (icon::MOSAIC, "Mosaic", "", false, mosaic_ok),
            (icon::POOLS, "Pools", "openPools", false, true),
            (icon::HEART, "Favorites", "openFavorites", favorites, true),
            (icon::SETTINGS, "Settings", "openSettings", false, true),
        ] {
            if ui.add_enabled_ui(enabled, |ui| theme::drawer_row(ui, glyph, label, on)).inner.clicked() {
                if action.is_empty() {
                    self.show_mosaic = true;
                    self.mosaic_seek = true;
                } else {
                    self.act(&ctx, action);
                }
            }
        }
    }

    fn tags_panel(&mut self, ui: &mut egui::Ui, slide: &Slide) {
        let mut clicked: Option<String> = None;
        egui::ScrollArea::vertical().id_salt("tags").auto_shrink(false).show(ui, |ui| {
            let t = tok(ui.ctx());
            let groups = super::slide::tag_groups(slide);
            for (category, tags) in groups {
                if !category.is_empty() {
                    ui.label(RichText::new(category.to_uppercase()).font(theme::medium(10.5)).color(t.muted));
                }
                for tag in tags {
                    let text = RichText::new(tag.replace('_', " ")).color(category_color(&category, t));
                    if ui.add(egui::Label::new(text).sense(Sense::click())).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        clicked = Some(tag);
                    }
                }
                ui.add_space(theme::S2);
            }
        });
        if let Some(tag) = clicked {
            self.toggle_tag_search(&tag);
        }
    }

    /// One picture (or a still of one) in `rect`, if it is at hand.
    fn draw_picture(&mut self, ui: &egui::Ui, rect: egui::Rect, url: &str, alpha: f32, elapsed: Duration) {
        if let Some(State::Ready(picture)) = self.media.get(url) {
            media::paint(ui, rect, &picture.frame_at(elapsed).texture, picture.size, alpha, 0.0);
            if let Some(after) = picture.next_frame_in(elapsed) {
                ui.ctx().request_repaint_after(after);
            }
        }
    }

    fn viewer(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, Sense::click());
        let Some(slide) = self.current() else {
            self.on_screen = OnScreen::default();
            self.front_page(ui, rect);
            return;
        };
        let t = tok(ui.ctx());
        let url = self.copies_or(&slide.file_url);
        let faved = self.fav.contains(&slide);
        if slide.media_type == MediaType::Video {
            self.on_screen = OnScreen::default();
            self.video_view(ui, rect, &response);
            if faved {
                fave_mark(ui, rect);
            }
            return;
        }
        let elapsed = self.shown_at.map(|t| t.elapsed()).unwrap_or_default();
        let thumb = thumb_source(&self.copies_or(&slide.preview_file_url));
        // The picture, or until it has come the thumbnail, which is usually here already.
        let (mut showing, mut place) = (String::new(), egui::Rect::NOTHING);
        match self.media.get(&url) {
            Some(State::Ready(picture)) => {
                let (w, h) = (picture.size[0] as f32, picture.size[1] as f32);
                let (max_w, max_h) = (self.number("maxWidth", f64::INFINITY) as f32, self.number("maxHeight", f64::INFINITY) as f32);
                let scale = if self.flag("autoFitSlide", true) { (rect.width() / w).min(rect.height() / h) } else { 1.0f32.min(max_w / w).min(max_h / h) };
                showing = url.clone();
                place = egui::Rect::from_center_size(rect.center(), Vec2::new(w * scale, h * scale));
            }
            Some(State::Failed(e)) => {
                let text = format!("Couldn't load the picture ({e})");
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(16.0), t.text2);
            }
            _ => {
                if let Some(State::Ready(p)) = self.media.get(&thumb) {
                    let (w, h) = (p.size[0] as f32, p.size[1] as f32);
                    let scale = (rect.width() / w).min(rect.height() / h);
                    showing = thumb.clone();
                    place = egui::Rect::from_center_size(rect.center(), Vec2::new(w * scale, h * scale));
                }
                ui.put(egui::Rect::from_center_size(rect.center(), Vec2::splat(36.0)), egui::Spinner::new().size(36.0).color(t.accent));
            }
        }
        if showing.is_empty() {
            self.on_screen = OnScreen::default();
            return;
        }
        if showing != self.on_screen.url {
            let was = std::mem::take(&mut self.on_screen);
            self.on_screen = OnScreen { url: showing.clone(), was: was.url, was_rect: was.rect, rect: place, at: Some(Instant::now()) };
        }
        self.on_screen.rect = place;
        let k = match (self.on_screen.at, theme::smooth() > 0.0 && !self.on_screen.was.is_empty()) {
            (Some(at), true) => (at.elapsed().as_secs_f32() / theme::smooth()).min(1.0),
            _ => 1.0,
        };
        if k < 1.0 {
            ui.ctx().request_repaint();
            let (was, was_rect) = (self.on_screen.was.clone(), self.on_screen.was_rect);
            // The old one stays nearly whole until the new one covers it: no dip towards the background halfway.
            self.draw_picture(ui, was_rect, &was, 1.0 - k * k * k, elapsed);
        }
        self.draw_picture(ui, place, &showing, k, elapsed);
        if faved {
            fave_mark(ui, place);
        }
        if showing == url && response.clicked() && self.mode == Mode::Favorites && self.flag("clickOpensPost", true) {
            self.open_source();
        }
    }

    fn video_view(&mut self, ui: &mut egui::Ui, rect: egui::Rect, response: &egui::Response) {
        let Some(player) = &self.video else {
            let text = self.video_error.clone().unwrap_or_else(|| "Video playback is not available.".into());
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(16.0), tok(ui.ctx()).text2);
            if response.clicked() {
                self.open_source();
            }
            return;
        };
        player.paint(ui, rect);
        if response.clicked() {
            player.set_paused(!player.status().paused);
        }
        ui.ctx().request_repaint_after(Duration::from_millis(250));
    }

    fn front_page(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let t = tok(ui.ctx());
        if self.mode == Mode::Favorites {
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "No favorites to show.", egui::FontId::proportional(16.0), t.text2);
            return;
        }
        let background = self.text("backgroundImage");
        if !background.is_empty() {
            self.media.request(&self.engine, &background);
            if let Some(State::Ready(p)) = self.media.get(&background) {
                let (w, h) = (p.size[0] as f32, p.size[1] as f32);
                let scale = (rect.width() / w).max(rect.height() / h);
                let size = Vec2::new(w * scale, h * scale);
                let painter = ui.painter().with_clip_rect(rect);
                painter.image(p.frames[0].texture.id(), egui::Rect::from_center_size(rect.center(), size), egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::from_white_alpha(120));
            }
        }
        let searching = self.search.as_ref().is_some_and(|s| s.loading);
        let cards = !searching && self.search.is_none() && self.flag("quickCards", true) && self.quick_cards(ui, rect);
        if !cards {
            let text = if searching { "Searching…" } else { "Type a search above." };
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(18.0), t.text2);
        }
    }

    /// The quick searches in the mosaic's columns: the first picture of the results with the name on glass over it.
    /// Click runs the search. Says whether there are any.
    fn quick_cards(&mut self, ui: &mut egui::Ui, rect: egui::Rect) -> bool {
        let quick: Vec<Value> = self.store.get("quickSearches").and_then(|v| v.as_array().cloned()).unwrap_or_default();
        let cards: Vec<(usize, Vec<Site>, String)> = quick
            .iter()
            .enumerate()
            .filter_map(|(n, q)| {
                let sites: Vec<Site> = q["sites"].as_array()?.iter().filter_map(|s| s.as_str().and_then(Site::from_id)).collect();
                (!sites.is_empty()).then(|| (n, sites, q["tags"].as_str().unwrap_or("").to_string()))
            })
            .collect();
        if cards.is_empty() {
            return false;
        }
        if !self.covers_asked && !self.flag("offlineMode", false) {
            self.covers_asked = true;
            for (n, sites, tags) in &cards {
                let (engine, tx, sites, tags, n) = (self.engine.clone(), self.tx.clone(), sites.clone(), tags.clone(), *n);
                self.engine.rt.spawn(async move {
                    if let Some(slide) = session::first_slide(&engine, sites, &tags).await {
                        let _ = tx.send(Msg::Cover { index: n, slide });
                        engine.wake();
                    }
                });
            }
        }
        // A picture is at least this tall for its width, so that the glass plate fits on it.
        let ratios: Vec<f32> = cards.iter().map(|(n, ..)| self.covers.get(n).filter(|s| s.width > 0 && s.height > 0).map_or(0.8, |s| (s.height as f32 / s.width as f32).max(0.6))).collect();
        let target = (self.number("readingThumbWidth", 220.0) as f32).max(260.0);
        let (cells, total, column) = masonry(&ratios, rect.width() - theme::S2, target);
        let big = column * ui.ctx().pixels_per_point() > 200.0;
        let t = tok(ui.ctx());
        let mut run = None;
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            egui::ScrollArea::vertical().id_salt("saved searches").auto_shrink(false).show_viewport(ui, |ui, viewport| {
                ui.set_height(total);
                let origin = ui.max_rect().min.to_vec2();
                for (i, (n, sites, tags)) in cards.iter().enumerate().filter(|(i, _)| cells[*i].intersects(viewport)) {
                    let tile = cells[i].translate(origin);
                    let id = ui.id().with(("saved", *n));
                    let cover = self.covers.get(n).cloned();
                    let url = cover.as_ref().map_or(String::new(), |s| s.sample_file_url.clone().filter(|u| big && !u.is_empty()).unwrap_or_else(|| s.preview_file_url.clone()));
                    let title = if tags.is_empty() { "(what is in the box)".to_string() } else { tags.clone() };
                    if self.tile(ui, tile, id, &url).on_hover_text(&title).clicked() {
                        run = Some(*n);
                    }
                    theme::badge(ui, egui::Rect::from_min_size(tile.min + Vec2::splat(6.0), Vec2::splat(24.0)), ((n + 1) % 10).to_string(), theme::medium(12.5), Color32::WHITE);
                    let plate = egui::Rect::from_min_max(egui::pos2(tile.left() + 6.0, tile.bottom() - 66.0), tile.right_bottom() - Vec2::splat(6.0));
                    ui.painter().add(glass::frost(ui.ctx(), plate, glass::Look { radius: theme::RADIUS_M as f32, tint: t.glass, passes: 3 }, 1.0));
                    let names: Vec<&str> = sites.iter().map(|s| s.name()).collect();
                    let main_tags = cover.and_then(|s| s.tag_groups).map_or(String::new(), |groups| {
                        ["artist", "character", "copyright"].iter().filter_map(|c| groups.get(*c)?.as_array()).flatten().filter_map(Value::as_str).take(3).collect::<Vec<_>>().join(" ").replace('_', " ")
                    });
                    // One row each, cut with … at the plate's edge.
                    let line = |text: String, y: f32, font: egui::FontId, color: Color32| {
                        let mut job = egui::text::LayoutJob::simple(text, font, color, plate.width() - 20.0);
                        job.wrap.max_rows = 1;
                        job.wrap.break_anywhere = true;
                        ui.painter().galley(egui::pos2(plate.left() + 10.0, plate.top() + y), ui.painter().layout_job(job), color);
                    };
                    line(title, 7.0, theme::semibold(14.0), t.text);
                    line(names.join(", "), 27.0, egui::FontId::proportional(11.5), t.text2);
                    line(main_tags, 42.0, egui::FontId::proportional(11.5), t.muted);
                }
            });
        });
        if let Some(n) = run {
            self.run_quick_search(n);
        }
        true
    }

    /// A new screen comes up out of a short dimming.
    fn page_fade(&mut self, ctx: &egui::Context) {
        let Some(at) = self.changed_at else { return };
        let k = if theme::smooth() > 0.0 { at.elapsed().as_secs_f32() / theme::smooth() } else { 1.0 };
        if k >= 1.0 {
            self.changed_at = None;
            return;
        }
        ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("page fade"))).rect_filled(ctx.content_rect(), 0.0, tok(ctx).base.gamma_multiply(0.5 * (1.0 - k)));
        ctx.request_repaint();
    }

    fn test_script(&mut self, ctx: &egui::Context) {
        let dir = std::path::PathBuf::from(std::env::var_os("MSG_SHOT_DIR").unwrap_or_default());
        for event in ctx.input(|i| i.events.clone()) {
            if let egui::Event::Screenshot { image, user_data, .. } = event {
                if let Some(name) = user_data.data.as_ref().and_then(|d| d.downcast_ref::<String>()) {
                    let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
                    let _ = image::save_buffer(dir.join(format!("{name}.png")), &bytes, image.size[0] as u32, image.size[1] as u32, image::ColorType::Rgba8);
                }
            }
        }
        let Some((started, steps)) = self.script.as_mut() else { return };
        let now = started.elapsed().as_secs_f64();
        let due: Vec<String> = steps.iter().take_while(|(t, _)| *t <= now).map(|(_, a)| a.clone()).collect();
        steps.drain(..due.len());
        for action in due {
            match action.split_once('=') {
                Some(("shot", name)) => ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(name.to_string()))),
                Some(("tab", name)) => self.open_tab(name),
                Some(("size", size)) => {
                    if let Some((w, h)) = size.split_once('x').and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?))) {
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(w, h)));
                    }
                }
                Some(("theme", name)) => {
                    self.set("appTheme", json!(name));
                    self.apply_theme(ctx);
                }
                _ if action == "drawer" => self.drawer_forced = !self.drawer_forced,
                _ if action == "next" => self.move_by(1),
                _ if action == "prev" => self.move_by(-1),
                _ if action == "quit" => std::process::exit(0),
                _ => self.act(ctx, &action),
            }
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    fn test_hook(&mut self, ctx: &egui::Context) {
        let Some((path, started, sent)) = self.hook.as_mut() else { return };
        for event in ctx.input(|i| i.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = event {
                let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
                let _ = image::save_buffer(&*path, &bytes, image.size[0] as u32, image.size[1] as u32, image::ColorType::Rgba8);
                self.store.flush();
                std::process::exit(0);
            }
        }
        let after: u64 = std::env::var("MSG_SHOT_AFTER").ok().and_then(|v| v.parse().ok()).unwrap_or(12);
        if !*sent && started.elapsed() > Duration::from_secs(after) {
            *sent = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    fn toasts(&mut self, ctx: &egui::Context) {
        self.notices.retain(|(_, at)| at.elapsed() < Duration::from_secs(9));
        if self.notices.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::new("toasts")).anchor(egui::Align2::LEFT_BOTTOM, [12.0, -80.0]).show(ctx, |ui| {
            for (text, _) in &self.notices {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(520.0);
                    ui.label(text);
                });
            }
        });
        ctx.request_repaint_after(Duration::from_secs(1));
    }
}

fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

fn net_ok(url: &str) -> bool {
    super::net::web_url(url).is_some()
}

/// Sites' thumbnails are small; a local file stands in as it is.
fn thumb_source(url: &str) -> String {
    url.to_string()
}

pub fn paint_fit(ui: &egui::Ui, rect: egui::Rect, texture: &egui::TextureHandle, size: [usize; 2], alpha: f32, radius: f32) {
    let (w, h) = (size[0] as f32, size[1] as f32);
    let scale = (rect.width() / w).min(rect.height() / h);
    media::paint(ui, egui::Rect::from_center_size(rect.center(), Vec2::new(w * scale, h * scale)), texture, size, alpha, radius);
}

/// The heart of a favorite, in the corner of its picture.
fn fave_mark(ui: &egui::Ui, picture: egui::Rect) {
    let t = tok(ui.ctx());
    let box_ = egui::Rect::from_min_size(egui::pos2(picture.right() - 36.0, picture.top() + 10.0), Vec2::splat(26.0));
    theme::badge(ui, box_, theme::heart_text(true), theme::heart_font(true, 14.0), t.accent);
}

fn category_color(category: &str, t: &theme::Tokens) -> Color32 {
    match category {
        "artist" => t.apricot,
        "copyright" => t.lavender,
        "character" => Color32::from_rgb(0x7f, 0xc9, 0x9b),
        "species" => t.accent,
        "meta" => t.muted,
        "lore" => Color32::from_rgb(0x8f, 0xb8, 0x8a),
        "invalid" => Color32::from_rgb(0xff, 0x6b, 0x6b),
        _ => t.text2,
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Whatever changed the screen last frame (a button, a hotkey, a search) is noted here, before Esc is read.
        let now = self.screen();
        if now != self.here {
            self.changed_at = Some(Instant::now());
        }
        if let Some(window) = self.window.filter(|_| self.title_theme != Some(ctx.theme())) {
            self.title_theme = Some(ctx.theme());
            theme::paint_title_bar(window, tok(&ctx), ctx.theme() == egui::Theme::Dark);
        }
        note(&mut self.back, &mut self.here, now, self.replaced.take());
        // A key being recorded stops the hotkeys, so it ends whichever way the settings page is left.
        if !self.show_settings {
            self.recording = None;
        }
        self.media.poll(&ctx);
        self.handle_messages();
        self.handle_keys(&ctx);
        self.tick(&ctx);
        self.preload();

        self.ambient(ui, ctx.content_rect());
        // The pages have the rail, and a toolbar over them (settings over pools over the mosaic; Back returns there).
        if self.covered() || self.current().is_none() {
            egui::Panel::left("rail").exact_size(RAIL_SPACE).resizable(false).show_separator_line(false).frame(egui::Frame::NONE).show(ui, |ui| self.rail(ui));
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| {
                if self.show_settings {
                    self.settings_page(ui);
                } else if self.show_pools {
                    ui.add_space(TOP_INSET);
                    self.pools_page(ui);
                } else if self.show_mosaic {
                    self.mosaic_page(ui);
                } else {
                    self.viewer(ui);
                }
            });
            if !self.show_settings {
                self.toolbar(&ctx);
            }
        } else {
            if self.show_tags {
                egui::Panel::left("pinned drawer").exact_size(DRAWER_SPACE).resizable(false).show_separator_line(false).frame(egui::Frame::NONE).show(ui, |_| {});
            }
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| self.viewer(ui));
            self.player(&ctx);
            self.drawer(&ctx);
        }
        self.page_fade(&ctx);
        self.test_hook(&ctx);
        self.test_script(&ctx);
        self.toasts(&ctx);
    }

    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        if let Some(player) = self.video.as_mut() {
            player.shutdown(gl);
        }
        media::clear_video_files();
        self.store.flush();
    }
}

// So the settings page (another file) can name it.
pub fn user_dir() -> std::path::PathBuf {
    files::user_data()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_a_page_is_a_step_back_and_a_replaced_search_is_kept() {
        let home = Screen::default();
        let page = Screen { settings: true, ..Screen::default() };
        let a = Screen { search: Some("a".into()), ..Screen::default() };
        let (mut back, mut here) = (vec![], home.clone());

        note(&mut back, &mut here, page.clone(), None::<u8>);
        assert!(back.len() == 1 && here == page);
        note(&mut back, &mut here, home.clone(), None);
        assert!(back.is_empty() && here == home);

        // Home from a search looks like returning, but the search can still be gone back to.
        note(&mut back, &mut here, a.clone(), None);
        note(&mut back, &mut here, home.clone(), Some(1));
        assert!(back.len() == 2 && back[1].0 == a && back[1].1 == Some(1));

        // The same screen with its search read again is no new screen.
        note(&mut back, &mut here, home, Some(2));
        assert_eq!(back.len(), 2);
    }
}
