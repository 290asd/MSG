// The look of the window: the colours, sizes, fonts and icons, and the small widgets every page shares.
use eframe::egui::epaint::text::VariationCoords;
use eframe::egui::style::{ScrollStyle, TextCursorStyle, WidgetVisuals};
use eframe::egui::{self, Color32, CornerRadius, CursorIcon, FontData, FontDefinitions, FontFamily, FontId, FontTweak, Margin, RichText, Sense, Shadow, Stroke, TextStyle, Theme, Vec2, Visuals};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

/// The spacing scale (4 px steps).
pub const S1: f32 = 4.0;
pub const S2: f32 = 8.0;
pub const S3: f32 = 12.0;
pub const S4: f32 = 16.0;
pub const S6: f32 = 24.0;

pub const RADIUS_S: u8 = 6;
pub const RADIUS_M: u8 = 10;
pub const RADIUS_L: u8 = 14;

/// Lucide's code points (https://lucide.dev), drawn with `glyph`.
pub mod icon {
    pub const HOME: char = '\u{e0f5}';
    pub const MOSAIC: char = '\u{e0ff}';
    pub const SAVED: char = '\u{e060}';
    pub const SLIDESHOW: char = '\u{e080}';
    pub const PLAY: char = '\u{e13c}';
    pub const PAUSE: char = '\u{e12e}';
    pub const PREVIOUS: char = '\u{e15f}';
    pub const NEXT: char = '\u{e160}';
    pub const HEART: char = '\u{e0f2}';
    pub const DOWNLOAD: char = '\u{e0b2}';
    pub const POST: char = '\u{e0b9}';
    pub const TAGS: char = '\u{e0ef}';
    pub const THUMBS: char = '\u{e4cf}';
    pub const FIT: char = '\u{e21a}';
    pub const SEARCH: char = '\u{e151}';
    pub const HISTORY: char = '\u{e1f5}';
    pub const FILTERS: char = '\u{e29a}';
    pub const SITES: char = '\u{e0e8}';
    pub const FOLLOW: char = '\u{e060}';
    pub const FOLLOWING: char = '\u{e51f}';
    pub const POOLS: char = '\u{e100}';
    pub const SETTINGS: char = '\u{e154}';
    pub const BACK: char = '\u{e06e}';
    pub const FORWARD: char = '\u{e06f}';
    pub const CLOSE: char = '\u{e1b2}';
    pub const RANDOM: char = '\u{e15e}';
    pub const VOLUME: char = '\u{e1ab}';
    pub const MUTED: char = '\u{e1ac}';
    pub const PALETTE: char = '\u{e1dd}';
    pub const KEYBOARD: char = '\u{e284}';
    pub const FOLDER: char = '\u{e0d7}';
    pub const INFO: char = '\u{e0f9}';
}

/// The colours of a theme.
#[derive(Clone, Copy)]
pub struct Tokens {
    /// The window under everything.
    pub base: Color32,
    /// Cards, groups, a picture still on its way.
    pub surface: Color32,
    /// Menus and tooltips.
    pub raised: Color32,
    /// The rail of a slider or switch.
    pub top: Color32,
    pub text: Color32,
    pub text2: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub apricot: Color32,
    pub lavender: Color32,
    /// The fine edge of a surface.
    pub hairline: Color32,
    /// A control at rest, under the pointer, and pressed or selected.
    pub control: Color32,
    pub hover: Color32,
    pub press: Color32,
    /// What lies over the blurred window in a glass surface, and over the blurred picture behind everything.
    pub glass: Color32,
    pub ambient: Color32,
}

// Written with the alpha this way because a Color32 holds premultiplied values.
const fn alpha(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied_const(r, g, b, a)
}

pub const DARK: Tokens = Tokens {
    base: Color32::from_rgb(0x17, 0x19, 0x1f),
    surface: Color32::from_rgb(0x20, 0x23, 0x2b),
    raised: Color32::from_rgb(0x29, 0x2d, 0x37),
    top: Color32::from_rgb(0x36, 0x3b, 0x48),
    text: Color32::from_rgb(0xf1, 0xf1, 0xf3),
    text2: Color32::from_rgb(0xb8, 0xbc, 0xc8),
    muted: Color32::from_rgb(0x77, 0x7f, 0x90),
    accent: Color32::from_rgb(0xff, 0x96, 0x78),
    apricot: Color32::from_rgb(0xff, 0xb3, 0x6b),
    lavender: Color32::from_rgb(0xd9, 0xa7, 0xff),
    hairline: alpha(255, 255, 255, 24),
    control: alpha(255, 255, 255, 14),
    hover: alpha(255, 255, 255, 30),
    press: alpha(0xff, 0x96, 0x78, 60),
    glass: alpha(0x1c, 0x1f, 0x27, 150),
    ambient: alpha(0x17, 0x19, 0x1f, 190),
};

