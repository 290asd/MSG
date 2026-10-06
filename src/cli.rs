// MSG in the terminal: e621 searches and the favorites as a slideshow of pictures, drawn with Sixel (Windows
// Terminal) or half blocks (other terminals). The searching, favorites, downloads, hotkeys and settings.json
// are the window version's (the modules below); this file is the screen and the keys, term.rs the pictures.
#![allow(dead_code)] // The shared modules have parts only the window uses.

mod downloads;
mod favorites;
mod files;
mod hotkeys;
mod net;
mod session;
mod sites;
mod slide;
mod store;
mod term;

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{self, BeginSynchronizedUpdate, Clear, ClearType, EndSynchronizedUpdate, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};
use downloads::Downloader;
use favorites::Favorites;
use hotkeys::Hotkeys;
use serde_json::{json, Value};
use session::{Engine, Msg, Search};
use sites::Site;
use slide::{MediaType, Slide};
use std::collections::HashMap;
use std::io::{self, BufWriter, Write};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};
use store::Store;
use term::{Area, Cache, Output, State};

const USAGE: &str = "msg-cli [--sixel | --blocks] [tags…]

A slideshow of e621 pictures in the terminal. Settings and favorites are shared with MSG
(%APPDATA%\\MSG\\settings.json). Sixel is used when the terminal says it draws it (Windows Terminal does),
half blocks elsewhere.
Press ? in the program for the keys.";
/// The picture on the screen and the next ones are loaded and drawn ahead.
const AHEAD: usize = 4;
const NOTICE_FOR: Duration = Duration::from_secs(9);

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    Slideshow,
    Favorites,
}

/// The text box that has the keyboard.
#[derive(Clone, Copy, PartialEq)]
enum Input {
    Off,
    Query,
    Filter,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Pick {
    History,
    Pools,
}

/// A list to choose from (the search history, the saved pools): (label, value).
struct Picker {
    pick: Pick,
    title: &'static str,
    items: Vec<(String, String)>,
    selected: usize,
}

/// Where things go on the screen.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Layout {
    cols: u16,
    rows: u16,
    top: u16,
    image: Area,
    tags_x: u16,
}

struct Cli {
    engine: Engine,
    output: Output,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    downloader: Downloader,
    search: Option<Search>,
    generation: u64,
    query: String,
    filter: String,
    mode: Mode,
    fav: Favorites,
    cur: [usize; 2],
    cache: Cache,
    copies: HashMap<String, String>,
    hotkeys: Hotkeys,
    playing: bool,
    shown_at: Option<Instant>,
    shown: Option<(Mode, usize)>,
    show_tags: bool,
    show_bars: bool,
    help: bool,
    input: Input,
    picker: Option<Picker>,
    notice: Option<(String, Instant)>,
    progress: String,
    // Where the tags were drawn (row, tag), for clicks.
    tag_rows: Vec<(u16, String)>,
    // What is on the screen: the picture part and the bars, redrawn only when they change.
    drawn: Option<String>,
    drawn_bars: Option<(String, bool, String)>,
    quit: bool,
}

fn main() {
    let (mut words, mut output) = (vec![], None);
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--sixel" => output = Some(Output::Sixel),
            "--blocks" => output = Some(Output::Blocks),
            "-h" | "--help" => return println!("{USAGE}"),
            _ => words.push(arg),
        }
    }
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().expect("async runtime");
    let store = Store::load(files::user_data(), files::legacy_dir());
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        default_hook(info);
    }));
    let result = terminal::enable_raw_mode().and_then(|_| {
        let mut cli = Cli::new(rt.handle().clone(), store.clone(), output.unwrap_or_else(detect_output));
        if !words.is_empty() {
            cli.start_search(&words.join(" "));
        }
        execute!(io::stdout(), EnterAlternateScreen, Hide, EnableMouseCapture)?;
        cli.run()
    });
    restore();
    store.flush();
    if let Err(e) = result {
        eprintln!("msg-cli: {e}");
    }
}

