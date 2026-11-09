// The Pools page: every saved pool on a row of its own, its pages as a stack in perspective. The page in front stands
// at the left, facing you, and the rest go off to the right behind it. Rows are read as they come into view.
use super::app::{App, TOP_INSET};
use super::media::{self, State};
use super::session::{self, Search};
use super::sites::Site;
use super::theme::{self, icon, tok};
use eframe::egui::epaint::{Mesh, Vertex, WHITE_UV};
use eframe::egui::scroll_area::ScrollSource;
use eframe::egui::{self, Color32, Rect, RichText, Sense, Vec2};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// How far the pages behind the front one are turned (radians).
const TILT: f32 = 0.87;
const COLUMNS: u32 = 10;
/// How much room a row's name takes above its pages, and the space under them.
const HEADER: f32 = 28.0;
const ROW_GAP: f32 = 20.0;
/// A row's height with its name and gap, until Ctrl and the wheel change it (poolRowHeight).
const DEFAULT_ROW: f64 = 340.0;
/// A row is read at most this often: e621 allows two requests a second, and opening a pool takes two.
const ASK_EVERY: Duration = Duration::from_millis(1100);
/// How many pages are drawn at most, and how many past those are loaded ahead.
const MOST_DRAWN: usize = 40;
const LOAD_AHEAD: usize = 3;

/// A row's pages and where it has been browsed to.
#[derive(Default)]
pub struct PoolRow {
    search: Option<Search>,
    pos: f32,
    target: f32,
    wheel: f32,
    /// Where the pages ended on the last frame: past that the wheel scrolls the page.
    reach: f32,
}

impl PoolRow {
    fn pages(&self) -> usize {
        self.search.as_ref().map_or(0, |s| s.slides.len())
    }

    /// Takes in a page of results if they are this row's.
    pub fn accept(&mut self, batch: session::Batch) -> Option<session::Batch> {
        match &mut self.search {
            Some(search) if search.generation == batch.generation => {
                search.accept(batch);
                None
            }
            _ => Some(batch),
        }
    }
}

/// A page's place in the stack, `r` pages after the front one (below zero: gone by).
#[derive(Debug, PartialEq)]
struct Place {
    center: f32,
    scale: f32,
    tilt: f32,
    shade: f32,
    alpha: f32,
}

fn place(r: f32, left: f32, front_width: f32, height: f32) -> Place {
    let flat = left + front_width / 2.0;
    if r < 0.0 {
        return Place { center: flat + r * front_width * 0.9, scale: 1.0, tilt: 0.0, shade: 1.0, alpha: (1.0 + r).max(0.0) };
    }
    let unit = height * 0.72;
    let near = r.min(1.0);
    let turned = near * near * (3.0 - 2.0 * near);
    let far = (r - 1.0).max(0.0);
    Place {
        center: egui::lerp(flat..=left + front_width + unit * 0.2, turned) + far * unit * 0.36,
        scale: egui::lerp(1.0..=0.86, turned) * 0.975f32.powf(far).max(0.62),
        tilt: TILT * turned,
        shade: egui::lerp(1.0..=0.82, turned) * 0.965f32.powf(far).max(0.42),
        alpha: 1.0,
    }
}

/// A page's picture on a plane turned around its middle, its left edge away from you. In columns, each with the true
/// perspective at its edges, as one quad would bend the picture along its diagonal. Returns what it covers.
fn paint_plane(painter: &egui::Painter, texture: Option<egui::TextureId>, surface: Color32, center: Vec2, size: Vec2, tilt: f32, shade: f32, alpha: f32) -> Rect {
    let mut mesh = texture.map_or_else(Mesh::default, Mesh::with_texture);
    let focal = size.y * 2.4;
    let mut covers = Rect::NOTHING;
    for k in 0..=COLUMNS {
        let u = k as f32 / COLUMNS as f32;
        let along = (u - 0.5) * size.x;
        let perspective = focal / (focal - along * tilt.sin());
        let x = center.x + along * tilt.cos() * perspective;
        let half = size.y / 2.0 * perspective;
        // The edge going away is in the shadow of the page before it.
        let light = shade * egui::lerp(1.0..=0.62 + 0.38 * u, tilt / TILT);
        let color = if texture.is_some() { Color32::from_gray((light * 255.0) as u8) } else { surface.gamma_multiply(light) }.gamma_multiply(alpha);
        let base = mesh.vertices.len() as u32;
        let (top, bottom) = (egui::pos2(x, center.y - half), egui::pos2(x, center.y + half));
        let (uv_top, uv_bottom) = if texture.is_some() { (egui::pos2(u, 0.0), egui::pos2(u, 1.0)) } else { (WHITE_UV, WHITE_UV) };
        mesh.vertices.push(Vertex { pos: top, uv: uv_top, color });
        mesh.vertices.push(Vertex { pos: bottom, uv: uv_bottom, color });
        if k > 0 {
            mesh.add_triangle(base - 2, base - 1, base);
            mesh.add_triangle(base - 1, base, base + 1);
        }
        covers.extend_with(top);
        covers.extend_with(bottom);
    }
    painter.add(egui::Shape::mesh(mesh));
    covers
}

