// The settings window and the saved pools window.
use super::app::{App, Tab};
use super::hotkeys::{self, ACTIONS};
use super::media::State;
use super::sites::Site;
use eframe::egui::{self, RichText, Vec2};
use serde_json::{json, Value};

const TABS: [(Tab, &str); 10] = [
    (Tab::Appearance, "Appearance"),
    (Tab::Sites, "Sites & accounts"),
    (Tab::Slideshow, "Slideshow"),
    (Tab::Filtering, "Filtering"),
    (Tab::Hotkeys, "Hotkeys"),
    (Tab::Folders, "Folders"),
    (Tab::Favorites, "Favorites"),
    (Tab::Quick, "Quick searches"),
    (Tab::History, "History"),
    (Tab::About, "About"),
];

impl App {
    fn check(&mut self, ui: &mut egui::Ui, key: &str, default: bool, label: &str) -> bool {
        let mut on = self.flag(key, default);
        let changed = ui.checkbox(&mut on, label).changed();
        if changed {
            self.set(key, json!(on));
        }
        changed
    }

    fn line(&mut self, ui: &mut egui::Ui, key: &str, label: &str, secret: bool) {
        let mut text = self.text(key);
        ui.horizontal(|ui| {
            ui.label(label);
            if ui.add(egui::TextEdit::singleline(&mut text).password(secret).desired_width(280.0)).changed() {
                self.set(key, json!(text.trim()));
            }
        });
    }

    fn pick_folder(&self, title: &str) -> Option<String> {
        rfd::FileDialog::new().set_title(title).pick_folder().map(|p| p.to_string_lossy().into_owned())
    }