pub const LIGHT: Tokens = Tokens {
    base: Color32::from_rgb(0xf3, 0xef, 0xe9),
    surface: Color32::from_rgb(0xfb, 0xf8, 0xf3),
    raised: Color32::from_rgb(0xff, 0xff, 0xff),
    top: Color32::from_rgb(0xdd, 0xd7, 0xcd),
    text: Color32::from_rgb(0x1e, 0x20, 0x26),
    text2: Color32::from_rgb(0x4e, 0x54, 0x62),
    muted: Color32::from_rgb(0x7c, 0x82, 0x90),
    accent: Color32::from_rgb(0xd9, 0x57, 0x37),
    apricot: Color32::from_rgb(0xc2, 0x74, 0x26),
    lavender: Color32::from_rgb(0x8a, 0x58, 0xc4),
    hairline: alpha(0, 0, 0, 26),
    control: alpha(0, 0, 0, 12),
    hover: alpha(0, 0, 0, 26),
    press: alpha(0xd9, 0x57, 0x37, 56),
    glass: alpha(0xfa, 0xf7, 0xf2, 170),
    ambient: alpha(0xf3, 0xef, 0xe9, 205),
};

pub fn tok(ctx: &egui::Context) -> &'static Tokens {
    if ctx.theme() == Theme::Light { &LIGHT } else { &DARK }
}

/// The Animations setting: 0 off, 1 fast, 2 slow.
static SPEED: AtomicU8 = AtomicU8::new(1);

/// How long a small change takes (seconds), a larger one, and the picture behind everything.
pub fn quick() -> f32 {
    [0.0, 0.05, 0.15][SPEED.load(Ordering::Relaxed) as usize]
}

pub fn smooth() -> f32 {
    [0.0, 0.09, 0.22][SPEED.load(Ordering::Relaxed) as usize]
}

/// The slideshow's drawer keeps its unhurried slide unless animations are off.
pub fn drawer_slide() -> f32 {
    if SPEED.load(Ordering::Relaxed) == 0 { 0.0 } else { 0.22 }
}

pub fn fade() -> f32 {
    [0.0, 0.14, 0.45][SPEED.load(Ordering::Relaxed) as usize]
}

/// Takes the Animations setting into use. Not set: fast, or off if Windows is set to show no animations.
pub fn set_motion(ctx: &egui::Context, setting: &str) {
    const GET_CLIENT_AREA_ANIMATION: u32 = 0x1042;
    let speed = match setting {
        "off" => 0,
        "slow" => 2,
        "fast" => 1,
        _ => {
            let mut animate = 1i32;
            let read = unsafe { SystemParametersInfoW(GET_CLIENT_AREA_ANIMATION, 0, (&raw mut animate).cast(), 0) } != 0;
            if read && animate == 0 { 0 } else { 1 }
        }
    };
    SPEED.store(speed, Ordering::Relaxed);
    ctx.all_styles_mut(|style| style.animation_time = quick());
}

fn family(name: &str) -> FontFamily {
    FontFamily::Name(name.into())
}

pub fn medium(size: f32) -> FontId {
    FontId::new(size, family("medium"))
}

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, family("semibold"))
}

/// An icon as text.
pub fn glyph(icon: char, size: f32) -> RichText {
    RichText::new(icon.to_string()).font(FontId::new(size, family("icons")))
}

fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let inter: &'static [u8] = include_bytes!("../img/fonts/Inter.ttf");
    for (name, weight) in [("body", 420.0), ("medium", 520.0), ("semibold", 620.0)] {
        let tweak = FontTweak { coords: VariationCoords::new([("wght", weight)]), ..Default::default() };
        fonts.font_data.insert(name.to_string(), Arc::new(FontData::from_static(inter).tweak(tweak)));
    }
    fonts.font_data.insert("lucide".to_string(), Arc::new(FontData::from_static(include_bytes!("../img/fonts/lucide.ttf")).tweak(FontTweak { y_offset_factor: 0.07, ..Default::default() })));
    // egui's own fonts stay behind Inter: tags can be in any script.
    let fallback = fonts.families[&FontFamily::Proportional].clone();
    for name in ["medium", "semibold"] {
        fonts.families.insert(family(name), std::iter::once(name.to_string()).chain(fallback.iter().cloned()).collect());
    }
    fonts.families.get_mut(&FontFamily::Proportional).expect("egui has it").insert(0, "body".to_string());
    fonts.families.insert(family("icons"), vec!["lucide".to_string()]);
    fonts
}

