// The settings page, the saved pools page and the mosaic page.
use super::app::{App, Mode, Tab, TOP_INSET};
use super::hotkeys::{self, ACTIONS};
use super::logo;
use super::media::State;
use super::sites::Site;
use super::slide::{MediaType, Slide};
use super::theme::{self, icon, tok};
use eframe::egui::{self, RichText, Vec2};
use serde_json::{json, Value};

/// A pool on the pools page: the cover, and the name and page count under it.
const POOL_TILE: Vec2 = Vec2::new(160.0, 270.0);
const POOL_COVER: f32 = 210.0;
const POOL_GAP: f32 = 14.0;
/// The space between the mosaic's pictures.
const THUMB_GAP: f32 = 6.0;
const TILE_RADIUS: u8 = 8;

const TABS: [(Tab, char, &str); 10] = [
    (Tab::Appearance, icon::PALETTE, "Appearance"),
    (Tab::Sites, icon::SITES, "Sites & accounts"),
    (Tab::Slideshow, icon::SLIDESHOW, "Slideshow"),
    (Tab::Filtering, icon::FILTERS, "Filtering"),
    (Tab::Hotkeys, icon::KEYBOARD, "Hotkeys"),
    (Tab::Folders, icon::FOLDER, "Folders"),
    (Tab::Favorites, icon::HEART, "Favorites"),
    (Tab::Quick, icon::SAVED, "Quick searches"),
    (Tab::History, icon::HISTORY, "History"),
    (Tab::About, icon::INFO, "About"),
];

impl App {
    /// The test script's way to a section of the settings.
    pub fn open_tab(&mut self, name: &str) {
        if let Some((tab, _, _)) = TABS.iter().find(|(_, _, n)| n.eq_ignore_ascii_case(name)) {
            self.tab = *tab;
        }
    }

    fn check(&mut self, ui: &mut egui::Ui, key: &str, default: bool, label: &str) -> bool {
        let mut on = self.flag(key, default);
        let changed = theme::row(ui, label, |ui| theme::toggle(ui, &mut on).changed());
        if changed {
            self.set(key, json!(on));
        }
        changed
    }

    fn line(&mut self, ui: &mut egui::Ui, key: &str, label: &str, secret: bool) {
        let mut text = self.text(key);
        if theme::row(ui, label, |ui| ui.add(egui::TextEdit::singleline(&mut text).password(secret).desired_width(280.0)).changed()) {
            self.set(key, json!(text.trim()));
        }
    }

    fn pick_folder(&self, title: &str) -> Option<String> {
        rfd::FileDialog::new().set_title(title).pick_folder().map(|p| p.to_string_lossy().into_owned())
    }