    pub fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }
        let mut open = true;
        egui::Window::new("Settings").open(&mut open).default_size([720.0, 520.0]).show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_min_width(130.0);
                    for (tab, name) in TABS {
                        if ui.selectable_label(self.tab == tab, name).clicked() {
                            self.tab = tab;
                            self.recording = None;
                        }
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.vertical(|ui| match self.tab {
                        Tab::Appearance => self.tab_appearance(ui, ctx),
                        Tab::Sites => self.tab_sites(ui),
                        Tab::Slideshow => self.tab_slideshow(ui),
                        Tab::Filtering => self.tab_filtering(ui),
                        Tab::Hotkeys => self.tab_hotkeys(ui, ctx),
                        Tab::Folders => self.tab_folders(ui),
                        Tab::Favorites => self.tab_favorites(ui),
                        Tab::Quick => self.tab_quick(ui),
                        Tab::History => self.tab_history(ui),
                        Tab::About => self.tab_about(ui),
                    });
                });
            });
        });
        if !open {
            self.show_settings = false;
            self.recording = None;
        }
    }

    fn tab_appearance(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Appearance");
        let theme = self.text("appTheme");
        ui.horizontal(|ui| {
            ui.label("Theme");
            for (value, name) in [("dark", "Dark"), ("light", "Light"), ("system", "System")] {
                if ui.radio(theme == value || (theme.is_empty() && value == "dark"), name).clicked() {
                    self.set("appTheme", json!(value));
                    self.apply_theme(ctx);
                }
            }
        });
        let add = self.text("tagClick") == "add";
        ui.horizontal(|ui| {
            ui.label("Clicking a tag");
            if ui.radio(!add, "searches for that tag").clicked() {
                self.set("tagClick", json!("replace"));
            }
            if ui.radio(add, "adds it to the search").clicked() {
                self.set("tagClick", json!("add"));
            }
        });
        self.check(ui, "clickOpensPost", true, "Favorites: clicking the picture opens its post");
        self.check(ui, "showDownloadButton", true, "Show the Download button");
        self.check(ui, "searchSortMenu", false, "Sort menu in the search bar");
        self.check(ui, "quickCards", true, "Quick search cards on the front page");
        ui.separator();
        ui.label("Background image (the start screen)");
        ui.horizontal(|ui| {
            if ui.button("Choose…").clicked() {
                if let Some(path) = rfd::FileDialog::new().add_filter("Images", &["jpg", "jpeg", "png", "webp", "gif", "bmp"]).pick_file() {
                    let dir = super::app::user_dir();
                    let _ = std::fs::create_dir_all(&dir);
                    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("jpg").to_lowercase();
                    let target = dir.join(format!("background-{stamp}.{ext}"));
                    match std::fs::copy(&path, &target) {
                        Ok(_) => self.set("backgroundImage", json!(target.to_string_lossy())),
                        Err(e) => self.notice(format!("Couldn't use that file: {e}")),
                    }
                }
            }
            if ui.button("Clear").clicked() {
                self.set("backgroundImage", json!(""));
            }
        });
    }

    fn tab_sites(&mut self, ui: &mut egui::Ui) {
        ui.heading("Sites & accounts");
        ui.label("Sites to search (also in the Sites menu):");
        let ticked = super::session::ticked_sites(&self.store);
        ui.horizontal_wrapped(|ui| {
            for site in Site::ALL {
                let mut on = ticked.contains(&site);
                if ui.checkbox(&mut on, site.name()).changed() {
                    let map: serde_json::Map<String, Value> = Site::ALL.iter().map(|s| (s.id().to_string(), json!(if *s == site { on } else { ticked.contains(s) }))).collect();
                    self.set("sitesToSearch", Value::Object(map));
                }
            }
        });
        ui.separator();
        ui.label(RichText::new("Optional, except Rule34 (rule34.xxx → My Account → Options). Kept in settings.json on this computer.").weak());
        self.line(ui, "e621Login", "e621 username", false);
        self.line(ui, "e621ApiKey", "e621 API key", true);
        self.line(ui, "derpibooruApiKey", "Derpibooru API key", true);
        self.line(ui, "tantabusApiKey", "Tantabus API key", true);
        self.line(ui, "gelbUserId", "Gelbooru user ID", false);
        self.line(ui, "gelbApiKey", "Gelbooru API key", true);
        self.line(ui, "rule34UserId", "Rule34 user ID", false);
        self.line(ui, "rule34ApiKey", "Rule34 API key", true);
    }

    fn tab_slideshow(&mut self, ui: &mut egui::Ui) {
        ui.heading("Slideshow");
        ui.horizontal(|ui| {
            ui.label("Seconds per slide");
            let mut secs = self.number("secondsPerSlide", 6.0);
            if ui.add(egui::DragValue::new(&mut secs).range(1.0..=600.0)).changed() {
                self.set("secondsPerSlide", json!(secs));
            }
        });
        self.check(ui, "autoFitSlide", true, "Fit the picture to the window");
        ui.add_enabled_ui(!self.flag("autoFitSlide", true), |ui| {
            for (key, label) in [("maxWidth", "Max width (px)"), ("maxHeight", "Max height (px)")] {
                ui.horizontal(|ui| {
                    ui.label(label);
                    let mut v = self.number(key, 0.0);
                    if ui.add(egui::DragValue::new(&mut v).range(0.0..=20000.0)).changed() {
                        self.set(key, if v < 1.0 { Value::Null } else { json!(v) });
                    }
                    ui.label(RichText::new("0 = no limit").weak());
                });
            }
        });
        self.check(ui, "playVideosToEnd", false, "Play videos to the end before moving on");
        self.check(ui, "videoAutoplay", true, "Play videos automatically");
        self.check(ui, "videoAutoMute", false, "Start videos muted");
    }

    fn tab_filtering(&mut self, ui: &mut egui::Ui) {
        ui.heading("Filtering");
        ui.label("Applies to the next search.");
        for (key, default, label) in [
            ("includeImages", true, "Images"), ("includeGifs", true, "GIFs"), ("includeWebms", true, "Videos"),
            ("includeSafe", true, "Safe"), ("includeQuestionable", false, "Questionable"), ("includeExplicit", false, "Explicit"),
            ("includeDupes", false, "Include duplicates (same md5)"),
        ] {
            self.check(ui, key, default, label);
        }
        ui.label("None of the ratings ticked: no rating filter.");
        ui.separator();
        ui.label("Blacklist (tags, separated by spaces or lines)");
        let mut text = self.text("blacklist");
        if ui.add(egui::TextEdit::multiline(&mut text).desired_width(f32::INFINITY).desired_rows(5)).changed() {
            self.set("blacklist", json!(text));
        }
    }

    fn tab_hotkeys(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Hotkeys");
        ui.label(RichText::new("Click a key, then press the new one (Delete clears, Esc cancels). Fixed: Enter, F11, Esc, 1–0 (quick searches).").weak());
        // The key being recorded.
        if let Some((id, slot)) = self.recording.clone() {
            for event in ctx.input(|i| i.events.clone()) {
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
        egui::Grid::new("hotkeys").num_columns(2).show(ui, |ui| {
            for action in &ACTIONS {
                ui.label(action.label);
                ui.horizontal(|ui| {
                    let codes = self.hotkeys.map[action.id].clone();
                    for (slot, code) in codes.iter().enumerate() {
                        let recording = self.recording.as_ref().is_some_and(|(i, s)| i == action.id && *s == slot);
                        if ui.selectable_label(recording, if recording { "press a key…".to_string() } else { hotkeys::label(*code) }).clicked() {
                            self.recording = Some((action.id.to_string(), slot));
                        }
                    }
                    if ui.small_button("+").clicked() {
                        self.recording = Some((action.id.to_string(), codes.len()));
                    }
                });
                ui.end_row();
            }
        });
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
        ui.label("Your folders (the \"Your folders\" site; tick it in Sites):");
        let list = |app: &App, key: &str| -> Vec<String> { app.store.get(key).and_then(|v| v.as_array().cloned()).unwrap_or_default().iter().filter_map(|v| v.as_str().map(String::from)).collect() };
        let (mut folders, mut off) = (list(self, "localFolders"), list(self, "localFoldersOff"));
        let mut remove = None;
        for (i, folder) in folders.iter().enumerate() {
            ui.horizontal(|ui| {
                let mut on = !off.contains(folder);
                if ui.checkbox(&mut on, folder).changed() {
                    if on {
                        off.retain(|f| f != folder);
                    } else {
                        off.push(folder.clone());
                    }
                    self.set("localFoldersOff", json!(off));
                }
                if ui.small_button("✕").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            folders.remove(i);
            self.set("localFolders", json!(folders));
        }
        if ui.button("Add folder…").clicked() {
            if let Some(path) = self.pick_folder("Choose a folder with images and videos") {
                if !folders.contains(&path) {
                    folders.push(path);
                    self.set("localFolders", json!(folders));
                }
            }
        }
        ui.separator();
        let current = self.text("downloadFolder");
        ui.label(format!("Download folder: {}", if current.is_empty() { "Downloads/MSG (default)".to_string() } else { current }));
        ui.horizontal(|ui| {
            if ui.button("Choose…").clicked() {
                if let Some(path) = self.pick_folder("Choose the download folder") {
                    self.set("downloadFolder", json!(path));
                    self.settings_changed();
                }
            }
            if ui.button("Default").clicked() {
                self.set("downloadFolder", json!(""));
                self.settings_changed();
            }
        });
    }

    fn tab_favorites(&mut self, ui: &mut egui::Ui) {
        ui.heading("Favorites");
        ui.label(format!("{} favorites.", self.fav.items.len()));
        if ui.button("Import from e621, Derpibooru and Gelbooru").on_hover_text("Uses the details under Sites & accounts. Already-added items are skipped.").clicked() {
            super::favorites::import(&self.engine, self.tx.clone());
        }
        if !self.progress.is_empty() {
            ui.label(&self.progress);
        }
        self.check(ui, "syncE621Favorites", true, "Also fave and unfave on e621 (needs the e621 username and API key)");
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Download all favorites being shown").clicked() {
                let urls: Vec<String> = self.fav.view.iter().map(|s| s.file_url.clone()).filter(|u| super::net::web_url(u).is_some()).collect();
                self.downloader.download_many(urls, "MSG/favorites");
            }
            if ui.button("Stop").clicked() {
                self.downloader.cancel_bulk();
            }
        });
        ui.separator();
        ui.label("Offline copies (in the background, into the download folder):");
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
    }

    fn tab_quick(&mut self, ui: &mut egui::Ui) {
        ui.heading("Quick searches");
        ui.label("Keys 1–0 select the sites and search for the tags (or what is in the box when the tags are empty).");
        let mut quick: Vec<Value> = self.store.get("quickSearches").and_then(|v| v.as_array().cloned()).unwrap_or_default();
        while quick.len() < 10 {
            quick.push(json!({"sites": [], "tags": ""}));
        }
        let mut changed = false;
        for (n, entry) in quick.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{}", (n + 1) % 10));
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
                let mut tags = entry["tags"].as_str().unwrap_or("").to_string();
                if ui.add(egui::TextEdit::singleline(&mut tags).desired_width(300.0).hint_text("tags")).changed() {
                    entry["tags"] = json!(tags);
                    changed = true;
                }
            });
        }
        if changed {
            self.set("quickSearches", Value::Array(quick));
        }
    }

    fn tab_history(&mut self, ui: &mut egui::Ui) {
        ui.heading("History");
        self.check(ui, "storeHistory", true, "Remember my searches");
        if ui.button("Clear the history").clicked() {
            self.set("searchHistory", json!([]));
        }
    }

    fn tab_about(&mut self, ui: &mut egui::Ui) {
        ui.heading(format!("MSG {}", env!("CARGO_PKG_VERSION")));
        ui.label("A desktop slideshow for booru sites, based on Chirmaya's BooruSlideshow 10.6.");
        ui.separator();
        ui.label("Original license (Chirmaya): open license to copy/modify as long as it is not published under the name \"Booru Slideshow\".");
        ui.label("Libraries: egui/eframe (MIT or Apache-2.0), image, reqwest, tokio and others under MIT or Apache-2.0.");
        ui.hyperlink_to("Source", "https://github.com/290asd/MSG");
    }

    pub fn save_pool(&mut self, id: u64, name: String, count: usize, cover: String) {
        let mut pools: Vec<Value> = self.store.get("savedPools").and_then(|v| v.as_array().cloned()).unwrap_or_default();
        pools.retain(|p| p["id"].as_u64() != Some(id));
        pools.push(json!({"id": id, "name": name, "count": count, "cover": cover}));
        self.set("savedPools", Value::Array(pools));
        self.notice(format!("Saved the pool “{name}”."));
        self.settings_changed();
    }

    pub fn pools_window(&mut self, ctx: &egui::Context) {
        if !self.show_pools {
            return;
        }
        let mut open = true;
        let mut pools: Vec<Value> = self.store.get("savedPools").and_then(|v| v.as_array().cloned()).unwrap_or_default();
        let mut go = None;
        let mut remove = None;
        egui::Window::new("Pools").open(&mut open).default_size([560.0, 480.0]).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Sort");
                for (value, name) in [("added", "Recently added"), ("name", "Name"), ("pages", "Most pages")] {
                    if ui.radio(self.pool_sort == value, name).clicked() {
                        self.pool_sort = value.into();
                        self.set("poolsSort", json!(value));
                    }
                }
            });
            match self.pool_sort.as_str() {
                "name" => pools.sort_by_key(|p| p["name"].as_str().unwrap_or("").to_lowercase()),
                "pages" => pools.sort_by_key(|p| std::cmp::Reverse(p["count"].as_u64().unwrap_or(0))),
                _ => pools.reverse(),
            }
            if pools.is_empty() {
                ui.label("No saved pools. Search pool:<number> (or paste a pool link) and press ☆ Save pool.");
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                for pool in &pools {
                    let id = pool["id"].as_u64().unwrap_or(0);
                    ui.horizontal(|ui| {
                        let cover = pool["cover"].as_str().unwrap_or("").to_string();
                        let cover = super::downloads::display_url(&self.copies, &cover);
                        self.media.request(&self.engine, &cover);
                        let (rect, response) = ui.allocate_exact_size(Vec2::new(72.0, 96.0), egui::Sense::click());
                        ui.painter().rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
                        if let Some(State::Ready(p)) = self.media.get(&cover) {
                            let (w, h) = (p.size[0] as f32, p.size[1] as f32);
                            let scale = (rect.width() / w).min(rect.height() / h);
                            let r = egui::Rect::from_center_size(rect.center(), Vec2::new(w * scale, h * scale));
                            ui.painter().image(p.frames[0].texture.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                        }
                        if response.clicked() {
                            go = Some(id);
                        }
                        ui.vertical(|ui| {
                            ui.label(RichText::new(pool["name"].as_str().unwrap_or("?")).strong());
                            ui.label(format!("{} pages", pool["count"].as_u64().unwrap_or(0)));
                            ui.horizontal(|ui| {
                                if ui.button("Open").clicked() {
                                    go = Some(id);
                                }
                                if ui.button("Remove").clicked() {
                                    remove = Some(id);
                                }
                            });
                        });
                    });
                    ui.separator();
                }
            });
        });
        if let Some(id) = remove {
            let mut all: Vec<Value> = self.store.get("savedPools").and_then(|v| v.as_array().cloned()).unwrap_or_default();
            all.retain(|p| p["id"].as_u64() != Some(id));
            self.set("savedPools", Value::Array(all));
        }
        if let Some(id) = go {
            self.show_pools = false;
            self.start_search(&format!("pool:{id}"));
        }
        if !open {
            self.show_pools = false;
        }
    }
}