fn visuals(t: &Tokens, dark: bool) -> Visuals {
    let mut v = if dark { Visuals::dark() } else { Visuals::light() };
    let shadow = Shadow { offset: [0, 6], blur: 24, spread: 0, color: Color32::from_black_alpha(if dark { 110 } else { 40 }) };
    v.panel_fill = t.base;
    v.window_fill = t.raised;
    v.window_stroke = Stroke::new(1.0, t.hairline);
    v.window_corner_radius = RADIUS_M.into();
    v.menu_corner_radius = RADIUS_M.into();
    v.window_shadow = shadow;
    v.popup_shadow = shadow;
    v.faint_bg_color = t.surface;
    v.extreme_bg_color = t.surface;
    v.code_bg_color = t.surface;
    v.text_edit_bg_color = Some(t.control);
    v.hyperlink_color = t.accent;
    v.selection.bg_fill = t.accent.gamma_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0, t.accent);
    v.weak_text_color = Some(t.muted);
    v.text_cursor = TextCursorStyle { stroke: Stroke::new(1.5, t.accent), ..v.text_cursor };
    v.slider_trailing_fill = true;
    v.interact_cursor = Some(CursorIcon::PointingHand);
    v.indent_has_left_vline = false;
    let look = |bg_fill, weak_bg_fill, fg: Color32| WidgetVisuals { bg_fill, weak_bg_fill, bg_stroke: Stroke::NONE, corner_radius: CornerRadius::same(RADIUS_S), fg_stroke: Stroke::new(1.0, fg), expansion: 0.0 };
    v.widgets.noninteractive = WidgetVisuals { bg_stroke: Stroke::new(1.0, t.hairline), ..look(t.surface, t.surface, t.text) };
    v.widgets.inactive = look(t.top, t.control, t.text);
    v.widgets.hovered = look(t.muted, t.hover, t.text);
    v.widgets.active = look(t.accent, t.press, t.text);
    v.widgets.open = look(t.top, t.hover, t.text);
    v
}

#[link(name = "user32")]
unsafe extern "system" {
    fn SystemParametersInfoW(action: u32, param: u32, value: *mut std::ffi::c_void, flags: u32) -> i32;
}

#[link(name = "dwmapi")]
unsafe extern "system" {
    fn DwmSetWindowAttribute(window: isize, attribute: u32, value: *const std::ffi::c_void, size: u32) -> i32;
}

/// The title bar in the window's own colour (Windows 11; elsewhere the call does nothing).
pub fn paint_title_bar(window: isize, t: &Tokens, dark: bool) {
    const IMMERSIVE_DARK_MODE: u32 = 20;
    const CAPTION_COLOR: u32 = 35;
    let [r, g, b, _] = t.base.to_array();
    let (colour, dark) = (u32::from(r) | u32::from(g) << 8 | u32::from(b) << 16, i32::from(dark));
    unsafe {
        DwmSetWindowAttribute(window, IMMERSIVE_DARK_MODE, (&raw const dark).cast(), 4);
        DwmSetWindowAttribute(window, CAPTION_COLOR, (&raw const colour).cast(), 4);
    }
}

/// The fonts and the styles of both themes. Once, at the start.
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    ctx.set_visuals_of(Theme::Dark, visuals(&DARK, true));
    ctx.set_visuals_of(Theme::Light, visuals(&LIGHT, false));
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Small, FontId::proportional(11.0)),
            (TextStyle::Body, FontId::proportional(13.5)),
            (TextStyle::Button, medium(13.0)),
            (TextStyle::Heading, semibold(22.0)),
            (TextStyle::Monospace, FontId::monospace(12.5)),
        ]
        .into();
        let spacing = &mut style.spacing;
        spacing.item_spacing = Vec2::new(S2, 6.0);
        spacing.button_padding = Vec2::new(10.0, 5.0);
        spacing.interact_size = Vec2::new(32.0, 28.0);
        spacing.menu_margin = Margin::same(6);
        spacing.window_margin = Margin::same(12);
        spacing.icon_width = 16.0;
        spacing.icon_width_inner = 10.0;
        spacing.icon_spacing = 8.0;
        spacing.scroll = ScrollStyle { bar_width: 10.0, floating_width: 4.0, handle_min_length: 32.0, ..ScrollStyle::floating() };
    });
}

