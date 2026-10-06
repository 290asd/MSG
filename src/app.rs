// The window: search bar, the picture, the controls, the tags and the thumbnails. The state of the
// slideshow lives here; the settings window is in settings_ui.rs.
use super::downloads::{self, Downloader};
use super::favorites::{self, Favorites};
use super::files;
use super::hotkeys::Hotkeys;
use super::media::{self, MediaCache, State};
use super::session::{self, Engine, Msg, Search};
use super::sites::Site;
use super::video::Player;
use super::slide::{MediaType, Slide};
use crate::store::Store;
use eframe::egui::{self, Color32, RichText, Sense, Vec2};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How many of the next slides are loaded ahead (their thumbnails, and the first few pictures).
const AHEAD: usize = 12;
/// A thumbnail in the strip: its width and the space between two.
const THUMB_STEP: f32 = 56.0 + 8.0;

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Slideshow,
    Favorites,
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
    pub notices: Vec<(String, Instant)>,
    pub progress: String,
    pub recording: Option<(String, usize)>,
    pub pool_sort: String,
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
            notices: vec![],
            progress: String::new(),
            recording: None,
            pool_sort: "added".into(),
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
        app.apply_theme(&cc.egui_ctx);
        app.refresh_copies();
        app.downloader.schedule(Duration::from_secs(15));
        if let Ok(text) = std::env::var("MSG_SEARCH") {
            app.start_search(&text);
        }
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

    fn list(&self) -> &[Slide] {
        match self.mode {
            Mode::Slideshow => self.search.as_ref().map(|s| s.slides.as_slice()).unwrap_or(&[]),
            Mode::Favorites => &self.fav.view,
        }
    }

    fn idx(&self) -> usize {
        self.cur[self.mode as usize]
    }

    fn set_idx(&mut self, i: usize) {
        let len = self.list().len();
        self.cur[self.mode as usize] = if len == 0 { 0 } else { i.min(len - 1) };
        self.shown_at = None;
    }

    pub fn current(&self) -> Option<Slide> {
        self.list().get(self.idx()).cloned()
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

    // ---------- searching ----------

    pub fn start_search(&mut self, text: &str) {
        self.mode = Mode::Slideshow;
        self.generation += 1;
        let text = text.trim();
        self.query = text.to_string();
        let sites = session::ticked_sites(&self.store);
        if sites.is_empty() {
            self.notice("Tick at least one site (Sites ▾).");
            return;
        }
        self.search = Some(Search::new(self.generation, text, sites, &self.store));
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

    fn load_more(&mut self) {
        if let Some(search) = &mut self.search {
            search.load_more(&self.engine, self.tx.clone());
        }
    }

    fn handle_messages(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Batch(batch) => {
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
            }
        }
    }

    // ---------- actions ----------

    pub fn toggle_favorite(&mut self) {
        let Some(slide) = self.current() else { return };
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
        } else {
            let text = if add && !self.query.is_empty() { format!("{} {tag}", self.query) } else { tag.to_string() };
            self.start_search(&text);
        }
    }

    pub fn apply_favorite_filter(&mut self) {
        self.fav.filter = self.fav_filter_box.trim().to_string();
        self.fav.random = false;
        self.rebuild_favorites();
        self.cur[Mode::Favorites as usize] = 0;
        self.shown_at = None;
    }

    pub fn open_favorites(&mut self, on: bool) {
        self.mode = if on { Mode::Favorites } else { Mode::Slideshow };
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
            "openFavorites" => self.open_favorites(self.mode == Mode::Slideshow),
            "openSettings" => self.show_settings = !self.show_settings,
            "setBackground" => self.use_as_background(),
            "showInterface" => self.show_controls = !self.show_controls,
            // Back to the front page (the quick search cards); a new search starts from there as before.
            "home" => {
                self.mode = Mode::Slideshow;
                self.search = None;
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
        for action in self.hotkeys.pressed(&events, wants_text) {
            self.act(ctx, action);
        }
        // Fixed keys: Enter also plays and pauses, F11 is fullscreen, Esc closes things, 1–0 are the quick searches.
        for event in &events {
            if let egui::Event::Key { key, pressed: true, repeat: false, modifiers, .. } = event {
                match key {
                    egui::Key::F11 => {
                        let full = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!full));
                    }
                    egui::Key::Escape => {
                        if self.show_settings {
                            self.show_settings = false;
                        } else if self.show_pools {
                            self.show_pools = false;
                        } else if ctx.input(|i| i.viewport().fullscreen.unwrap_or(false)) {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
                        }
                    }
                    egui::Key::Enter if !wants_text && !modifiers.any() => self.playing = !self.playing,
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
                player.load(&url, referer, !to_end, autoplay);
            }
            None => player.stop(),
            _ => {}
        }
    }

    fn tick(&mut self, ctx: &egui::Context) {
        let current = self.current();
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
            if let Some(player) = &self.video {
                if player.status().ended {
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
        if self.mode == Mode::Slideshow && self.search.as_ref().is_some_and(|s| s.has_more() && s.slides.len() < i + 25) {
            self.load_more();
        }
    }

    // ---------- drawing ----------

    /// One row: the buttons are laid out from the right end, and the search box takes the width that is left.
    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("🏠").on_hover_text("Front page").clicked() {
                self.act(&ui.ctx().clone(), "home");
            }
            if self.mode == Mode::Slideshow {
                self.sites_menu(ui);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("⚙").clicked() {
                    self.show_settings = !self.show_settings;
                }
                let label = if self.mode == Mode::Favorites { "◀ Slideshow" } else { "♥ Favorites" };
                if ui.button(label).clicked() {
                    self.open_favorites(self.mode == Mode::Slideshow);
                }
                if self.mode == Mode::Slideshow {
                    self.search_bar(ui);
                } else {
                    self.favorites_bar(ui);
                }
            });
        });
    }

    /// Right to left (see top_bar).
    fn search_bar(&mut self, ui: &mut egui::Ui) {
        if ui.button("📚 Pools").clicked() {
            self.show_pools = !self.show_pools;
        }
        if let Some(pool) = self.search.as_ref().and_then(|s| s.pool()) {
            if ui.button("☆ Save pool").clicked() {
                let cover = self.search.as_ref().and_then(|s| s.slides.first().map(|x| x.preview_file_url.clone())).unwrap_or_default();
                self.save_pool(pool.0, pool.1, pool.2, cover);
            }
        }
        if self.flag("searchSortMenu", false) {
            self.sort_menu(ui);
        }
        let search = ui.button("Search").clicked();
        let history: Vec<String> = self.store.get("searchHistory").and_then(|v| v.as_array().cloned()).unwrap_or_default().iter().rev().filter_map(|v| v.as_str().map(String::from)).collect();
        if !history.is_empty() {
            ui.menu_button("🕘", |ui| {
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
        let response = ui.add(egui::TextEdit::singleline(&mut query).hint_text("Search tags…").desired_width(f32::INFINITY));
        self.query = query;
        let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if search || enter {
            let text = self.query.clone();
            self.start_search(&text);
        }
    }

    fn sites_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Sites", |ui| {
            let ticked = session::ticked_sites(&self.store);
            for site in Site::ALL {
                let mut on = ticked.contains(&site);
                if ui.checkbox(&mut on, site.name()).changed() {
                    let map: serde_json::Map<String, Value> = Site::ALL.iter().map(|s| (s.id().to_string(), json!(if *s == site { on } else { ticked.contains(s) }))).collect();
                    self.set("sitesToSearch", Value::Object(map));
                }
            }
        });
        ui.menu_button("Filters", |ui| {
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

    /// Right to left (see top_bar).
    fn favorites_bar(&mut self, ui: &mut egui::Ui) {
        ui.label(format!("{} shown", self.fav.view.len()));
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
        if ui.button("🔀 Random").clicked() {
            self.fav.filter = self.fav_filter_box.trim().to_string();
            self.fav.random = true;
            self.rebuild_favorites();
            self.cur[Mode::Favorites as usize] = 0;
            self.shown_at = None;
        }
        let filter_clicked = ui.button("Filter").clicked();
        let mut filter = std::mem::take(&mut self.fav_filter_box);
        let response = ui.add(egui::TextEdit::singleline(&mut filter).hint_text("Filter by tags (-tag, ~tag, tag*)").desired_width(f32::INFINITY));
        self.fav_filter_box = filter;
        let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if filter_clicked || enter {
            self.apply_favorite_filter();
        }
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        let len = self.list().len();
        // The next slides as thumbnails.
        let i = self.idx();
        // As many as fit in the window (and two more, for the scrolling), so only those are loaded.
        let fit = (ui.available_width() / THUMB_STEP).ceil() as usize + 2;
        let thumbs: Vec<(usize, String)> = self.list().iter().enumerate().skip(i + 1).take(fit).map(|(n, s)| (n, s.preview_file_url.clone())).collect();
        if !thumbs.is_empty() && self.flag("showThumbs", true) {
            let mut jump = None;
            egui::ScrollArea::horizontal().id_salt("thumbs").show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (n, url) in &thumbs {
                        let url = thumb_source(&self.copies_or(url));
                        // Also asks for what is missing (the list may have grown); a thumbnail in use is not let go.
                        self.media.request(&self.engine, &url);
                        let size = Vec2::new(THUMB_STEP - 8.0, 42.0);
                        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
                        ui.painter().rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
                        if let Some(State::Ready(p)) = self.media.get(&url) {
                            paint_fit(ui, rect, &p.frames[0].texture, p.size);
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
        ui.horizontal_wrapped(|ui| {
            if ui.button("⏮").clicked() {
                self.move_by(-1);
            }
            if ui.button(if self.playing { "⏸" } else { "▶" }).clicked() {
                self.playing = !self.playing;
            }
            if ui.button("⏭").clicked() {
                self.move_by(1);
            }
            ui.label(if len == 0 { "0 / 0".to_string() } else { format!("{} / {}{}", i + 1, len, if self.search.as_ref().is_some_and(|s| s.has_more()) && self.mode == Mode::Slideshow { "+" } else { "" }) });
            let mut secs = self.number("secondsPerSlide", 6.0);
            if ui.add(egui::DragValue::new(&mut secs).range(1.0..=600.0).suffix(" s")).changed() {
                self.set("secondsPerSlide", json!(secs));
            }
            let faved = self.current().is_some_and(|s| self.fav.contains(&s));
            if ui.button(RichText::new(if faved { "♥" } else { "♡" }).color(if faved { Color32::from_rgb(230, 70, 90) } else { ui.visuals().text_color() })).clicked() {
                self.toggle_favorite();
            }
            if self.flag("showDownloadButton", true) && ui.button("💾 Download").clicked() {
                self.download_current();
            }
            if ui.button("Post").clicked() {
                self.open_source();
            }
            if ui.selectable_label(self.show_tags, "# Tags").clicked() {
                self.show_tags = !self.show_tags;
                self.set("showTags", json!(self.show_tags));
            }
            let thumbs = self.flag("showThumbs", true);
            if ui.selectable_label(thumbs, "Thumbnails").clicked() {
                self.set("showThumbs", json!(!thumbs));
            }
            let mut fit = self.flag("autoFitSlide", true);
            if ui.checkbox(&mut fit, "Fit").changed() {
                self.set("autoFitSlide", json!(fit));
            }
            if !self.progress.is_empty() {
                ui.label(RichText::new(&self.progress).weak());
            }
        });
    }

    fn tags_panel(&mut self, ui: &mut egui::Ui, slide: &Slide) {
        let mut clicked: Option<String> = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            let groups = super::slide::tag_groups(slide);
            for (category, tags) in groups {
                if !category.is_empty() {
                    ui.label(RichText::new(&category).strong());
                }
                for tag in tags {
                    let text = RichText::new(tag.replace('_', " ")).color(category_color(&category, ui.visuals().dark_mode));
                    if ui.add(egui::Label::new(text).sense(Sense::click())).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        clicked = Some(tag);
                    }
                }
                ui.add_space(6.0);
            }
        });
        if let Some(tag) = clicked {
            self.toggle_tag_search(&tag);
        }
    }

    fn viewer(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, Sense::click());
        let Some(slide) = self.current() else {
            self.front_page(ui, rect);
            return;
        };
        let url = self.copies_or(&slide.file_url);
        if slide.media_type == MediaType::Video {
            self.video_view(ui, rect, &response);
            return;
        }
        let fit = self.flag("autoFitSlide", true);
        let (max_w, max_h) = (self.number("maxWidth", f64::INFINITY) as f32, self.number("maxHeight", f64::INFINITY) as f32);
        let (faved, opens_post, elapsed) = (self.fav.contains(&slide), self.mode == Mode::Favorites && self.flag("clickOpensPost", true), self.shown_at.map(|t| t.elapsed()).unwrap_or_default());
        let mut open_post = false;
        match self.media.get(&url) {
            Some(State::Ready(picture)) => {
                let frame = picture.frame_at(elapsed);
                let (w, h) = (picture.size[0] as f32, picture.size[1] as f32);
                let scale = if fit { (rect.width() / w).min(rect.height() / h) } else { 1.0f32.min(max_w / w).min(max_h / h) };
                let image_rect = egui::Rect::from_center_size(rect.center(), Vec2::new(w * scale, h * scale));
                media::paint(ui, image_rect, &frame.texture, picture.size);
                if faved {
                    ui.painter().rect_stroke(image_rect, 0.0, egui::Stroke::new(2.0, Color32::from_rgb(230, 70, 90)), egui::StrokeKind::Outside);
                }
                if let Some(after) = picture.next_frame_in(elapsed) {
                    ui.ctx().request_repaint_after(after);
                }
                open_post = response.clicked() && opens_post;
            }
            Some(State::Failed(e)) => {
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, format!("Couldn't load the picture ({e})"), egui::FontId::proportional(18.0), Color32::GRAY);
            }
            _ => {
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "Loading…", egui::FontId::proportional(18.0), Color32::GRAY);
                ui.ctx().request_repaint_after(Duration::from_millis(200));
            }
        }
        if open_post {
            self.open_source();
        }
    }

    fn video_view(&mut self, ui: &mut egui::Ui, rect: egui::Rect, response: &egui::Response) {
        let Some(player) = &self.video else {
            let text = self.video_error.clone().unwrap_or_else(|| "Video playback is not available.".into());
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(16.0), Color32::GRAY);
            if response.clicked() {
                self.open_source();
            }
            return;
        };
        player.paint(ui, rect);
        let status = player.status();
        let (mut volume, mut muted) = (self.number("videoVolume", 0.5), self.flag("videoMuted", false));
        let (mut pause, mut seek, mut audio) = (None, None, false);
        egui::Area::new(egui::Id::new("videobar")).fixed_pos(egui::pos2(rect.left() + 12.0, rect.bottom() - 44.0)).show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_width((rect.width() - 48.0).max(200.0));
                ui.horizontal(|ui| {
                    if ui.button(if status.paused { "▶" } else { "⏸" }).clicked() {
                        pause = Some(!status.paused);
                    }
                    ui.label(format!("{} / {}", clock(status.position), clock(status.duration)));
                    let mut at = status.position;
                    let bar = ui.add_sized([(ui.available_width() - 190.0).max(60.0), 18.0], egui::Slider::new(&mut at, 0.0..=status.duration.max(0.1)).show_value(false));
                    if bar.changed() {
                        seek = Some(at);
                    }
                    if ui.button(if muted { "🔇" } else { "🔊" }).clicked() {
                        muted = !muted;
                        audio = true;
                    }
                    if ui.add(egui::Slider::new(&mut volume, 0.0..=1.0).show_value(false)).changed() {
                        audio = true;
                    }
                });
            });
        });
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
        if response.clicked() {
            player.set_paused(!status.paused);
        }
        ui.ctx().request_repaint_after(Duration::from_millis(250));
    }

    fn front_page(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        if self.mode == Mode::Favorites {
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "No favorites to show.", egui::FontId::proportional(18.0), Color32::GRAY);
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
        let text = if searching { "Searching…" } else { "Type a search above." };
        ui.painter().text(egui::pos2(rect.center().x, rect.top() + 40.0), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(22.0), Color32::GRAY);
        if !searching && self.search.is_none() && self.flag("quickCards", true) {
            self.quick_cards(ui, rect);
        }
    }

    /// The quick searches as cards (two to a row): the first picture of the results and the main tags. Click runs the search.
    fn quick_cards(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
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
            return;
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
        let width = ((rect.width() - 60.0) / 2.0).clamp(200.0, 420.0);
        let area = egui::Rect::from_min_size(egui::pos2(rect.center().x - width - 6.0, rect.top() + 90.0), Vec2::new(width * 2.0 + 12.0, rect.height() - 100.0));
        let mut run = None;
        ui.scope_builder(egui::UiBuilder::new().max_rect(area), |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                for pair in cards.chunks(2) {
                    ui.horizontal(|ui| {
                        for (n, sites, tags) in pair {
                            let (card, response) = ui.allocate_exact_size(Vec2::new(width, 112.0), Sense::click());
                            let painter = ui.painter();
                            painter.rect_filled(card, 8.0, ui.visuals().faint_bg_color.gamma_multiply(2.0));
                            if response.hovered() {
                                painter.rect_stroke(card, 8.0, egui::Stroke::new(1.5, ui.visuals().selection.bg_fill), egui::StrokeKind::Inside);
                            }
                            let picture = egui::Rect::from_min_size(card.min + Vec2::splat(8.0), Vec2::splat(96.0));
                            let mut main_tags = String::new();
                            if let Some(slide) = self.covers.get(n).cloned() {
                                let url = self.copies_or(&slide.preview_file_url);
                                self.media.request(&self.engine, &url);
                                if let Some(State::Ready(p)) = self.media.get(&url) {
                                    paint_fit(ui, picture, &p.frames[0].texture, p.size);
                                }
                                if let Some(groups) = &slide.tag_groups {
                                    let picked: Vec<&str> = ["artist", "character", "copyright"].iter().filter_map(|c| groups.get(*c)?.as_array()).flatten().filter_map(Value::as_str).take(3).collect();
                                    main_tags = picked.join(" ");
                                }
                            }
                            let text_x = card.left() + 116.0;
                            let title = if tags.is_empty() { "(what is in the box)".to_string() } else { tags.clone() };
                            let names: Vec<&str> = sites.iter().map(|s| s.name()).collect();
                            let painter = ui.painter();
                            painter.text(egui::pos2(text_x, card.top() + 14.0), egui::Align2::LEFT_TOP, format!("{}  {}", (n + 1) % 10, title), egui::FontId::proportional(15.0), ui.visuals().strong_text_color());
                            painter.text(egui::pos2(text_x, card.top() + 40.0), egui::Align2::LEFT_TOP, names.join(", "), egui::FontId::proportional(12.0), Color32::GRAY);
                            painter.text(egui::pos2(text_x, card.top() + 62.0), egui::Align2::LEFT_TOP, main_tags.replace('_', " "), egui::FontId::proportional(12.0), Color32::GRAY);
                            if response.clicked() {
                                run = Some(*n);
                            }
                        }
                    });
                }
            });
        });
        if let Some(n) = run {
            self.run_quick_search(n);
        }
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

