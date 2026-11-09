// The look of the window: the colours, sizes, fonts and icons, and the small widgets every page shares.
use eframe::egui::epaint::text::VariationCoords;
use eframe::egui::style::{ScrollStyle, TextCursorStyle, WidgetVisuals};
use eframe::egui::{self, Color32, CornerRadius, CursorIcon, FontData, FontDefinitions, FontFamily, FontId, FontTweak, Margin, RichText, Sense, Shadow, Stroke, TextStyle, Theme, Vec2, Visuals};
use std::sync::atomic::{AtomicBool, Ordering};
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
    pub const CLOSE: char = '\u{e1b2}';
    pub const RANDOM: char = '\u{e15e}';
    pub const VOLUME: char = '\u{e1ab}';
    pub const MUTED: char = '\u{e1ac}';
    pub const LOADING: char = '\u{e10a}';
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

static STILL: AtomicBool = AtomicBool::new(false);

/// How long a small change takes (seconds), and a larger one. Zero when Windows is set to show no animations.
pub fn quick() -> f32 {
    if STILL.load(Ordering::Relaxed) { 0.0 } else { 0.15 }
}

pub fn smooth() -> f32 {
    if STILL.load(Ordering::Relaxed) { 0.0 } else { 0.22 }
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
    fonts.font_data.insert("lucide".to_string(), Arc::new(FontData::from_static(include_bytes!("../img/fonts/lucide.ttf"))));
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
        style.animation_time = quick();
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
    let widths: Vec<f32> = options.iter().map(|(_, text)| ui.painter().layout_no_wrap(text.to_string(), font.clone(), t.text).size().x + 44.0).collect();
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
        ui.painter().text(egui::pos2(cell.left() + 20.0, cell.center().y), egui::Align2::CENTER_CENTER, icon.to_string(), FontId::new(15.0, family("icons")), color);
        ui.painter().text(egui::pos2(cell.left() + 32.0, cell.center().y), egui::Align2::LEFT_CENTER, *text, font.clone(), color);
        focus_ring(ui, &response, RADIUS_S);
        if response.clicked() {
            clicked = Some(n);
        }
    }
    clicked
}