fn restore() {
    let _ = execute!(io::stdout(), DisableMouseCapture, Show, LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();
}

/// Asks the terminal (in raw mode) whether it draws Sixel. A program started by double-clicking is handed to
/// Windows Terminal without WT_SESSION, so the terminal's own answer decides; that is the guess only when it
/// doesn't answer in half a second.
fn detect_output() -> Output {
    let _ = execute!(io::stdout(), Print("\x1b[c"));
    let deadline = Instant::now() + Duration::from_millis(500);
    let mut answer = String::new();
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        if !event::poll(left).unwrap_or(false) {
            break;
        }
        // The answer comes as keys: Esc, then "[?61;4;…c".
        if let Ok(Event::Key(KeyEvent { code: KeyCode::Char(c), kind: KeyEventKind::Press, .. })) = event::read() {
            answer.push(c);
            if c == 'c' {
                break;
            }
        }
    }
    match sixel_in(&answer) {
        Some(true) => Output::Sixel,
        Some(false) => Output::Blocks,
        None if std::env::var_os("WT_SESSION").is_some() => Output::Sixel,
        None => Output::Blocks,
    }
}

/// Whether a Primary Device Attributes answer ("[?61;4;6c") lists 4, Sixel graphics; None when it isn't one.
fn sixel_in(answer: &str) -> Option<bool> {
    let params = answer.strip_prefix("[?")?.strip_suffix('c')?;
    Some(params.split(';').skip(1).any(|p| p == "4"))
}

impl Cli {
    fn new(rt: tokio::runtime::Handle, store: Arc<Store>, output: Output) -> Cli {
        let (tx, rx) = channel();
        // The loop looks for news ten times a second, so there is nothing to wake.
        let engine = Engine { rt, client: net::client(), store: store.clone(), wake: Arc::new(|| {}) };
        let downloader = Downloader::new(engine.clone(), tx.clone());
        let mut cli = Cli {
            output,
            fav: Favorites::load(&store),
            hotkeys: Hotkeys::load(&store),
            show_tags: store.get("showTags").and_then(|v| v.as_bool()).unwrap_or(false),
            engine,
            tx,
            rx,
            downloader,
            search: None,
            generation: 0,
            query: String::new(),
            filter: String::new(),
            mode: Mode::Slideshow,
            cur: [0, 0],
            cache: Cache::new(output),
            copies: HashMap::new(),
            playing: false,
            shown_at: None,
            shown: None,
            show_bars: true,
            help: false,
            input: Input::Off,
            picker: None,
            notice: None,
            progress: String::new(),
            tag_rows: vec![],
            drawn: None,
            drawn_bars: None,
            quit: false,
        };
        cli.refresh_copies();
        cli.rebuild_favorites();
        cli.downloader.schedule(Duration::from_secs(15));
        cli
    }