    /// The welcome page of the first start: the basics in a few lines, and whether the BooruSlideshow settings came along.
    pub fn onboarding_page(&mut self, ui: &mut egui::Ui) {
        let t = tok(ui.ctx());
        let keys = |id: &str| self.hotkeys.map.get(id).map(|codes| codes.iter().map(|&c| hotkeys::label(c)).collect::<Vec<_>>().join(" / ")).unwrap_or_default();
        let basics = [
            ("Home", "The newest posts of the searches you follow. Search for a tag and press ☆ Follow to add it.".to_string()),
            ("Search", "Type tags in the bar at the top. Suggestions come as you type; Enter takes one.".to_string()),
            ("Mosaic and slideshow", "Click a picture to open it. Enter or Space starts the slideshow, Esc goes back.".to_string()),
            ("In the slideshow", format!(
                "{} and {} change the picture, so does the wheel. {} plays or pauses, {} faves, {} shows the tags. The rest is in the drawer at the left edge.",
                keys("previous"), keys("next"), keys("playPause"), keys("favorite"), keys("toggleTags"),
            )),
            ("Settings", "Sites and accounts, hotkeys, filters and the look are behind the gear in the rail.".to_string()),
        ];
        let imported = match &self.store.broken {
            Some(aside) => Some(format!("settings.json couldn't be read, so MSG started empty. The old file is kept as {}.", aside.display())),
            None => self.store.imported.then(|| format!("Your BooruSlideshow settings and {} favorites were brought over.", self.fav.items.len())),
        };
        egui::ScrollArea::vertical().id_salt("onboarding").auto_shrink(false).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.set_max_width(ui.available_width().min(560.0));
                ui.add_space(theme::S6 * 2.0);
                let (logo, _) = ui.allocate_exact_size(Vec2::new(144.0, 96.0), egui::Sense::hover());
                logo::paint(ui.painter(), logo, logo::angle(ui.ctx(), self.flag("spinLogo", true)), &t);
                ui.add_space(theme::S3);
                ui.label(RichText::new("Welcome to MSG").font(theme::semibold(24.0)));
                if let Some(text) = &imported {
                    ui.add_space(theme::S2);
                    ui.label(RichText::new(text).color(t.accent));
                }
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    theme::group(ui, "", |ui| {
                        for (title, text) in &basics {
                            ui.add_space(10.0);
                            ui.label(RichText::new(*title).font(theme::medium(14.0)));
                            ui.label(RichText::new(text).color(t.text2));
                            ui.add_space(10.0);
                        }
                    });
                });
                ui.add_space(theme::S6);
                let start = egui::Button::new(RichText::new("Get started").font(theme::medium(15.0)).color(t.base)).fill(t.accent).min_size(Vec2::new(160.0, 40.0));
                if ui.add(start).clicked() {
                    self.onboarding = false;
                }
                ui.add_space(theme::S6);
            });
        });
    }

    /// The settings as a page that fills the window: the sections on glass at the left, the chosen one beside them.
    pub fn settings_page(&mut self, ui: &mut egui::Ui) {
        const LIST_W: f32 = 212.0;
        let ctx = ui.ctx().clone();
        let area = ui.available_rect_before_wrap();
        let list = egui::Rect::from_min_size(area.min + Vec2::new(0.0, 6.0), Vec2::new(LIST_W, area.height() - 12.0));
        super::glass::panel(ui, list, super::glass::bar(&ctx));
        ui.scope_builder(egui::UiBuilder::new().max_rect(list.shrink(theme::S2)), |ui| {
            ui.horizontal(|ui| {
                if theme::icon_button(ui, icon::BACK, "Back", false).clicked() {
                    self.go_back();
                }
                ui.label(RichText::new("Settings").font(theme::semibold(15.0)));
            });
            ui.add_space(theme::S2);
            for (tab, glyph, name) in TABS {
                if theme::drawer_row(ui, glyph, name, self.tab == tab).clicked() {
                    self.tab = tab;
                    self.recording = None;
                }
            }
        });
        // The content in a readable width, centred in what is left.
        let rest = egui::Rect::from_min_max(egui::pos2(list.right() + theme::S2, area.top()), area.max);
        ui.scope_builder(egui::UiBuilder::new().max_rect(rest), |ui| {
            egui::ScrollArea::vertical().id_salt("settings").auto_shrink(false).show(ui, |ui| {
                let width = ui.available_width().min(680.0);
                ui.vertical_centered(|ui| {
                    ui.set_max_width(width);
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.add_space(theme::S6);
                        ui.spacing_mut().item_spacing.y = 4.0;
                        match self.tab {
                            Tab::Appearance => self.tab_appearance(ui, &ctx),
                            Tab::Sites => self.tab_sites(ui),
                            Tab::Slideshow => self.tab_slideshow(ui),
                            Tab::Filtering => self.tab_filtering(ui),
                            Tab::Hotkeys => self.tab_hotkeys(ui, &ctx),
                            Tab::Folders => self.tab_folders(ui),
                            Tab::Favorites => self.tab_favorites(ui),
                            Tab::Quick => self.tab_quick(ui),
                            Tab::History => self.tab_history(ui),
                            Tab::About => self.tab_about(ui),
                        }
                        ui.add_space(theme::S6);
                    });
                });
            });
        });
    }

    fn tab_appearance(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Appearance");
        let theme_now = self.text("appTheme");
        let choices = ["dark", "light", "system"];
        theme::group(ui, "Look", |ui| {
            let chosen = choices.iter().position(|c| *c == theme_now).unwrap_or(0);
            if let Some(n) = theme::row(ui, "Theme", |ui| theme::choice(ui, &["System", "Light", "Dark"], 2 - chosen)) {
                self.set("appTheme", json!(choices[2 - n]));
                self.apply_theme(ctx);
            }
            let chosen = match self.text("animations").as_str() {
                "off" => 0,
                "slow" => 2,
                _ => 1,
            };
            if let Some(n) = theme::row(ui, "Animations", |ui| theme::choice(ui, &["Off", "Fast", "Slow"], chosen)) {
                let speed = ["off", "fast", "slow"][n];
                self.set("animations", json!(speed));
                theme::set_motion(ctx, speed);
            }
            self.check(ui, "spinLogo", true, "Turn the logo");
        });
        theme::group(ui, "Front page", |ui| {
            let chosen = if self.reading_home() { 0 } else if self.hot_home() { 1 } else { 2 };
            if let Some(n) = theme::row(ui, "Show", |ui| theme::choice(ui, &["Mosaic", "Hottest", "Saved searches"], chosen)) {
                self.set("frontPage", json!(super::app::FRONT_PAGES[n]));
            }
            let periods = ["day", "week", "month", "year"];
            let chosen = periods.iter().position(|p| *p == self.text("hotPeriod")).unwrap_or(1);
            if let Some(n) = theme::row(ui, "Hottest of the last", |ui| theme::choice(ui, &["Day", "Week", "Month", "Year"], chosen)) {
                self.set("hotPeriod", json!(periods[n]));
            }
            self.check(ui, "quickCards", true, "Saved searches on the front page");
            let mut limit = self.auto_follow_limit();
            let hover = "The artists you have the most favorites from, two at least. Each one adds about half a second to loading the mosaic. An artist you stop following stays out.";
            if theme::row(ui, "Artists the mosaic follows automatically (0 = off)", |ui| ui.add(egui::DragValue::new(&mut limit).range(0.0..=100.0).speed(0.2).fixed_decimals(0)).on_hover_text(hover).changed()) {
                self.set("autoFollowArtists", json!(limit.round()));
            }
            self.check(ui, "mosaicFirst", true, "Open searches and favorites as a mosaic (off: straight to the slideshow)");
        });
        self.following(ui);
        theme::group(ui, "Browsing", |ui| {
            let add = self.text("tagClick") == "add";
            if let Some(n) = theme::row(ui, "Clicking a tag", |ui| theme::choice(ui, &["Searches for it", "Adds it to the search"], if add { 1 } else { 0 })) {
                self.set("tagClick", json!(if n == 1 { "add" } else { "replace" }));
            }
            self.check(ui, "clickOpensPost", true, "Favorites: clicking the picture opens its post");
            self.check(ui, "showDownloadButton", true, "Show the Download button");
            self.check(ui, "searchSortMenu", false, "Sort menu with the search");
        });
    }

    fn tab_sites(&mut self, ui: &mut egui::Ui) {
        ui.heading("Sites & accounts");
        let ticked = super::session::ticked_sites(&self.store);
        theme::group(ui, "Sites to search", |ui| {
            for site in Site::ALL {
                let mut on = ticked.contains(&site);
                if theme::row(ui, site.name(), |ui| theme::toggle(ui, &mut on).changed()) {
                    let map: serde_json::Map<String, Value> = Site::ALL.iter().map(|s| (s.id().to_string(), json!(if *s == site { on } else { ticked.contains(s) }))).collect();
                    self.set("sitesToSearch", Value::Object(map));
                }
            }
        });
        theme::group(ui, "Accounts", |ui| {
            self.line(ui, "e621Login", "e621 username", false);
            self.line(ui, "e621ApiKey", "e621 API key", true);
            self.line(ui, "derpibooruApiKey", "Derpibooru API key", true);
            self.line(ui, "tantabusApiKey", "Tantabus API key", true);
            self.line(ui, "gelbUserId", "Gelbooru user ID", false);
            self.line(ui, "gelbApiKey", "Gelbooru API key", true);
            self.line(ui, "rule34UserId", "Rule34 user ID", false);
            self.line(ui, "rule34ApiKey", "Rule34 API key", true);
        });
        theme::note(ui, "Optional, except Rule34 (rule34.xxx → My Account → Options). Kept in settings.json on this computer.");
    }

    fn tab_slideshow(&mut self, ui: &mut egui::Ui) {
        ui.heading("Slideshow");
        theme::group(ui, "Pictures", |ui| {
            let mut secs = self.number("secondsPerSlide", 6.0);
            if theme::row(ui, "Seconds per slide", |ui| ui.add(egui::DragValue::new(&mut secs).range(1.0..=600.0)).changed()) {
                self.set("secondsPerSlide", json!(secs));
            }
            self.check(ui, "autoFitSlide", true, "Fit the picture to the window");
            ui.add_enabled_ui(!self.flag("autoFitSlide", true), |ui| {
                for (key, label) in [("maxWidth", "Max width (px, 0 = no limit)"), ("maxHeight", "Max height (px, 0 = no limit)")] {
                    let mut v = self.number(key, 0.0);
                    if theme::row(ui, label, |ui| ui.add(egui::DragValue::new(&mut v).range(0.0..=20000.0)).changed()) {
                        self.set(key, if v < 1.0 { Value::Null } else { json!(v) });
                    }
                }
            });
        });
        theme::group(ui, "Videos", |ui| {
            self.check(ui, "playVideosToEnd", false, "Play videos to the end before moving on");
            self.check(ui, "videoAutoplay", true, "Play videos automatically");
            self.check(ui, "videoAutoMute", false, "Start videos muted");
            self.check(ui, "videoHwdec", false, "Decode videos with the graphics card (lighter, but some videos show streaks and dots; applies to the next video)");
        });
    }

    fn tab_filtering(&mut self, ui: &mut egui::Ui) {
        ui.heading("Filtering");
        theme::group(ui, "Kinds", |ui| {
            for (key, default, label) in [("includeImages", true, "Images"), ("includeGifs", true, "GIFs"), ("includeWebms", true, "Videos"), ("includeDupes", false, "Include duplicates (same md5)")] {
                self.check(ui, key, default, label);
            }
        });
        theme::group(ui, "Ratings", |ui| {
            for (key, default, label) in [("includeSafe", true, "Safe"), ("includeQuestionable", false, "Questionable"), ("includeExplicit", false, "Explicit")] {
                self.check(ui, key, default, label);
            }
        });
        theme::note(ui, "Applies to the next search. None of the ratings on: no rating filter.");
        theme::group(ui, "Blacklist (tags, separated by spaces or lines)", |ui| {
            let mut text = self.text("blacklist");
            ui.add_space(theme::S2);
            if ui.add(egui::TextEdit::multiline(&mut text).desired_width(f32::INFINITY).desired_rows(5)).changed() {
                self.set("blacklist", json!(text));
            }
            ui.add_space(theme::S2);
        });
    }

    fn tab_hotkeys(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Hotkeys");
        theme::note(ui, "Click a key, then press the new one or a mouse button / the wheel (Delete clears, Esc cancels; the left button can't be bound). Fixed: Enter, F11, Esc, 1–0 (saved searches).");
        // The key being recorded.
        if let Some((id, slot)) = self.recording.clone() {
            for event in ctx.input(|i| i.events.clone()) {
                if let Some(code) = hotkeys::mouse_code_of(&event) {
                    if let Some(codes) = self.hotkeys.map.get_mut(&id) {
                        if slot < codes.len() {
                            codes[slot] = code;
                        } else {
                            codes.push(code);
                        }
                    }
                    self.recording = None;
                    let value = self.hotkeys.to_value();
                    self.set("appHotkeys", value);
                    break;
                }
                if let egui::Event::Key { key, pressed: true, modifiers, .. } = event {
                    match key {
                        egui::Key::Escape => self.recording = None,
                        egui::Key::Delete => {
                            if let Some(codes) = self.hotkeys.map.get_mut(&id) {
                                if slot < codes.len() {
                                    codes.remove(slot);
                                }
                            }
                            self.recording = None;
                        }
                        _ => {
                            if let Some(code) = hotkeys::code_of(key, modifiers) {
                                if let Some(codes) = self.hotkeys.map.get_mut(&id) {
                                    if slot < codes.len() {
                                        codes[slot] = code;
                                    } else {
                                        codes.push(code);
                                    }
                                }
                                self.recording = None;
                            }
                        }
                    }
                    let value = self.hotkeys.to_value();
                    self.set("appHotkeys", value);
                }
            }
        }
        theme::group(ui, "", |ui| {
            for action in &ACTIONS {
                theme::row(ui, action.label, |ui| {
                    let codes = self.hotkeys.map[action.id].clone();
                    if ui.small_button("+").on_hover_text("Add a key").clicked() {
                        self.recording = Some((action.id.to_string(), codes.len()));
                    }
                    for (slot, code) in codes.iter().enumerate().rev() {
                        let recording = self.recording.as_ref().is_some_and(|(i, s)| i == action.id && *s == slot);
                        if theme::chip(ui, &if recording { "press a key…".to_string() } else { hotkeys::label(*code) }, recording).clicked() {
                            self.recording = Some((action.id.to_string(), slot));
                        }
                    }
                });
            }
        });
        ui.add_space(theme::S2);
        if ui.button("Reset all").clicked() {
            for action in &ACTIONS {
                self.hotkeys.map.insert(action.id.to_string(), action.defaults.to_vec());
            }
            let value = self.hotkeys.to_value();
            self.set("appHotkeys", value);
        }
    }

    fn tab_folders(&mut self, ui: &mut egui::Ui) {
        ui.heading("Folders");
        let list = |app: &App, key: &str| -> Vec<String> { app.store.get(key).and_then(|v| v.as_array().cloned()).unwrap_or_default().iter().filter_map(|v| v.as_str().map(String::from)).collect() };
        let (mut folders, mut off) = (list(self, "localFolders"), list(self, "localFoldersOff"));
        let mut remove = None;
        theme::group(ui, "Your folders (the \"Your folders\" site; tick it in Sites)", |ui| {
            for (i, folder) in folders.iter().enumerate() {
                theme::row(ui, folder.as_str(), |ui| {
                    if theme::icon_button(ui, icon::CLOSE, "Remove", false).clicked() {
                        remove = Some(i);
                    }
                    let mut on = !off.contains(folder);
                    if theme::toggle(ui, &mut on).changed() {
                        if on {
                            off.retain(|f| f != folder);
                        } else {
                            off.push(folder.clone());
                        }
                        self.set("localFoldersOff", json!(off));
                    }
                });
            }
            theme::row(ui, "", |ui| {
                if ui.button("Add folder…").clicked() {
                    if let Some(path) = self.pick_folder("Choose a folder with images and videos") {
                        if !folders.contains(&path) {
                            folders.push(path);
                            self.set("localFolders", json!(folders));
                        }
                    }
                }
            });
        });
        if let Some(i) = remove {
            folders.remove(i);
            self.set("localFolders", json!(folders));
        }
        let current = self.text("downloadFolder");
        theme::group(ui, "Downloads", |ui| {
            theme::row(ui, if current.is_empty() { "Downloads/MSG (default)" } else { current.as_str() }, |ui| {
                if ui.button("Default").clicked() {
                    self.set("downloadFolder", json!(""));
                    self.settings_changed();
                }
                if ui.button("Choose…").clicked() {
                    if let Some(path) = self.pick_folder("Choose the download folder") {
                        self.set("downloadFolder", json!(path));
                        self.settings_changed();
                    }
                }
            });
        });
    }

    fn tab_favorites(&mut self, ui: &mut egui::Ui) {
        ui.heading("Favorites");
        theme::group(ui, "Your favorites", |ui| {
            theme::row(ui, format!("{} favorites", self.fav.items.len()), |ui| {
                if ui.button("Import").on_hover_text("From e621, Derpibooru and Gelbooru, with the details under Sites & accounts. Already-added items are skipped.").clicked() {
                    super::favorites::import(&self.engine, self.tx.clone());
                }
            });
            if !self.progress.is_empty() {
                ui.label(RichText::new(&self.progress).weak());
            }
            self.check(ui, "syncE621Favorites", true, "Also fave and unfave on e621 (needs the e621 username and API key)");
            theme::row(ui, "Download all favorites being shown", |ui| {
                if ui.button("Stop").clicked() {
                    self.downloader.cancel_bulk();
                }
                if ui.button("Download").clicked() {
                    let urls: Vec<String> = self.fav.view.iter().map(|s| s.file_url.clone()).filter(|u| super::net::web_url(u).is_some()).collect();
                    self.downloader.download_many(urls, "MSG/favorites");
                }
            });
        });
        theme::group(ui, "Offline copies (in the background, into the download folder)", |ui| {
            if self.check(ui, "autoDownloadFavorites", false, "Download every favorite") {
                self.settings_changed();
            }
            if self.check(ui, "autoDownloadPools", false, "Download every saved pool (needs e621)") {
                self.settings_changed();
            }
            if self.check(ui, "useLocalCopies", true, "Use downloaded copies when they are on the disk") {
                self.refresh_copies();
            }
            if self.check(ui, "offlineMode", false, "Offline mode: nothing from the internet, only your folders and the downloads") {
                self.settings_changed();
            }
        });
    }

    fn tab_quick(&mut self, ui: &mut egui::Ui) {
        ui.heading("Quick searches");
        theme::note(ui, "Keys 1–0 select the sites and search for the tags (or what is in the box when the tags are empty).");
        let mut quick: Vec<Value> = self.store.get("quickSearches").and_then(|v| v.as_array().cloned()).unwrap_or_default();
        while quick.len() < 10 {
            quick.push(json!({"sites": [], "tags": ""}));
        }
        let mut changed = false;
        theme::group(ui, "", |ui| {
            for (n, entry) in quick.iter_mut().enumerate() {
                theme::row(ui, format!("{}", (n + 1) % 10), |ui| {
                    let mut tags = entry["tags"].as_str().unwrap_or("").to_string();
                    if ui.add(egui::TextEdit::singleline(&mut tags).desired_width(340.0).hint_text("tags")).changed() {
                        entry["tags"] = json!(tags);
                        changed = true;
                    }
                    let mut ids: Vec<String> = entry["sites"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
                    ui.menu_button(format!("Sites ({})", ids.len()), |ui| {
                        for site in Site::ALL {
                            let mut on = ids.iter().any(|i| i == site.id());
                            if ui.checkbox(&mut on, site.name()).changed() {
                                ids.retain(|i| i != site.id());
                                if on {
                                    ids.push(site.id().to_string());
                                }
                                entry["sites"] = json!(ids);
                                changed = true;
                            }
                        }
                    });
                });
            }
        });
        if changed {
            self.set("quickSearches", Value::Array(quick));
        }
    }

    fn tab_history(&mut self, ui: &mut egui::Ui) {
        ui.heading("History");
        theme::group(ui, "", |ui| {
            self.check(ui, "storeHistory", true, "Remember my searches");
            theme::row(ui, "Searches remembered so far", |ui| {
                if ui.button("Clear the history").clicked() {
                    self.set("searchHistory", json!([]));
                }
            });
        });
    }

    fn tab_about(&mut self, ui: &mut egui::Ui) {
        let (logo, _) = ui.allocate_exact_size(Vec2::new(240.0, 160.0), egui::Sense::hover());
        logo::paint(ui.painter(), logo, logo::angle(ui.ctx(), self.flag("spinLogo", true)), &tok(ui.ctx()));
        ui.heading(format!("MSG {}", env!("CARGO_PKG_VERSION")));
        theme::group(ui, "", |ui| {
            ui.add_space(theme::S2);
            ui.label("A desktop slideshow for booru sites, based on Chirmaya's BooruSlideshow 10.6.");
            ui.hyperlink_to("Source", "https://github.com/290asd/MSG");
            ui.add_space(theme::S2);
        });
        theme::group(ui, "Licenses", |ui| {
            ui.add_space(theme::S2);
            ui.label("Original license (Chirmaya): open license to copy/modify as long as it is not published under the name \"Booru Slideshow\".");
            ui.label("Libraries: egui/eframe (MIT or Apache-2.0), image, reqwest, tokio and others under MIT or Apache-2.0.");
            ui.label("Fonts: Inter (SIL Open Font License 1.1), Lucide icons (ISC).");
            ui.add_space(theme::S2);
        });
    }

    pub fn save_pool(&mut self, id: u64, name: String, count: usize, cover: String) {
        super::session::save_pool(&self.store, id, &name, count, &cover);
        self.notice(format!("Saved the pool “{name}”."));
        self.settings_changed();
    }

    /// The pools page's part of the top bar, right to left (see top_bar).
    pub fn pools_bar(&mut self, ui: &mut egui::Ui) {
        if ui.button("◀ Back").clicked() {
            self.go_back();
        }
        // One e621 job at a time (the favorites import's progress shows here too).
        let busy = !self.progress.is_empty();
        let find = ui.add_enabled(!busy, egui::Button::new("Find pools in favorites")).on_hover_text("Adds the e621 pools that your e621 favorites are in.");
        if find.clicked() {
            let favorites = self.fav.items.iter().filter(|s| s.site_id == "E621").map(|s| super::slide::id_text(&s.id)).collect();
            let saved = self.store.get("savedPools").and_then(|v| v.as_array().cloned()).unwrap_or_default().iter().filter_map(|p| p["id"].as_u64()).collect();
            super::session::find_pools(&self.engine, self.tx.clone(), favorites, saved);
        }
        let sort = self.text("poolsSort");
        let names = [("added", "Recently added"), ("name", "Name"), ("pages", "Most pages")];
        let label = names.iter().find(|(v, _)| *v == sort).map_or("Recently added", |(_, n)| *n);
        egui::ComboBox::from_id_salt("poolsort").selected_text(label).show_ui(ui, |ui| {
            for (value, name) in names {
                if ui.selectable_label(label == name, name).clicked() {
                    self.set("poolsSort", json!(value));
                }
            }
        });
        if busy {
            ui.label(RichText::new(&self.progress).weak());
        }
        ui.add(egui::TextEdit::singleline(&mut self.pool_filter).hint_text("Filter pools by name…").desired_width(f32::INFINITY));
    }

    /// The saved pools as covers that fill the window. Only the rows on the screen are drawn (and their covers
    /// loaded). A click opens the pool, × (on hover) removes it.
    pub fn pools_page(&mut self, ui: &mut egui::Ui) {
        let words: Vec<String> = self.pool_filter.to_lowercase().split_whitespace().map(String::from).collect();
        let mut pools = super::session::saved_pools(&self.store);
        pools.retain(|p| {
            let name = p["name"].as_str().unwrap_or("").to_lowercase();
            words.iter().all(|w| name.contains(w.as_str()))
        });
        let rect = ui.available_rect_before_wrap();
        if pools.is_empty() {
            let text = if words.is_empty() { "No saved pools. Search pool:<number> (or paste a pool link) and press ☆ Save pool, or use Find pools in favorites." } else { "No pools match." };
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(16.0), egui::Color32::GRAY);
            return;
        }
        // The scroll bar's room is left out; the columns are centred.
        let width = rect.width() - 16.0;
        let columns = ((width + POOL_GAP) / (POOL_TILE.x + POOL_GAP)).floor().max(1.0) as usize;
        let margin = ((width + POOL_GAP - columns as f32 * (POOL_TILE.x + POOL_GAP)) / 2.0).max(0.0);
        let (mut go, mut remove) = (None, None);
        ui.spacing_mut().item_spacing = Vec2::splat(POOL_GAP);
        egui::ScrollArea::vertical().id_salt("pools").auto_shrink(false).show_rows(ui, POOL_TILE.y, pools.len().div_ceil(columns), |ui, rows| {
            for row in pools.chunks(columns).skip(rows.start).take(rows.len()) {
                ui.horizontal(|ui| {
                    ui.add_space(margin);
                    for pool in row {
                        let id = pool["id"].as_u64().unwrap_or(0);
                        let name = pool["name"].as_str().unwrap_or("?");
                        let (tile, response) = ui.allocate_exact_size(POOL_TILE, egui::Sense::click());
                        let cover_rect = egui::Rect::from_min_size(tile.min, Vec2::new(POOL_TILE.x, POOL_COVER));
                        let hovered = ui.rect_contains_pointer(tile);
                        ui.painter().rect_filled(cover_rect, 6.0, ui.visuals().extreme_bg_color);
                        let cover = super::downloads::display_url(&self.copies, pool["cover"].as_str().unwrap_or(""));
                        self.media.request(&self.engine, &cover);
                        if let Some(State::Ready(p)) = self.media.get(&cover) {
                            super::app::paint_fit(ui, cover_rect, &p.frames[0].texture, p.size, 1.0, 6.0);
                        }
                        if hovered {
                            ui.painter().rect_stroke(cover_rect, 6.0, egui::Stroke::new(2.0, ui.visuals().selection.bg_fill), egui::StrokeKind::Outside);
                        }
                        // The name on two rows at most, the page count under it.
                        let mut job = egui::text::LayoutJob::simple(name.to_string(), egui::FontId::proportional(13.0), ui.visuals().strong_text_color(), POOL_TILE.x);
                        job.wrap.max_rows = 2;
                        let galley = ui.painter().layout_job(job);
                        ui.painter().galley(egui::pos2(tile.left(), cover_rect.bottom() + 6.0), galley, ui.visuals().text_color());
                        let pages = format!("{} pages", pool["count"].as_u64().unwrap_or(0));
                        ui.painter().text(tile.left_bottom(), egui::Align2::LEFT_BOTTOM, pages, egui::FontId::proportional(12.0), egui::Color32::GRAY);
                        if hovered {
                            let corner = egui::Rect::from_min_size(egui::pos2(cover_rect.right() - 30.0, cover_rect.top() + 6.0), Vec2::splat(24.0));
                            if ui.put(corner, egui::Button::new("×")).on_hover_text("Remove from the saved pools").clicked() {
                                remove = Some(id);
                            }
                        }
                        if response.on_hover_text(name).clicked() {
                            go = Some(id);
                        }
                    }
                });
            }
        });
        if let Some(id) = remove {
            let mut all: Vec<Value> = self.store.get("savedPools").and_then(|v| v.as_array().cloned()).unwrap_or_default();
            all.retain(|p| p["id"].as_u64() != Some(id));
            self.set("savedPools", Value::Array(all));
        }
        if let Some(id) = go {
            self.start_search(&format!("pool:{id}"));
        }
    }

    /// The mosaic's part of the top bar, right to left (see top_bar); the search or favorites bar comes after it.
    /// What the reading list follows, and stopping it (Settings → Appearance).
    fn following(&mut self, ui: &mut egui::Ui) {
        let list = super::session::reading_list(&self.store);
        let mut remove = None;
        theme::group(ui, "Following", |ui| {
            if list.is_empty() {
                theme::row(ui, "Nothing followed yet. Search for an artist or tag and press Follow.", |_| {});
            }
            for (tags, sites) in &list {
                let names: Vec<&str> = sites.iter().map(|s| s.name()).collect();
                theme::row(ui, format!("{}  ·  {}", tags.replace('_', " "), names.join(", ")), |ui| {
                    if theme::icon_button(ui, icon::CLOSE, "Stop following", false).clicked() {
                        remove = Some(tags.clone());
                    }
                });
            }
        });
        // The mosaic reads the list again when it is next opened.
        if let Some(tags) = remove {
            self.toggle_follow(&tags);
        }
    }

    /// Ctrl and the wheel over a mosaic: bigger or smaller pictures.
    pub fn zoom_mosaic(&mut self, ui: &egui::Ui) {
        let zoom = ui.input(|i| i.zoom_delta());
        if zoom != 1.0 {
            self.set("readingThumbWidth", json!((self.number("readingThumbWidth", 220.0) * zoom as f64).clamp(100.0, 600.0)));
        }
    }

    /// One picture of a mosaic: on the surface colour until it has come, then fading in.
    pub fn tile(&mut self, ui: &mut egui::Ui, rect: egui::Rect, id: egui::Id, url: &str) -> egui::Response {
        self.tile_in(ui, rect, id, url, None)
    }

    /// `veil`: while a mosaic's first screen is still coming, how far this picture has come out (0 until all of
    /// them are here); till then it is a silhouette (see `mosaic_page`).
    fn tile_in(&mut self, ui: &mut egui::Ui, rect: egui::Rect, id: egui::Id, url: &str, veil: Option<f32>) -> egui::Response {
        let response = ui.interact(rect, id, egui::Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        let ctx = ui.ctx().clone();
        let t = tok(&ctx);
        let url = super::downloads::display_url(&self.copies, url);
        self.media.request(&self.engine, &url);
        let ready = matches!(self.media.get(&url), Some(State::Ready(_)));
        if let Some(out) = veil {
            let done = ready || matches!(self.media.get(&url), Some(State::Failed(_)));
            silhouette(ui, rect, done, 1.0 - out);
            if let Some(State::Ready(p)) = self.media.get(&url) {
                if out > 0.0 {
                    // Growing a little into its place as it comes out.
                    let shown = egui::Rect::from_center_size(rect.center(), rect.size() * (0.96 + 0.04 * out));
                    super::app::paint_fit(ui, shown, &p.frames[0].texture, p.size, out, TILE_RADIUS as f32);
                }
            }
            return response;
        }
        let shown = rect;
        let fade = ctx.animate_bool_with_time(id.with("in"), ready, theme::smooth());
        if fade < 1.0 {
            ui.painter().rect_filled(shown, TILE_RADIUS, t.surface);
        }
        if let Some(State::Ready(p)) = self.media.get(&url) {
            super::app::paint_fit(ui, shown, &p.frames[0].texture, p.size, fade, TILE_RADIUS as f32);
            // The sites say a picture's shape; of your own files it is known now.
            if super::net::web_url(&url).is_none() {
                self.ratios.entry(url).or_insert(p.size[1] as f32 / p.size[0] as f32);
            }
        }
        response
    }

    /// The posts being shown in columns that fill the window (about the chosen width each), in their order: each
    /// picture keeps its shape and goes under the shortest column. Only the pictures on the screen are drawn (and
    /// loaded), and near the end the next page is read. A click shows the post in the slideshow.
    pub fn mosaic_page(&mut self, ui: &mut egui::Ui) {
        self.zoom_mosaic(ui);
        let favorites = self.mode == Mode::Favorites;
        let rect = ui.available_rect_before_wrap();
        let ratios: Vec<f32> = self.list().iter().map(|x| if x.width > 0 && x.height > 0 { x.height as f32 / x.width as f32 } else { self.ratios.get(&x.preview_file_url).copied().unwrap_or(1.0) }).collect();
        if ratios.is_empty() {
            let text = if favorites {
                "No favorites to show."
            } else if self.search.as_ref().is_some_and(|s| s.loading) {
                // The posts are still being read: a mosaic of silhouettes that the pictures will fill.
                const SHAPES: [f32; 8] = [1.4, 0.75, 1.0, 1.25, 0.66, 1.5, 0.9, 1.1];
                let target = self.number("readingThumbWidth", 220.0) as f32;
                let shapes: Vec<f32> = SHAPES.iter().cycle().take(200).copied().collect();
                let (cells, _, _) = masonry(&shapes, rect.width() - theme::S2, target);
                for cell in cells.iter().map(|c| c.translate(rect.min.to_vec2())).filter(|c| c.intersects(rect)) {
                    silhouette(ui, cell, false, 1.0);
                }
                return;
            } else if self.reading_list && super::session::reading_list(&self.store).is_empty() {
                "Nothing followed yet. Search for an artist or tag and press Follow."
            } else {
                "No posts."
            };
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(16.0), tok(ui.ctx()).text2);
            return;
        }
        let target = self.number("readingThumbWidth", 220.0) as f32;
        let (cells, total, column) = masonry(&ratios, rect.width() - theme::S2, target);
        // Wide columns get the sites' larger samples, as their thumbnails are about 150 px.
        let big = column * ui.ctx().pixels_per_point() > 200.0;
        let (mut go, mut near_end, mut fave, mut artist_search) = (None, false, None, None);
        // A list read again starts from the top.
        let generation = if favorites { self.fav_view } else { self.search.as_ref().map_or(0, |s| s.generation) };
        egui::ScrollArea::vertical().id_salt(("mosaic", self.mode as usize, generation)).auto_shrink(false).show_viewport(ui, |ui, viewport| {
            ui.set_height(total);
            near_end = viewport.bottom() + rect.height() >= total;
            let origin = ui.max_rect().min.to_vec2();
            if std::mem::take(&mut self.mosaic_seek) {
                // The post the slideshow was on comes up about a third of the way down the window.
                if let Some(cell) = cells.get(self.idx()) {
                    let top = egui::Rect::from_min_size(egui::pos2(cell.left(), cell.center().y - rect.height() / 3.0), Vec2::new(1.0, 1.0));
                    ui.scroll_to_rect_animation(top.translate(origin), Some(egui::Align::Min), egui::style::ScrollAnimation::none());
                    let now = ui.input(|i| i.time);
                    let idx = self.idx();
                    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("mosaic flash"), (idx, now)));
                }
            }
            // The post the slideshow was on flashes twice, so the eye finds where it was.
            const FLASH: f64 = 1.8;
            let flash = ui.ctx().data(|d| d.get_temp::<(usize, f64)>(egui::Id::new("mosaic flash"))).and_then(|(n, at)| {
                let p = (ui.input(|i| i.time) - at) / FLASH;
                (p < 1.0).then_some((n, p as f32))
            });
            let t = tok(ui.ctx());
            let url_of = |slide: &Slide| {
                let video = slide.media_type == MediaType::Video;
                slide.sample_file_url.clone().filter(|u| big && !video && !u.is_empty()).unwrap_or_else(|| slide.preview_file_url.clone())
            };
            // The first screen comes out at once when all its pictures are here (or after a while, if some are
            // slow): till then they are silhouettes, and those whose picture has come stand out, like a loading
            // bar. Then they come out from the top down.
            let ctx = ui.ctx().clone();
            let now = ctx.input(|i| i.time);
            let key = egui::Id::new(("mosaic reveal", self.mode as usize, generation));
            let (since, mut reveal) = ctx.data(|d| d.get_temp::<(f64, Option<f64>)>(key)).unwrap_or((now, None));
            if reveal.is_none() {
                let visible: Vec<String> = cells.iter().enumerate().filter(|(_, c)| c.intersects(viewport)).map(|(n, _)| super::downloads::display_url(&self.copies, &url_of(&self.list()[n]))).collect();
                let here = visible.iter().filter(|u| matches!(self.media.get(u), Some(State::Ready(_) | State::Failed(_)))).count();
                if here == visible.len() || now - since > 8.0 {
                    reveal = Some(now);
                }
            }
            const WAVE: f64 = 0.35;
            let spread = (theme::fade() as f64 / 0.14 * WAVE).min(1.0);
            let length = spread + theme::fade() as f64;
            let veiling = reveal.is_none_or(|at| now - at < length);
            ctx.data_mut(|d| d.insert_temp(key, (since, reveal)));
            if veiling {
                ctx.request_repaint();
            }
            for (n, cell) in cells.iter().enumerate().filter(|(_, c)| c.intersects(viewport)) {
                let slide = self.list()[n].clone();
                let video = slide.media_type == MediaType::Video;
                let url = url_of(&slide);
                let tile = cell.translate(origin);
                let id = ui.id().with(("post", n));
                let veil = veiling.then(|| {
                    let Some(at) = reveal else { return 0.0 };
                    let delay = ((cell.top() - viewport.top()) / viewport.height().max(1.0)).clamp(0.0, 1.0) as f64 * spread;
                    let fade = theme::fade() as f64;
                    if fade == 0.0 { 1.0 } else { ((now - at - delay) / fade).clamp(0.0, 1.0) as f32 }
                });
                let response = self.tile_in(ui, tile, id, &url, veil);
                let faved = self.fav.contains(&slide);
                if !favorites && faved {
                    ui.painter().rect_stroke(tile, TILE_RADIUS, egui::Stroke::new(2.0, t.accent), egui::StrokeKind::Inside);
                }
                if let Some((_, p)) = flash.filter(|(f, _)| *f == n) {
                    let pulse = (p * 2.0 * std::f32::consts::TAU).cos() * -0.5 + 0.5;
                    let fade = 1.0 - p;
                    ui.painter().rect_filled(tile, TILE_RADIUS, egui::Color32::WHITE.gamma_multiply(0.35 * pulse * fade));
                    ui.painter().rect_stroke(tile, TILE_RADIUS, egui::Stroke::new(3.0, t.accent.gamma_multiply(fade)), egui::StrokeKind::Inside);
                    ui.ctx().request_repaint();
                }
                if video {
                    theme::badge(ui, egui::Rect::from_min_size(tile.min + Vec2::splat(6.0), Vec2::splat(24.0)), theme::glyph_text(icon::PLAY), theme::icon_font(12.0), egui::Color32::WHITE);
                }
                if ui.rect_contains_pointer(tile) {
                    let corner = egui::Rect::from_min_size(egui::pos2(tile.right() - 34.0, tile.top() + 6.0), Vec2::splat(28.0));
                    let tip = if faved { "Remove from the favorites" } else { "Add to the favorites" };
                    if theme::overlay_button(ui, corner, id.with("heart"), theme::heart_text(faved), theme::heart_font(faved, 15.0), if faved { t.accent } else { egui::Color32::WHITE }, tip).clicked() {
                        fave = Some(slide.clone());
                    }
                    if let Some(name) = artist_of(&slide) {
                        if theme::overlay_chip(ui, tile, id.with("artist"), &name.replace('_', " ")).clicked() {
                            artist_search = Some(name);
                        }
                    }
                }
                if response.clicked() {
                    go = Some(n);
                }
            }
        });
        if near_end && !favorites {
            self.load_more();
        }
        self.size_pill(ui.ctx());
        if let Some(slide) = fave {
            self.toggle_favorite_of(slide);
        }
        if let Some(name) = artist_search {
            self.start_search(&name);
        } else if let Some(n) = go {
            self.show_mosaic = false;
            self.playing = false;
            self.set_idx(n);
        }
    }

    /// The picture size and the number of posts, floating at the mosaic's corner.
    fn size_pill(&mut self, ctx: &egui::Context) {
        let (count, loading) = (self.list().len(), self.search.as_ref().is_some_and(|s| s.loading));
        egui::Area::new(egui::Id::new("size pill")).order(egui::Order::Middle).anchor(egui::Align2::RIGHT_BOTTOM, [-theme::S3, -theme::S3]).show(ctx, |ui| {
            super::glass::card(ui, super::glass::bar(ctx), egui::Margin::symmetric(12, 6), |ui| {
                ui.horizontal(|ui| {
                    if self.mode == Mode::Slideshow {
                        ui.label(RichText::new(if loading { "Loading…".to_string() } else { format!("{count} posts") }).small().weak());
                    }
                    ui.spacing_mut().slider_width = 100.0;
                    let mut size = self.number("readingThumbWidth", 220.0);
                    if ui.add(egui::Slider::new(&mut size, 100.0..=600.0).show_value(false)).on_hover_text("Picture size").changed() {
                        self.set("readingThumbWidth", json!(size));
                    }
                });
            });
        });
    }
}