fn paint_fit(ui: &egui::Ui, rect: egui::Rect, texture: &egui::TextureHandle, size: [usize; 2]) {
    let (w, h) = (size[0] as f32, size[1] as f32);
    let scale = (rect.width() / w).min(rect.height() / h);
    media::paint(ui, egui::Rect::from_center_size(rect.center(), Vec2::new(w * scale, h * scale)), texture, size);
}

fn category_color(category: &str, dark: bool) -> Color32 {
    match category {
        "artist" => Color32::from_rgb(0xf2, 0xac, 0x08),
        "copyright" => Color32::from_rgb(0xdd, 0x00, 0xdd),
        "character" => Color32::from_rgb(0x00, 0xaa, 0x00),
        "species" => Color32::from_rgb(0xed, 0x5d, 0x1f),
        "meta" => if dark { Color32::WHITE } else { Color32::BLACK },
        "lore" => Color32::from_rgb(0x22, 0x88, 0x22),
        "invalid" => Color32::from_rgb(0xff, 0x3d, 0x3d),
        "general" => if dark { Color32::from_rgb(0xb4, 0xc7, 0xd9) } else { Color32::from_rgb(0x2c, 0x5f, 0x8e) },
        _ => if dark { Color32::from_rgb(0xb4, 0xc7, 0xd9) } else { Color32::from_rgb(0x2c, 0x5f, 0x8e) },
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.media.poll(&ctx);
        self.handle_messages();
        self.handle_keys(&ctx);
        self.tick(&ctx);
        self.preload();

        if self.show_controls {
            egui::Panel::top("bar").show(ui, |ui| self.top_bar(ui));
            egui::Panel::bottom("controls").show(ui, |ui| self.controls(ui));
        }
        if self.show_tags {
            if let Some(slide) = self.current() {
                egui::Panel::right("tags").default_size(220.0).show(ui, |ui| self.tags_panel(ui, &slide));
            }
        }
        egui::CentralPanel::default().show(ui, |ui| self.viewer(ui));

        self.test_hook(&ctx);
        self.test_script(&ctx);
        self.settings_window(&ctx);
        self.pools_window(&ctx);
        self.toasts(&ctx);
    }

    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        if let Some(player) = self.video.as_mut() {
            player.shutdown(gl);
        }
        self.store.flush();
    }
}

// So the settings and pool windows (another file) can name it.
pub fn user_dir() -> std::path::PathBuf {
    files::user_data()
}