/// A keyboard user's ring around the widget that has the focus.
fn focus_ring(ui: &egui::Ui, response: &egui::Response, radius: u8) {
    if response.has_focus() {
        let color = tok(ui.ctx()).accent;
        ui.painter().rect_stroke(response.rect.expand(1.0), radius, Stroke::new(1.5, color), egui::StrokeKind::Outside);
    }
}

/// A square button with an icon; `on` shows it as selected.
pub fn icon_button(ui: &mut egui::Ui, icon: char, tip: &str, on: bool) -> egui::Response {
    icon_button_tinted(ui, icon, tip, on, None)
}

/// As `icon_button`, with the icon in the colour given.
pub fn icon_button_tinted(ui: &mut egui::Ui, icon: char, tip: &str, on: bool, color: Option<Color32>) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::click());
    let t = tok(ui.ctx());
    let hover = ui.ctx().animate_bool_with_time(response.id.with("hover"), response.hovered(), quick());
    if on {
        ui.painter().rect_filled(rect, RADIUS_S, t.press);
    } else if hover > 0.0 {
        ui.painter().rect_filled(rect, RADIUS_S, t.hover.gamma_multiply(hover));
    }
    let color = color.unwrap_or(if on { t.accent } else { t.text2.lerp_to_gamma(t.text, hover) });
    let color = if ui.is_enabled() { color } else { t.muted };
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, icon.to_string(), FontId::new(17.0, family("icons")), color);
    focus_ring(ui, &response, RADIUS_S);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), tip));
    response.on_hover_text(tip)
}

/// One page of the rail: the icon and its name under it.
pub fn rail_item(ui: &mut egui::Ui, rect: egui::Rect, icon: char, label: &str, on: bool) -> egui::Response {
    let response = ui.interact(rect, ui.id().with(label), if ui.is_enabled() { Sense::click() } else { Sense::hover() });
    let t = tok(ui.ctx());
    let hover = ui.ctx().animate_bool_with_time(response.id.with("hover"), response.hovered() && ui.is_enabled(), quick());
    let shape = rect.shrink(2.0);
    if !on && hover > 0.0 {
        ui.painter().rect_filled(shape, RADIUS_M, t.control.gamma_multiply(hover));
    }
    let color = if !ui.is_enabled() { t.muted.gamma_multiply(0.6) } else if on { t.accent } else { t.text2.lerp_to_gamma(t.text, hover) };
    ui.painter().text(egui::pos2(rect.center().x, rect.top() + 21.0), egui::Align2::CENTER_CENTER, icon.to_string(), FontId::new(20.0, family("icons")), color);
    ui.painter().text(egui::pos2(rect.center().x, rect.top() + 42.0), egui::Align2::CENTER_CENTER, label, medium(10.5), color);
    focus_ring(ui, &response, RADIUS_M);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label));
    response
}

/// A row of the search box's list; `note` goes at its right end.
pub fn drop_row(ui: &mut egui::Ui, text: &str, note: &str, on: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::click());
    let t = tok(ui.ctx());
    if on {
        ui.painter().rect_filled(rect, RADIUS_S, t.press);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, RADIUS_S, t.control);
    }
    let note_x = if note.is_empty() { rect.right() } else { rect.right() - 64.0 };
    ui.painter().text(egui::pos2(rect.right() - 8.0, rect.center().y), egui::Align2::RIGHT_CENTER, note, FontId::proportional(11.5), t.muted);
    let mut job = egui::text::LayoutJob::simple(text.to_string(), FontId::proportional(13.0), if on { t.accent } else { t.text }, note_x - rect.left() - 16.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = ui.painter().layout_job(job);
    ui.painter().galley(egui::pos2(rect.left() + 8.0, rect.center().y - galley.size().y / 2.0), galley, t.text);
    response
}