    fn run(&mut self) -> io::Result<()> {
        let mut out = BufWriter::with_capacity(1 << 16, io::stdout().lock());
        // Testing aid, as in the window: MSG_SCRIPT="10:next,14:prev,20:quit" does these at those seconds
        // (next, prev, quit or a hotkey action such as toggleTags).
        let started = Instant::now();
        let mut script: Vec<(f64, String)> = std::env::var("MSG_SCRIPT")
            .unwrap_or_default()
            .split(',')
            .filter_map(|s| s.split_once(':'))
            .filter_map(|(t, a)| Some((t.trim().parse().ok()?, a.trim().to_string())))
            .collect();
        while !self.quit {
            self.handle_messages();
            self.cache.poll();
            let lay = self.layout();
            self.tick();
            self.preload(lay.image);
            while script.first().is_some_and(|(t, _)| *t <= started.elapsed().as_secs_f64()) {
                match script.remove(0).1.as_str() {
                    "next" => self.move_by(1),
                    "prev" => self.move_by(-1),
                    "quit" => self.quit = true,
                    action => self.act(action),
                }
            }
            self.draw(&mut out, lay)?;
            if event::poll(Duration::from_millis(100))? {
                loop {
                    self.on_event(event::read()?, lay);
                    if !event::poll(Duration::ZERO)? {
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    // ---------- settings ----------

    fn flag(&self, key: &str, default: bool) -> bool {
        self.engine.store.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
    }

    fn text(&self, key: &str) -> String {
        self.engine.store.get(key).and_then(|v| v.as_str().map(String::from)).unwrap_or_default()
    }

    fn number(&self, key: &str, default: f64) -> f64 {
        match self.engine.store.get(key) {
            Some(Value::Number(n)) => n.as_f64().unwrap_or(default),
            Some(Value::String(s)) => s.trim().parse().unwrap_or(default),
            _ => default,
        }
    }

    fn set(&self, key: &str, value: Value) {
        self.engine.store.data.lock().unwrap().insert(key.to_string(), value);
        self.engine.store.save();
    }

    fn notice(&mut self, text: impl Into<String>) {
        self.notice = Some((text.into(), Instant::now()));
    }

    fn refresh_copies(&mut self) {
        self.copies = if self.flag("useLocalCopies", true) { downloads::local_copies(&self.engine.store) } else { HashMap::new() };
    }

    fn copies_or(&self, url: &str) -> String {
        downloads::display_url(&self.copies, url)
    }

    /// Pictures only: the favorites' videos (and Flash) are left out.
    fn rebuild_favorites(&mut self) {
        let offline = self.flag("offlineMode", false);
        let copies = self.copies.clone();
        self.fav.rebuild(&|s| matches!(s.media_type, MediaType::Image | MediaType::Gif) && (!offline || downloads::display_url(&copies, &s.file_url) != s.file_url));
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

    fn current(&self) -> Option<Slide> {
        self.list().get(self.idx()).cloned()
    }

    fn move_by(&mut self, delta: i64) {
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

    /// Searches e621 for pictures (the window's ticked sites are left as they are).
    fn start_search(&mut self, text: &str) {
        self.mode = Mode::Slideshow;
        self.generation += 1;
        let text = text.trim();
        self.query = text.to_string();
        let mut search = Search::new(self.generation, text, vec![Site::E621], &self.engine.store);
        search.cfg.videos = false;
        self.search = Some(search);
        self.cur[Mode::Slideshow as usize] = 0;
        self.shown_at = None;
        if self.flag("storeHistory", true) && !text.is_empty() {
            let mut history = self.history();
            history.retain(|h| h != text);
            history.push(text.to_string());
            let extra = history.len().saturating_sub(100);
            history.drain(..extra);
            self.set("searchHistory", json!(history));
        }
        self.load_more();
    }

    fn load_more(&mut self) {
        if let Some(search) = &mut self.search {
            search.load_more(&self.engine, self.tx.clone());
        }
    }

    fn history(&self) -> Vec<String> {
        self.engine.store.get("searchHistory").and_then(|v| v.as_array().cloned()).unwrap_or_default().iter().filter_map(|v| v.as_str().map(String::from)).collect()
    }

    /// The quick searches that search e621: (number, tags).
    fn quick(&self) -> Vec<(usize, String)> {
        let quick: Vec<Value> = self.engine.store.get("quickSearches").and_then(|v| v.as_array().cloned()).unwrap_or_default();
        quick
            .iter()
            .enumerate()
            .filter(|(_, q)| q["sites"].as_array().is_some_and(|s| s.iter().any(|s| s == "E621")))
            .map(|(n, q)| (n, q["tags"].as_str().unwrap_or("").trim().to_string()))
            .collect()
    }

    fn run_quick_search(&mut self, n: usize) {
        if let Some((_, tags)) = self.quick().into_iter().find(|(i, _)| *i == n) {
            let text = if tags.is_empty() { self.query.clone() } else { tags };
            self.start_search(&text);
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
                Msg::Progress(text) => self.progress = text,
                Msg::Downloaded(status) => {
                    self.refresh_copies();
                    self.notice(status);
                }
                Msg::Imported { .. } | Msg::Cover { .. } => {}
            }
        }
    }

    // ---------- actions ----------

    fn act(&mut self, action: &str) {
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
            "openSettings" => self.notice("The settings are in the MSG window."),
            "showInterface" => self.show_bars = !self.show_bars,
            "home" => {
                self.mode = Mode::Slideshow;
                self.search = None;
            }
            "openPools" => self.open_picker(Pick::Pools, 0),
            _ => {}
        }
    }

    fn toggle_favorite(&mut self) {
        let Some(slide) = self.current() else { return };
        let faved = self.fav.toggle(&slide);
        self.fav.save(&self.engine.store);
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
            if net::web_url(&slide.viewable_website_post_url).is_some() {
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

    fn open_favorites(&mut self, on: bool) {
        self.mode = if on { Mode::Favorites } else { Mode::Slideshow };
        self.shown_at = None;
        if on {
            self.rebuild_favorites();
        }
    }

    fn apply_filter(&mut self, random: bool) {
        self.fav.filter = self.filter.trim().to_string();
        self.fav.random = random;
        self.rebuild_favorites();
        self.cur[Mode::Favorites as usize] = 0;
        self.shown_at = None;
    }

    fn toggle_tag_search(&mut self, tag: &str) {
        let add = self.text("tagClick") == "add";
        if self.mode == Mode::Favorites {
            self.filter = if add && !self.filter.is_empty() { format!("{} {tag}", self.filter) } else { tag.to_string() };
            self.apply_filter(false);
        } else {
            let text = if add && !self.query.is_empty() { format!("{} {tag}", self.query) } else { tag.to_string() };
            self.start_search(&text);
        }
    }

    fn save_pool(&mut self) {
        let Some((id, name, count)) = self.search.as_ref().and_then(|s| s.pool()) else {
            return self.notice("Search pool:<number> (or paste a pool link) to save a pool.");
        };
        let cover = self.search.as_ref().and_then(|s| s.slides.first()).map(|s| s.preview_file_url.clone()).unwrap_or_default();
        session::save_pool(&self.engine.store, id, &name, count, &cover);
        self.notice(format!("Saved the pool “{name}”."));
        self.downloader.schedule(Duration::from_secs(5));
    }

    fn change_seconds(&mut self, by: f64) {
        let secs = (self.number("secondsPerSlide", 6.0).round() + by).clamp(1.0, 600.0);
        self.set("secondsPerSlide", json!(secs));
    }

    fn open_picker(&mut self, pick: Pick, selected: usize) {
        let (title, items): (&'static str, Vec<(String, String)>) = match pick {
            Pick::History => ("Search history", self.history().into_iter().rev().map(|h| (h.clone(), h)).collect()),
            Pick::Pools => {
                let pools = session::saved_pools(&self.engine.store);
                let items = pools.iter().map(|p| (format!("{}  ({} pages)", p["name"].as_str().unwrap_or("?"), p["count"].as_u64().unwrap_or(0)), p["id"].as_u64().unwrap_or(0).to_string())).collect();
                ("Saved pools", items)
            }
        };
        let selected = selected.min(items.len().saturating_sub(1));
        self.picker = Some(Picker { pick, title, items, selected });
    }

    fn picker_key(&mut self, code: KeyCode) {
        let Some(p) = self.picker.as_mut() else { return };
        match code {
            KeyCode::Up => p.selected = p.selected.saturating_sub(1),
            KeyCode::Down => p.selected = (p.selected + 1).min(p.items.len().saturating_sub(1)),
            KeyCode::Esc => self.picker = None,
            KeyCode::Enter => {
                let p = self.picker.take().unwrap();
                if let Some((_, value)) = p.items.get(p.selected) {
                    self.start_search(&if p.pick == Pick::Pools { format!("pool:{value}") } else { value.clone() });
                }
            }
            KeyCode::Delete => {
                let (pick, selected) = (p.pick, p.selected);
                let Some((_, value)) = p.items.get(selected).cloned() else { return };
                match pick {
                    Pick::History => {
                        let mut history = self.history();
                        history.retain(|h| *h != value);
                        self.set("searchHistory", json!(history));
                    }
                    Pick::Pools => {
                        let mut pools: Vec<Value> = self.engine.store.get("savedPools").and_then(|v| v.as_array().cloned()).unwrap_or_default();
                        pools.retain(|p| p["id"].as_u64().map(|id| id.to_string()) != Some(value.clone()));
                        self.set("savedPools", Value::Array(pools));
                    }
                }
                self.open_picker(pick, selected);
            }
            _ => {}
        }
    }

    // ---------- keys and the mouse ----------

    fn on_event(&mut self, event: Event, lay: Layout) {
        match event {
            // Windows reports the key going up too.
            Event::Key(k) if k.kind == KeyEventKind::Press => self.on_key(k),
            Event::Mouse(m) if m.kind == MouseEventKind::Down(MouseButton::Left) => self.on_click(m.column, m.row, lay),
            _ => {}
        }
    }

    fn on_key(&mut self, k: KeyEvent) {
        let (ctrl, alt) = (k.modifiers.contains(KeyModifiers::CONTROL), k.modifiers.contains(KeyModifiers::ALT));
        if ctrl && !alt && k.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        if self.help {
            self.help = false;
            return;
        }
        if self.picker.is_some() {
            return self.picker_key(k.code);
        }
        if self.input != Input::Off {
            match k.code {
                KeyCode::Esc => self.input = Input::Off,
                KeyCode::Enter => {
                    if std::mem::replace(&mut self.input, Input::Off) == Input::Query {
                        let text = self.query.clone();
                        self.start_search(&text);
                    } else {
                        self.apply_filter(false);
                    }
                }
                KeyCode::Backspace => {
                    self.text_box().pop();
                }
                // Ctrl and Alt together are AltGr (~ on a Nordic keyboard).
                KeyCode::Char(c) if ctrl == alt => self.text_box().push(c),
                _ => {
                    self.hotkeys_for(k, true);
                }
            }
            return;
        }
        if self.hotkeys_for(k, false) {
            return;
        }
        // Fixed keys, for what the window has buttons for.
        match k.code {
            KeyCode::Enter => self.playing = !self.playing,
            KeyCode::Esc => self.notice = None,
            KeyCode::Char('/') => self.input = if self.mode == Mode::Slideshow { Input::Query } else { Input::Filter },
            KeyCode::Char(c @ '0'..='9') if !ctrl && !alt && self.mode == Mode::Slideshow => self.run_quick_search(if c == '0' { 9 } else { c as usize - '1' as usize }),
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('h') => self.open_picker(Pick::History, 0),
            KeyCode::Char('p') => self.open_picker(Pick::Pools, 0),
            KeyCode::Char('P') => self.save_pool(),
            KeyCode::Char('z') if self.mode == Mode::Favorites => self.apply_filter(true),
            KeyCode::Char('+') => self.change_seconds(1.0),
            KeyCode::Char('-') => self.change_seconds(-1.0),
            KeyCode::Char('?') => self.help = true,
            _ => {}
        }
    }

    fn text_box(&mut self) -> &mut String {
        if self.input == Input::Filter { &mut self.filter } else { &mut self.query }
    }

    /// Runs the actions bound to the key in the settings (appHotkeys). Typing in a box: only keys with Ctrl/Alt.
    fn hotkeys_for(&mut self, k: KeyEvent, typing: bool) -> bool {
        let Some(code) = key_code(&k) else { return false };
        if typing && code & (hotkeys::CTRL | hotkeys::ALT) == 0 {
            return false;
        }
        let actions: Vec<&str> = hotkeys::ACTIONS.iter().filter(|a| self.hotkeys.map[a.id].contains(&code)).map(|a| a.id).collect();
        for action in &actions {
            self.act(action);
        }
        !actions.is_empty()
    }

    fn on_click(&mut self, x: u16, y: u16, lay: Layout) {
        if self.help || self.picker.is_some() {
            return;
        }
        if x >= lay.tags_x {
            if let Some((_, tag)) = self.tag_rows.iter().find(|(row, _)| *row == y).cloned() {
                self.toggle_tag_search(&tag);
            }
        } else if self.mode == Mode::Favorites && (lay.top..lay.top + lay.image.rows).contains(&y) && self.flag("clickOpensPost", true) {
            self.open_source();
        }
    }

    // ---------- the slideshow clock ----------

    fn tick(&mut self) {
        let Some(slide) = self.current() else {
            self.shown = None;
            return;
        };
        let here = (self.mode, self.idx());
        if self.shown != Some(here) {
            self.shown = Some(here);
            self.shown_at = None;
        }
        let url = self.copies_or(&slide.file_url);
        let (settled, failed) = match self.cache.get(&url) {
            Some(State::Ready { .. }) => (true, false),
            Some(State::Failed(_)) => (true, true),
            _ => (false, false),
        };
        if settled && self.shown_at.is_none() {
            self.shown_at = Some(Instant::now());
        }
        let Some(at) = self.shown_at.filter(|_| self.playing) else { return };
        let wait = if failed { Duration::from_secs(1) } else { Duration::from_secs_f64(self.number("secondsPerSlide", 6.0).max(1.0)) };
        if at.elapsed() >= wait {
            self.move_by(1);
        }
    }

    fn preload(&mut self, area: Area) {
        let i = self.idx();
        let urls: Vec<String> = self.list().iter().skip(i).take(AHEAD).map(|s| self.copies_or(&s.file_url)).collect();
        for url in urls {
            self.cache.request(&self.engine, &url, area);
        }
        if self.mode == Mode::Slideshow && self.search.as_ref().is_some_and(|s| s.has_more() && s.slides.len() < i + 25) {
            self.load_more();
        }
    }

    // ---------- drawing ----------

    fn layout(&self) -> Layout {
        let (cols, rows) = terminal::size().unwrap_or((80, 24));
        let top = self.show_bars as u16;
        let tags_w = if self.show_tags && self.current().is_some() { (cols / 4).clamp(20, 40).min(cols / 2) } else { 0 };
        // The last row is the bottom bar, or empty: a Sixel picture that reaches it scrolls the screen.
        let image = Area { cols: cols - tags_w, rows: rows.saturating_sub(top + 1), fit: self.flag("autoFitSlide", true) };
        Layout { cols, rows, top, image, tags_x: cols - tags_w }
    }

    fn draw(&mut self, out: &mut impl Write, lay: Layout) -> io::Result<()> {
        let slide = self.current();
        let url = slide.as_ref().map(|s| self.copies_or(&s.file_url));
        let picture = match url.as_deref().map(|u| self.cache.get(u)) {
            None => String::new(),
            Some(Some(State::Ready { out, .. })) if out.0 == lay.image => "ready".into(),
            Some(Some(State::Failed(e))) => format!("failed {e}"),
            Some(_) => "loading".into(),
        };
        let overlay = match &self.picker {
            Some(p) => format!("{:?} {} {}", p.pick, p.selected, p.items.len()),
            None => self.help.to_string(),
        };
        let searching = self.search.as_ref().map(|s| s.loading);
        let key = format!("{lay:?} {:?} {} {url:?} {picture} {overlay} {searching:?}", self.mode, self.idx());
        let bars = self.bars(lay);
        let full = self.drawn.as_deref() != Some(key.as_str());
        if !full && bars == self.drawn_bars {
            return Ok(());
        }
        queue!(out, BeginSynchronizedUpdate)?;
        if full {
            queue!(out, Clear(ClearType::All))?;
            self.tag_rows.clear();
            if self.help {
                self.draw_help(out, lay)?;
            } else if let Some(picker) = &self.picker {
                draw_picker(out, picker, lay)?;
            } else if let (Some(slide), Some(url)) = (&slide, &url) {
                self.draw_picture(out, lay, url)?;
                if lay.tags_x < lay.cols {
                    self.draw_tags(out, lay, slide)?;
                }
            } else {
                self.draw_front(out, lay)?;
            }
            self.drawn = Some(key);
        }
        if let Some((top, faved, bottom)) = &bars {
            let (heart, color) = if *faved { ("♥", Color::Rgb { r: 230, g: 70, b: 90 }) } else { ("♡", Color::Reset) };
            queue!(out, MoveTo(0, 0), Print(top), Clear(ClearType::UntilNewLine))?;
            queue!(out, MoveTo(0, lay.rows - 1), Print(" "), SetForegroundColor(color), Print(heart), ResetColor, Print(bottom), Clear(ClearType::UntilNewLine))?;
        }
        self.drawn_bars = bars;
        queue!(out, EndSynchronizedUpdate)?;
        out.flush()
    }

    /// The top bar (the search or the filter), whether the picture is a favorite, and the bottom bar.
    fn bars(&self, lay: Layout) -> Option<(String, bool, String)> {
        if !self.show_bars {
            return None;
        }
        let cursor = if self.input == Input::Off { "" } else { "▏" };
        let top = match self.mode {
            Mode::Slideshow => format!(" e621 › {}{cursor}", self.query),
            Mode::Favorites => format!(" Favorites › {}{cursor}   {} shown{}", self.filter, self.fav.view.len(), if self.fav.random { ", random" } else { "" }),
        };
        let (len, i) = (self.list().len(), self.idx());
        let more = if self.mode == Mode::Slideshow && self.search.as_ref().is_some_and(|s| s.has_more()) { "+" } else { "" };
        let count = if len == 0 { "0 / 0".to_string() } else { format!("{} / {len}{more}", i + 1) };
        let note = match &self.notice {
            Some((text, at)) if at.elapsed() < NOTICE_FOR => text.clone(),
            _ => self.progress.clone(),
        };
        let state = if self.playing { "playing" } else { "paused" };
        let bottom = format!("  {count}  {state}  {} s   {note}", self.number("secondsPerSlide", 6.0));
        let width = lay.cols.saturating_sub(1) as usize;
        Some((clip(&top, width), self.current().is_some_and(|s| self.fav.contains(&s)), clip(&bottom, width.saturating_sub(2))))
    }

    fn draw_picture(&mut self, out: &mut impl Write, lay: Layout, url: &str) -> io::Result<()> {
        let area = lay.image;
        let centered = |text: &str| MoveTo(area.cols.saturating_sub(text.chars().count() as u16) / 2, lay.top + area.rows / 2);
        match self.cache.get(url) {
            Some(State::Ready { out: (drawn_for, r), .. }) if *drawn_for == area => {
                let (x, y) = (area.cols.saturating_sub(r.cols) / 2, lay.top + area.rows.saturating_sub(r.rows) / 2);
                for (i, line) in r.lines.iter().enumerate() {
                    queue!(out, MoveTo(x, y + i as u16 * r.step), Print(line))?;
                }
            }
            Some(State::Failed(e)) => {
                let text = format!("Couldn't load the picture ({e})");
                queue!(out, centered(&text), Print(&text))?;
            }
            _ => queue!(out, centered("Loading…"), Print("Loading…"))?,
        }
        Ok(())
    }

    /// The tags by category, in the window's colours; a click searches for one.
    fn draw_tags(&mut self, out: &mut impl Write, lay: Layout, slide: &Slide) -> io::Result<()> {
        let (x, width, last) = (lay.tags_x + 1, (lay.cols - lay.tags_x - 1) as usize, lay.rows.saturating_sub(2));
        let mut y = lay.top;
        'groups: for (category, tags) in slide::tag_groups(slide) {
            if !category.is_empty() {
                if y > last {
                    break;
                }
                queue!(out, MoveTo(x, y), SetAttribute(Attribute::Bold), Print(clip(&category, width)), SetAttribute(Attribute::Reset))?;
                y += 1;
            }
            for tag in tags {
                if y > last {
                    break 'groups;
                }
                queue!(out, MoveTo(x, y), SetForegroundColor(tag_color(&category)), Print(clip(&tag.replace('_', " "), width)), ResetColor)?;
                self.tag_rows.push((y, tag));
                y += 1;
            }
            y += 1;
        }
        Ok(())
    }

    fn draw_front(&self, out: &mut impl Write, lay: Layout) -> io::Result<()> {
        let mut lines: Vec<String> = vec![];
        if self.mode == Mode::Favorites {
            lines.push("No favorites to show.".into());
        } else if self.search.as_ref().is_some_and(|s| s.loading) {
            lines.push("Searching…".into());
        } else {
            let how = if self.output == Output::Sixel { "Sixel" } else { "half blocks" };
            lines.push(format!("MSG {} · e621 in the terminal ({how})", env!("CARGO_PKG_VERSION")));
            lines.push(String::new());
            lines.push(format!("/ search   {} favorites   h history   p pools   ? keys   q quit", self.key_names("openFavorites")));
            let quick = self.quick();
            if !quick.is_empty() {
                lines.push(String::new());
                lines.push("Quick searches".into());
                for (n, tags) in quick {
                    lines.push(format!("  {}  {}", (n + 1) % 10, if tags.is_empty() { "(what is in the box)" } else { &tags }));
                }
            }
        }
        for (i, line) in lines.iter().enumerate().take(lay.rows.saturating_sub(lay.top + 2) as usize) {
            queue!(out, MoveTo(2, lay.top + 1 + i as u16), Print(clip(line, lay.cols.saturating_sub(3) as usize)))?;
        }
        Ok(())
    }

    fn draw_help(&self, out: &mut impl Write, lay: Layout) -> io::Result<()> {
        let mut lines = vec!["Keys (change them in MSG → Settings → Hotkeys)".to_string(), String::new()];
        for a in hotkeys::ACTIONS.iter().filter(|a| !matches!(a.id, "openSettings" | "setBackground" | "addToAnalysis")) {
            lines.push(format!("  {:<28} {}", self.key_names(a.id), a.label));
        }
        lines.push(String::new());
        for (keys, what) in [
            ("Enter", "Play / pause"),
            ("/", "Search (filter in the favorites)"),
            ("1 … 0", "Quick searches"),
            ("h, p", "Search history, saved pools"),
            ("Shift+P", "Save the pool being shown"),
            ("z", "Favorites in random order"),
            ("+, -", "Seconds per picture"),
            ("Click a tag", "Search for it"),
            ("q, Ctrl+C", "Quit"),
        ] {
            lines.push(format!("  {keys:<28} {what}"));
        }
        lines.push(String::new());
        lines.push("Any key closes this.".into());
        for (i, line) in lines.iter().enumerate().take(lay.rows.saturating_sub(3) as usize) {
            queue!(out, MoveTo(2, 1 + i as u16), Print(clip(line, lay.cols.saturating_sub(3) as usize)))?;
        }
        Ok(())
    }

    fn key_names(&self, action: &str) -> String {
        self.hotkeys.map.get(action).map(|codes| codes.iter().map(|c| hotkeys::label(*c)).collect::<Vec<_>>().join(", ")).unwrap_or_default()
    }
}

fn draw_picker(out: &mut impl Write, p: &Picker, lay: Layout) -> io::Result<()> {
    let width = lay.cols.saturating_sub(5) as usize;
    queue!(out, MoveTo(2, 1), SetAttribute(Attribute::Bold), Print(p.title), SetAttribute(Attribute::Reset))?;
    if p.items.is_empty() {
        queue!(out, MoveTo(2, 3), Print("Nothing here."))?;
    }
    let visible = lay.rows.saturating_sub(6).max(1) as usize;
    let start = p.selected.saturating_sub(visible - 1);
    for (row, (i, (label, _))) in p.items.iter().enumerate().skip(start).take(visible).enumerate() {
        let attribute = if i == p.selected { Attribute::Reverse } else { Attribute::Reset };
        queue!(out, MoveTo(2, 3 + row as u16), SetAttribute(attribute), Print(clip(&format!(" {label} "), width)), SetAttribute(Attribute::Reset))?;
    }
    queue!(out, MoveTo(2, lay.rows.saturating_sub(2)), Print("↑↓ choose   Enter open   Del remove   Esc close"))
}

/// The key as the settings save it: a JS key code, plus 256 for Ctrl, 512 for Shift and 1024 for Alt.
fn key_code(k: &KeyEvent) -> Option<u32> {
    let base = match k.code {
        KeyCode::Char(c) if c.is_ascii_alphanumeric() => c.to_ascii_uppercase() as u32,
        KeyCode::Char(' ') => 32,
        KeyCode::Left => 37,
        KeyCode::Up => 38,
        KeyCode::Right => 39,
        KeyCode::Down => 40,
        KeyCode::Enter => 13,
        KeyCode::Tab => 9,
        KeyCode::Esc => 27,
        KeyCode::PageUp => 33,
        KeyCode::PageDown => 34,
        KeyCode::End => 35,
        KeyCode::Home => 36,
        KeyCode::Insert => 45,
        KeyCode::Delete => 46,
        KeyCode::F(n) if (1..=12).contains(&n) => 111 + n as u32,
        _ => return None,
    };
    let m = k.modifiers;
    let bit = |flag: KeyModifiers, value: u32| if m.contains(flag) { value } else { 0 };
    Some(base + bit(KeyModifiers::CONTROL, hotkeys::CTRL) + bit(KeyModifiers::SHIFT, hotkeys::SHIFT) + bit(KeyModifiers::ALT, hotkeys::ALT))
}

/// The window's dark-theme tag colours.
fn tag_color(category: &str) -> Color {
    let (r, g, b) = match category {
        "artist" => (0xf2, 0xac, 0x08),
        "copyright" => (0xdd, 0x00, 0xdd),
        "character" => (0x00, 0xaa, 0x00),
        "species" => (0xed, 0x5d, 0x1f),
        "meta" => (0xff, 0xff, 0xff),
        "lore" => (0x22, 0x88, 0x22),
        "invalid" => (0xff, 0x3d, 0x3d),
        _ => (0xb4, 0xc7, 0xd9),
    };
    Color::Rgb { r, g, b }
}

fn clip(text: &str, width: usize) -> String {
    text.chars().take(width).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_codes_match_the_window() {
        let key = |code, modifiers| key_code(&KeyEvent::new(code, modifiers));
        assert_eq!(key(KeyCode::Char('f'), KeyModifiers::CONTROL), Some(hotkeys::CTRL + 70));
        assert_eq!(key(KeyCode::Char('D'), KeyModifiers::SHIFT), Some(hotkeys::SHIFT + 68));
        assert_eq!(key(KeyCode::Left, KeyModifiers::NONE), Some(37));
        assert_eq!(key(KeyCode::Char(' '), KeyModifiers::NONE), Some(32));
        assert_eq!(key(KeyCode::Char('?'), KeyModifiers::SHIFT), None);
    }

    #[test]
    fn sixel_from_the_terminals_answer() {
        // Windows Terminal's answer, xterm's without Sixel, and no answer.
        assert_eq!(sixel_in("[?61;4;6;7;14;21;22;23;24;28;32;42c"), Some(true));
        assert_eq!(sixel_in("[?1;2c"), Some(false));
        assert_eq!(sixel_in("[?4c"), Some(false));
        assert_eq!(sixel_in(""), None);
    }
}