/// Where pictures go in columns that fill `width`, each column about `target` wide: every picture under the shortest
/// column, starting below the toolbar. Very tall or wide ones are kept within 1:4 and 3:1 (letterboxed).
/// Returns the places, the height of them all and the width of a column.
pub fn masonry(ratios: &[f32], width: f32, target: f32) -> (Vec<egui::Rect>, f32, f32) {
    let columns = ((width + THUMB_GAP) / (target + THUMB_GAP)).floor().max(1.0) as usize;
    let column = (width - THUMB_GAP * (columns - 1) as f32) / columns as f32;
    let mut heights = vec![TOP_INSET; columns];
    let cells = ratios
        .iter()
        .map(|ratio| {
            let c = (0..columns).min_by(|a, b| heights[*a].total_cmp(&heights[*b])).unwrap_or(0);
            let cell = egui::Rect::from_min_size(egui::pos2(c as f32 * (column + THUMB_GAP), heights[c]), Vec2::new(column, column * ratio.clamp(0.25, 3.0)));
            heights[c] = cell.bottom() + THUMB_GAP;
            cell
        })
        .collect();
    (cells, heights.iter().copied().fold(0.0, f32::max) + theme::S3, column)
}

/// A picture's place before it shows: a light passing over it while it is on its way, a coral-tinged surface once
/// it is here. `strength` fades the silhouette as the picture comes out over it.
fn silhouette(ui: &egui::Ui, rect: egui::Rect, here: bool, strength: f32) {
    if strength <= 0.0 {
        return;
    }
    let t = tok(ui.ctx());
    let color = if here {
        t.surface.lerp_to_gamma(t.accent, 0.22)
    } else {
        // The light moves down and to the right across the whole mosaic.
        let time = ui.input(|i| i.time) as f32;
        let wave = 0.5 + 0.5 * (time * 2.4 - (rect.center().x + rect.center().y) * 0.004).sin();
        t.surface.lerp_to_gamma(t.hover, wave * 0.6)
    };
    ui.painter().rect_filled(rect, TILE_RADIUS, color.gamma_multiply(strength));
    ui.ctx().request_repaint();
}

/// The post's artist, where the site says who that is.
fn artist_of(slide: &Slide) -> Option<String> {
    let artists = slide.tag_groups.as_ref()?.get("artist")?.as_array()?;
    artists.iter().filter_map(Value::as_str).find(|a| super::favorites::is_artist(a)).map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masonry_fills_the_shortest_column_and_keeps_within_the_width() {
        let (cells, total, column) = masonry(&[1.0, 2.0, 1.0, 1.0], 206.0, 100.0);
        assert_eq!(column, 100.0);
        assert_eq!(cells[0].left(), 0.0);
        assert_eq!(cells[1].left(), 106.0);
        // The first column is the shorter one by then.
        assert_eq!((cells[2].left(), cells[2].top()), (0.0, TOP_INSET + 106.0));
        assert!(cells.iter().all(|c| c.right() <= 206.0 && c.bottom() < total));
    }
}