/// One page in the drawer: the icon and its name on a row.
pub fn drawer_row(ui: &mut egui::Ui, icon: char, label: &str, on: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), if ui.is_enabled() { Sense::click() } else { Sense::hover() });
    let t = tok(ui.ctx());
    let hover = ui.ctx().animate_bool_with_time(response.id.with("hover"), response.hovered() && ui.is_enabled(), quick());
    if on {
        ui.painter().rect_filled(rect, RADIUS_S, t.press);
    } else if hover > 0.0 {
        ui.painter().rect_filled(rect, RADIUS_S, t.control.gamma_multiply(hover));
    }
    let color = if !ui.is_enabled() { t.muted } else if on { t.accent } else { t.text2.lerp_to_gamma(t.text, hover) };
    ui.painter().text(egui::pos2(rect.left() + 18.0, rect.center().y), egui::Align2::CENTER_CENTER, icon.to_string(), FontId::new(16.0, family("icons")), color);
    ui.painter().text(egui::pos2(rect.left() + 36.0, rect.center().y), egui::Align2::LEFT_CENTER, label, medium(13.0), color);
    focus_ring(ui, &response, RADIUS_S);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label));
    response
}

/// The app's mark, in white: painted in a colour through the tint.
pub fn logo(ctx: &egui::Context) -> egui::TextureHandle {
    let picture = image::load_from_memory(include_bytes!("../img/msg_icon_white_48.png")).expect("the icon is part of the program").to_rgba8();
    let size = [picture.width() as usize, picture.height() as usize];
    ctx.load_texture("logo", egui::ColorImage::from_rgba_unmultiplied(size, picture.as_raw()), egui::TextureOptions::LINEAR)
}

pub fn icon_font(size: f32) -> FontId {
    FontId::new(size, family("icons"))
}

pub fn glyph_text(icon: char) -> String {
    icon.to_string()
}

/// A favorite's heart is filled; the font has only the outline, the filled one comes from egui's own fonts.
pub fn heart_text(faved: bool) -> String {
    if faved { "\u{2665}".to_string() } else { icon::HEART.to_string() }
}

pub fn heart_font(faved: bool, size: f32) -> FontId {
    if faved { FontId::proportional(size * 1.2) } else { icon_font(size) }
}

/// A small mark on a picture: a dark disc that keeps the sign readable on any picture.
pub fn badge(ui: &egui::Ui, rect: egui::Rect, text: String, font: FontId, color: Color32) {
    ui.painter().rect_filled(rect, RADIUS_S, Color32::from_black_alpha(150));
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, text, font, color);
}

/// A button on a picture.
pub fn overlay_button(ui: &mut egui::Ui, rect: egui::Rect, id: egui::Id, text: String, font: FontId, color: Color32, tip: &str) -> egui::Response {
    let response = ui.interact(rect, id, Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    ui.painter().rect_filled(rect, RADIUS_S, Color32::from_black_alpha(if response.hovered() { 200 } else { 150 }));
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, text, font, color);
    response.on_hover_text(tip)
}

/// A name in the bottom corner of a picture; cut with … if the picture is narrow.
pub fn overlay_chip(ui: &mut egui::Ui, tile: egui::Rect, id: egui::Id, text: &str) -> egui::Response {
    let mut job = egui::text::LayoutJob::simple(text.to_string(), medium(11.5), Color32::WHITE, tile.width() - 32.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = ui.painter().layout_job(job);
    let size = galley.size() + Vec2::new(16.0, 8.0);
    let rect = egui::Rect::from_min_size(egui::pos2(tile.left() + 6.0, tile.bottom() - 6.0 - size.y), size);
    let response = ui.interact(rect, id, Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    ui.painter().rect_filled(rect, RADIUS_S, Color32::from_black_alpha(if response.hovered() { 210 } else { 150 }));
    ui.painter().galley(rect.min + Vec2::new(8.0, 4.0), galley, Color32::WHITE);
    response.on_hover_text("Search for this artist")
}

/// The heart button: outlined, or filled for a favorite.
pub fn heart_button(ui: &mut egui::Ui, faved: bool, tip: &str, armed: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::click());
    let t = tok(ui.ctx());
    if armed {
        ui.painter().rect_filled(rect, RADIUS_S, t.press);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, RADIUS_S, t.hover);
    }
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, heart_text(faved), heart_font(faved, 17.0), if faved { t.accent } else { t.text2 });
    focus_ring(ui, &response, RADIUS_S);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), tip));
    response.on_hover_text(tip)
}

/// A switch for on and off.
pub fn toggle(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let (rect, mut response) = ui.allocate_exact_size(Vec2::new(36.0, 20.0), Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *on, ""));
    let t = tok(ui.ctx());
    let k = ui.ctx().animate_bool_with_time(response.id, *on, quick());
    ui.painter().rect_filled(rect, 10, t.top.lerp_to_gamma(t.accent, k));
    let x = egui::lerp((rect.left() + 10.0)..=(rect.right() - 10.0), k);
    ui.painter().circle_filled(egui::pos2(x, rect.center().y), 8.0, Color32::WHITE);
    focus_ring(ui, &response, 10);
    response
}