/// One page of a row, as far as the drawing goes.
struct Page {
    index: usize,
    preview: String,
    sample: String,
    /// Width ÷ height.
    aspect: f32,
}

impl App {
    /// The pools page's part of the top bar, right to left (see top_bar).
    pub fn pools_bar(&mut self, ui: &mut egui::Ui) {
        if theme::icon_button(ui, icon::BACK, "Back", false).clicked() {
            self.go_back();
        }
        // One e621 job at a time (the favorites import's progress shows here too).
        let busy = !self.progress.is_empty();
        let find = ui.add_enabled(!busy, egui::Button::new("Find pools in favorites")).on_hover_text("Adds the e621 pools that your e621 favorites are in.");
        if find.clicked() {
            let favorites = self.fav.items.iter().filter(|s| s.site_id == "E621").map(|s| super::slide::id_text(&s.id)).collect();
            let saved = self.store.get("savedPools").and_then(|v| v.as_array().cloned()).unwrap_or_default().iter().filter_map(|p| p["id"].as_u64()).collect();
            session::find_pools(&self.engine, self.tx.clone(), favorites, saved);
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
        ui.add(egui::TextEdit::singleline(&mut self.pool_filter).hint_text("Filter pools by name…").desired_width(f32::INFINITY).margin(egui::Margin::symmetric(10, 7)));
    }

    /// The saved pools, one row each. Only the rows on the screen are drawn; those and the next screenful are read.
    pub fn pools_page(&mut self, ui: &mut egui::Ui) {
        let words: Vec<String> = self.pool_filter.to_lowercase().split_whitespace().map(String::from).collect();
        let mut pools = session::saved_pools(&self.store);
        pools.retain(|p| {
            let name = p["name"].as_str().unwrap_or("").to_lowercase();
            words.iter().all(|w| name.contains(w.as_str()))
        });
        let area = ui.available_rect_before_wrap();
        let t = tok(ui.ctx());
        if pools.is_empty() {
            let text = if words.is_empty() { "No saved pools. Search pool:<number> or paste a pool link and press Save pool, or use Find pools in favorites." } else { "No pools match." };
            let galley = ui.painter().layout(text.to_string(), egui::FontId::proportional(15.0), t.text2, (area.width() - 2.0 * theme::S6).max(100.0));
            ui.painter().galley(area.center() - galley.size() / 2.0, galley, t.text2);
            return;
        }
        // Ctrl and the wheel make the rows taller or lower, up to one to a screen.
        let view = area.height() - TOP_INSET;
        let zoom = ui.input(|i| i.zoom_delta());
        if zoom != 1.0 {
            self.set("poolRowHeight", json!((self.number("poolRowHeight", DEFAULT_ROW) * zoom as f64).clamp(180.0, 900.0)));
        }
        let row_height = (self.number("poolRowHeight", DEFAULT_ROW) as f32).min(view.max(180.0));
        let total = TOP_INSET + pools.len() as f32 * row_height + theme::S3;
        // The wheel belongs to the row under it; the page scrolls with the bar, over the names and gaps, and with Shift.
        let source = ScrollSource { drag: egui::containers::scroll_area::DragScroll::Never, mouse_wheel: !self.pool_wheel_taken, ..ScrollSource::ALL };
        self.pool_wheel_taken = false;
        let (mut go, mut remove) = (None, None);
        egui::ScrollArea::vertical().id_salt("pools").auto_shrink(false).scroll_source(source).show_viewport(ui, |ui, viewport| {
            ui.set_height(total);
            let origin = ui.max_rect().min.to_vec2();
            for (n, pool) in pools.iter().enumerate() {
                let top = TOP_INSET + n as f32 * row_height;
                let cell = Rect::from_min_size(egui::pos2(theme::S1, top), Vec2::new(area.width() - theme::S4, row_height - ROW_GAP));
                let id = pool["id"].as_u64().unwrap_or(0);
                if cell.intersects(viewport) {
                    self.pool_row(ui, cell.translate(origin), pool, id, &mut go, &mut remove);
                }
                // The row's first pages, on the screen first and then the next screenful, one row at a time.
                if cell.top() < viewport.bottom() + view && cell.bottom() > viewport.top() - view {
                    self.read_pool_row(ui.ctx(), id, cell.intersects(viewport));
                }
            }
            for event in ui.input(|i| i.events.clone()) {
                if let egui::Event::MouseWheel { unit, delta, modifiers, .. } = event {
                    if modifiers.shift {
                        let lines = if delta.y != 0.0 { delta.y } else { delta.x };
                        ui.scroll_with_delta(Vec2::new(0.0, if unit == egui::MouseWheelUnit::Point { lines } else { lines * 90.0 }));
                    }
                }
            }
        });
        if let Some(id) = remove {
            let mut all: Vec<Value> = self.store.get("savedPools").and_then(|v| v.as_array().cloned()).unwrap_or_default();
            all.retain(|p| p["id"].as_u64() != Some(id));
            self.set("savedPools", Value::Array(all));
            self.pool_rows.remove(&id);
        }
        if let Some((id, page)) = go {
            self.start_search(&format!("pool:{id}"));
            // The first answer has a hundred pages; further in, the pool opens from its start.
            if page < 100 {
                self.open_at(page);
            }
        }
    }

    /// Starts reading a row's pages (or more of them) when it is its turn.
    fn read_pool_row(&mut self, ctx: &egui::Context, id: u64, on_screen: bool) {
        if self.flag("offlineMode", false) {
            return;
        }
        let row = self.pool_rows.entry(id).or_default();
        let wants = match &row.search {
            None => true,
            Some(search) => !search.loading && search.has_more() && row.target as usize + 25 > search.slides.len(),
        };
        // A row on the screen can't be overtaken by the ones waiting below it.
        if !wants || (!on_screen && self.pool_waiting_on_screen) {
            self.pool_waiting_on_screen |= wants && on_screen;
            return;
        }
        self.pool_waiting_on_screen |= on_screen;
        let wait = ASK_EVERY.saturating_sub(self.pool_asked.elapsed());
        if !wait.is_zero() {
            ctx.request_repaint_after(wait);
            return;
        }
        self.pool_asked = Instant::now();
        let generation = self.next_generation();
        let row = self.pool_rows.entry(id).or_default();
        let search = row.search.get_or_insert_with(|| Search::new(generation, &format!("pool:{id}"), vec![Site::E621], &self.store));
        search.load_more(&self.engine, self.tx.clone());
    }

    fn pool_row(&mut self, ui: &mut egui::Ui, cell: Rect, pool: &Value, id: u64, go: &mut Option<(u64, usize)>, remove: &mut Option<u64>) {
        let ctx = ui.ctx().clone();
        let t = tok(&ctx);
        let mut row = self.pool_rows.remove(&id).unwrap_or_default();
        let name = pool["name"].as_str().unwrap_or("?").to_string();
        let row_hovered = ui.rect_contains_pointer(cell);
        let stack = Rect::from_min_max(cell.min + Vec2::new(0.0, HEADER), cell.max);
        let height = stack.height();
        let pages = row.pages();
        let last = pages.saturating_sub(1) as f32;

        // Browsing: the wheel (or a touchpad's sideways swipe), dragging, the buttons, and clicking a page.
        let response = ui.interact(stack, ui.id().with(("pool stack", id)), Sense::click_and_drag());
        let step = (height * 0.72 * 0.36).max(1.0);
        if ui.rect_contains_pointer(stack) && ui.input(|i| i.pointer.hover_pos()).is_some_and(|p| p.x <= row.reach) {
            for event in ui.input(|i| i.events.clone()) {
                let egui::Event::MouseWheel { unit, delta, modifiers, .. } = event else { continue };
                if modifiers.shift || modifiers.ctrl {
                    continue;
                }
                self.pool_wheel_taken = true;
                let along = if delta.x.abs() > delta.y.abs() { delta.x } else { delta.y };
                if unit == egui::MouseWheelUnit::Point {
                    row.wheel += along;
                    let steps = (row.wheel / 40.0).trunc();
                    row.wheel -= steps * 40.0;
                    row.target -= steps;
                } else if along != 0.0 {
                    row.target = row.target.round() - along.signum();
                }
            }
            // Next frame too: the page must not scroll under a row being browsed.
            self.pool_wheel_taken = true;
        }
        if response.dragged() {
            row.target -= response.drag_delta().x / step;
            row.pos = row.target.clamp(0.0, last);
        }
        if response.drag_stopped() {
            row.target = row.target.round();
        }
        row.target = row.target.clamp(0.0, last);
        let tau = theme::quick();
        row.pos = if tau == 0.0 { row.target } else { row.pos + (row.target - row.pos) * (1.0 - (-ui.input(|i| i.stable_dt).min(0.05) / tau).exp()) };
        if (row.target - row.pos).abs() < 0.002 {
            row.pos = row.target;
        } else {
            ctx.request_repaint();
        }

        // The pages on the screen, and a few more to have ready. Before a row has been read, its cover.
        let height_of = |aspect: f32| if aspect > 1.3 { height * 1.3 / aspect } else { height };
        let left = stack.left() + theme::S1;
        let mut shown: Vec<Page> = vec![];
        let first = (row.pos.floor() as i64 - 1).max(0) as usize;
        let slides = row.search.as_ref().map_or(&[][..], |s| s.slides.as_slice());
        let front_aspect = slides.get(row.target as usize).map_or(0.7, slide_aspect);
        let front_width = height_of(front_aspect) * front_aspect;
        let mut extra = 0;
        for (index, slide) in slides.iter().enumerate().skip(first) {
            if shown.len() >= MOST_DRAWN + LOAD_AHEAD {
                break;
            }
            let beyond = place(index as f32 - row.pos, left, front_width, height).center > stack.right() + height;
            extra += usize::from(beyond);
            if extra > LOAD_AHEAD {
                break;
            }
            let video = slide.media_type == super::slide::MediaType::Video;
            shown.push(Page { index, preview: slide.preview_file_url.clone(), sample: slide.sample_file_url.clone().filter(|u| !video && !u.is_empty()).unwrap_or_else(|| slide.preview_file_url.clone()), aspect: slide_aspect(slide) });
        }
        if shown.is_empty() {
            shown.push(Page { index: 0, preview: pool["cover"].as_str().unwrap_or("").to_string(), sample: String::new(), aspect: 0.7 });
        }

        // The cover first, then the rest from the near end, as the downloads wait in the order they were asked for.
        for page in &mut shown {
            page.preview = super::downloads::display_url(&self.copies, &page.preview);
            page.sample = super::downloads::display_url(&self.copies, &page.sample);
            self.media.request(&self.engine, &page.preview);
        }
        let hover = ui.input(|i| i.pointer.hover_pos()).filter(|_| ui.rect_contains_pointer(stack));
        let front = row.target.round() as usize;
        let at_rest = row.pos == row.target && !response.dragged();
        let mut covers: Vec<(usize, Rect)> = vec![];
        let painter = ui.painter().with_clip_rect(stack.expand(theme::S2).intersect(ui.clip_rect()));
        for page in shown.iter().rev() {
            let r = page.index as f32 - row.pos;
            if r <= -1.0 {
                continue;
            }
            let p = place(r, left, front_width, height);
            if p.center - height > stack.right() {
                continue;
            }
            let picture_height = height_of(page.aspect);
            let size = Vec2::new(picture_height * page.aspect, picture_height) * p.scale;
            let middle = Vec2::new(p.center, stack.center().y);
            // The page in front is looked at straight on, sharp if its larger picture has come.
            if at_rest && page.index == front && r == 0.0 {
                let place_rect = Rect::from_center_size(middle.to_pos2(), size);
                painter.add(egui::Shadow { offset: [0, 8], blur: 26, spread: 0, color: Color32::from_black_alpha(130) }.as_shape(place_rect, theme::RADIUS_S));
                painter.rect_filled(place_rect, theme::RADIUS_S, t.surface);
                self.media.request(&self.engine, &page.sample);
                let url = if matches!(self.media.get(&page.sample), Some(State::Ready(_))) { &page.sample } else { &page.preview };
                if let Some(State::Ready(picture)) = self.media.get(url) {
                    media::paint(ui, place_rect, &picture.frames[0].texture, picture.size, 1.0, theme::RADIUS_S as f32);
                }
                if row_hovered {
                    painter.hline(place_rect.x_range(), place_rect.bottom() + 5.0, egui::Stroke::new(2.0, t.accent));
                }
                covers.push((page.index, place_rect));
                continue;
            }
            let texture = match self.media.get(&page.preview) {
                Some(State::Ready(picture)) => Some(picture.frames[0].texture.id()),
                _ => None,
            };
            let lit = hover.is_some_and(|h| covers.iter().all(|(_, c)| !c.contains(h)) && (p.center - h.x).abs() < size.x * 0.25);
            covers.push((page.index, paint_plane(&painter, texture, t.surface, middle, size, p.tilt, (p.shade + if lit { 0.16 } else { 0.0 }).min(1.0), p.alpha)));
        }

        row.reach = covers.iter().map(|(_, c)| c.right()).fold(0.0, f32::max);
        // What is under the pointer: the nearest page there is the last one drawn.
        let hit = hover.and_then(|h| covers.iter().rev().find(|(_, c)| c.contains(h)).map(|(i, _)| *i));
        if hit.is_some() && !response.dragged() {
            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if response.clicked() {
            match hit {
                Some(i) if i == front => *go = Some((id, i)),
                Some(i) => row.target = i as f32,
                None => {}
            }
        }

        // The name and the small print above the pages; the buttons when the pointer is on the row.
        let header = Rect::from_min_size(cell.min + Vec2::new(theme::S1, 0.0), Vec2::new(cell.width() - theme::S1, HEADER - 4.0));
        let count = pool["count"].as_u64().unwrap_or(0);
        let artist = row.search.as_ref().and_then(|s| s.slides.first()).and_then(super::settings_ui::artist_of);
        let mut job = egui::text::LayoutJob::default();
        let append = |job: &mut egui::text::LayoutJob, text: &str, font: egui::FontId, color: Color32| job.append(text, 0.0, egui::TextFormat::simple(font, color));
        append(&mut job, &name, theme::semibold(14.0), t.text);
        append(&mut job, &format!("   {count} pages"), egui::FontId::proportional(11.5), t.muted);
        if let Some(artist) = &artist {
            append(&mut job, &format!("   {}", artist.replace('_', " ")), egui::FontId::proportional(11.5), t.apricot);
        }
        if front > 0 {
            append(&mut job, &format!("   {} / {count}", front + 1), theme::medium(11.5), t.accent);
        }
        job.wrap.max_width = header.width() - 120.0;
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        let galley = ui.painter().layout_job(job);
        ui.painter().galley(egui::pos2(header.left(), header.center().y - galley.size().y / 2.0), galley, t.text);
        if row_hovered {
            ui.scope_builder(egui::UiBuilder::new().max_rect(header).layout(egui::Layout::right_to_left(egui::Align::Center)), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                if theme::icon_button(ui, icon::CLOSE, "Remove from the saved pools", false).clicked() {
                    *remove = Some(id);
                }
                ui.add_enabled_ui((front as f32) < last, |ui| {
                    if theme::icon_button(ui, icon::FORWARD, "Next page", false).clicked() {
                        row.target += 1.0;
                    }
                });
                ui.add_enabled_ui(front > 0, |ui| {
                    if theme::icon_button(ui, icon::BACK, "Previous page", false).clicked() {
                        row.target -= 1.0;
                    }
                });
            });
        }
        self.pool_rows.insert(id, row);
    }
}

fn slide_aspect(slide: &super::slide::Slide) -> f32 {
    if slide.width > 0 && slide.height > 0 { (slide.width as f32 / slide.height as f32).clamp(0.35, 2.4) } else { 0.7 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_front_page_is_flat_at_the_left_and_the_rest_go_right_and_recede() {
        let front = place(0.0, 10.0, 100.0, 200.0);
        assert_eq!((front.center, front.scale, front.tilt, front.shade), (60.0, 1.0, 0.0, 1.0));
        let (a, b, c) = (place(1.0, 10.0, 100.0, 200.0), place(2.0, 10.0, 100.0, 200.0), place(30.0, 10.0, 100.0, 200.0));
        assert!(a.center > front.center && b.center > a.center && c.center > b.center);
        assert!(a.scale < 1.0 && b.scale < a.scale && c.scale >= 0.86 * 0.62);
        assert!(a.tilt == TILT && b.tilt == TILT);
        let gone = place(-0.5, 10.0, 100.0, 200.0);
        assert!(gone.center < front.center && gone.alpha == 0.5);
    }
}