/// Two or more choices side by side, one of them chosen. Returns the one clicked.
pub fn segmented(ui: &mut egui::Ui, options: &[(char, &str)], chosen: usize) -> Option<usize> {
    let t = tok(ui.ctx());
    let font = medium(12.5);
    let widths: Vec<f32> = options.iter().map(|(icon, text)| ui.painter().layout_no_wrap(text.to_string(), font.clone(), t.text).size().x + if *icon == NO_ICON { 24.0 } else { 44.0 }).collect();
    let (rect, area) = ui.allocate_exact_size(Vec2::new(widths.iter().sum(), 32.0), Sense::hover());
    ui.painter().rect_filled(rect, RADIUS_S, t.control);
    let mut clicked = None;
    let mut x = rect.left();
    let left_of = |n: usize| widths[..n].iter().sum::<f32>();
    // The mark slides to the chosen one.
    let mark_x = ui.ctx().animate_value_with_time(area.id.with("mark"), left_of(chosen), smooth());
    let mark_w = ui.ctx().animate_value_with_time(area.id.with("width"), widths[chosen], smooth());
    ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(rect.left() + mark_x, rect.top()), Vec2::new(mark_w, rect.height())).shrink(2.0), 5, t.press);
    for (n, (icon, text)) in options.iter().enumerate() {
        let cell = egui::Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(widths[n], rect.height()));
        x += widths[n];
        let response = ui.interact(cell, area.id.with(n), Sense::click());
        let color = if n == chosen { t.accent } else if response.hovered() { t.text } else { t.text2 };
        if *icon != NO_ICON {
            ui.painter().text(egui::pos2(cell.left() + 20.0, cell.center().y), egui::Align2::CENTER_CENTER, icon.to_string(), FontId::new(15.0, family("icons")), color);
        }
        ui.painter().text(egui::pos2(cell.left() + if *icon == NO_ICON { 12.0 } else { 32.0 }, cell.center().y), egui::Align2::LEFT_CENTER, *text, font.clone(), color);
        focus_ring(ui, &response, RADIUS_S);
        if response.clicked() {
            clicked = Some(n);
        }
    }
    clicked
}

const NO_ICON: char = '\0';

/// segmented with words only.
pub fn choice(ui: &mut egui::Ui, options: &[&str], chosen: usize) -> Option<usize> {
    let options: Vec<(char, &str)> = options.iter().map(|o| (NO_ICON, *o)).collect();
    segmented(ui, &options, chosen).filter(|n| *n != chosen)
}

/// A key on a key cap.
pub fn chip(ui: &mut egui::Ui, text: &str, on: bool) -> egui::Response {
    let t = tok(ui.ctx());
    let font = medium(12.0);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, if on { t.accent } else { t.text });
    let (rect, response) = ui.allocate_exact_size(Vec2::new((galley.size().x + 16.0).max(28.0), 26.0), Sense::click());
    ui.painter().rect_filled(rect, RADIUS_S, if on { t.press } else if response.hovered() { t.hover } else { t.control });
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, t.text);
    focus_ring(ui, &response, RADIUS_S);
    response
}

/// A group of settings on a surface of its own, under a small title (if it has one).
pub fn group<R>(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let t = tok(ui.ctx());
    ui.add_space(S3);
    if !title.is_empty() {
        ui.label(RichText::new(title).font(medium(12.0)).color(t.text2));
        ui.add_space(S1);
    }
    egui::Frame::NONE.fill(t.surface.gamma_multiply(0.85)).stroke(Stroke::new(1.0, t.hairline)).corner_radius(RADIUS_M).inner_margin(Margin::symmetric(14, 2)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 0.0;
        add(ui)
    }).inner
}

/// A row of a group: the name at the left, the control at the right, a fine line between rows.
pub fn row<R>(ui: &mut egui::Ui, label: impl Into<egui::WidgetText>, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    if ui.min_rect().height() > 1.0 {
        let color = tok(ui.ctx()).hairline;
        ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top(), Stroke::new(1.0, color));
    }
    let label = label.into();
    ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), 42.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let inner = add(ui);
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| ui.add(egui::Label::new(label).wrap()));
        inner
    })
    .inner
}

/// A small explanation under or between the groups.
pub fn note(ui: &mut egui::Ui, text: &str) {
    ui.add_space(S1);
    ui.label(RichText::new(text).size(12.0).color(tok(ui.ctx()).muted));
}