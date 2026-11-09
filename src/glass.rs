// Frosted glass: what is already drawn under a surface, blurred, with a tint over it. egui has no such thing, so
// it is a paint callback like the pictures' (media.rs) and the videos' (video.rs). Dual Kawase: the window is
// halved a few times and grown back with a small filter each step, which is cheap whatever the radius.
use super::theme::{self, tok};
use super::video;
use eframe::egui::{self, Color32, Margin, Rect, Shadow, Shape, Stroke};
use eframe::egui_glow;
use eframe::glow::{self, HasContext};
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex, OnceLock};

const LEVELS: usize = 6;

const DOWN: &str = "in vec2 uv;
uniform sampler2D tex;
uniform vec2 h;
out vec4 color;
void main() {
    vec4 s = texture(tex, uv) * 4.0;
    s += texture(tex, uv - h) + texture(tex, uv + h);
    s += texture(tex, uv + vec2(h.x, -h.y)) + texture(tex, uv - vec2(h.x, -h.y));
    color = s / 8.0;
}";

const UP: &str = "in vec2 uv;
uniform sampler2D tex;
uniform vec2 h;
out vec4 color;
void main() {
    vec4 s = texture(tex, uv + vec2(-h.x * 2.0, 0.0)) + texture(tex, uv + vec2(h.x * 2.0, 0.0));
    s += texture(tex, uv + vec2(0.0, h.y * 2.0)) + texture(tex, uv + vec2(0.0, -h.y * 2.0));
    s += (texture(tex, uv + vec2(-h.x, h.y)) + texture(tex, uv + vec2(h.x, h.y))) * 2.0;
    s += (texture(tex, uv + vec2(h.x, -h.y)) + texture(tex, uv - h)) * 2.0;
    color = s / 12.0;
}";

// The dither keeps the long, dark gradients of a blur from showing bands.
const SHOW: &str = "in vec2 uv;
uniform sampler2D tex;
uniform vec2 screen;
uniform vec4 box;
uniform float radius;
uniform vec4 tint;
uniform float opacity;
out vec4 color;
void main() {
    vec2 q = abs(uv * screen - box.xy) - box.zw + radius;
    float cover = clamp(0.5 - (length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - radius), 0.0, 1.0) * opacity;
    vec3 c = texture(tex, uv).rgb * (1.0 - tint.a) + tint.rgb;
    c += (fract(52.9829189 * fract(dot(gl_FragCoord.xy, vec2(0.06711056, 0.00583715)))) - 0.5) / 255.0;
    color = vec4(c * cover, cover);
}";

struct Programs {
    down: glow::Program,
    up: glow::Program,
    show: glow::Program,
    vao: glow::VertexArray,
}

struct Level {
    fbo: glow::Framebuffer,
    texture: glow::Texture,
    w: i32,
    h: i32,
}

static PROGRAMS: OnceLock<Option<Programs>> = OnceLock::new();
static CHAIN: Mutex<Vec<Level>> = Mutex::new(Vec::new());

/// How a surface looks: its corners, what lies over the blur, and how many halvings the blur goes through.
#[derive(Clone, Copy)]
pub struct Look {
    pub radius: f32,
    pub tint: Color32,
    pub passes: usize,
}

/// The toolbars, the rail and the drawer.
pub fn bar(ctx: &egui::Context) -> Look {
    Look { radius: theme::RADIUS_L as f32, tint: tok(ctx).glass, passes: 4 }
}

unsafe fn compile(gl: &glow::Context) -> Option<Programs> {
    unsafe {
        Some(Programs { down: video::quad_program(gl, DOWN)?, up: video::quad_program(gl, UP)?, show: video::quad_program(gl, SHOW)?, vao: gl.create_vertex_array().ok()? })
    }
}

unsafe fn fit_chain(gl: &glow::Context, chain: &mut Vec<Level>, w: i32, h: i32) {
    if chain.first().is_some_and(|l| (l.w, l.h) == ((w / 2).max(1), (h / 2).max(1))) {
        return;
    }
    unsafe {
        for level in chain.drain(..) {
            gl.delete_framebuffer(level.fbo);
            gl.delete_texture(level.texture);
        }
        for i in 1..=LEVELS as i32 {
            let (lw, lh) = ((w >> i).max(1), (h >> i).max(1));
            let Ok(texture) = gl.create_texture() else { return };
            let Ok(fbo) = gl.create_framebuffer() else { return };
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::RGBA8 as i32, lw, lh, 0, glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelUnpackData::Slice(None));
            for (key, value) in [(glow::TEXTURE_MIN_FILTER, glow::LINEAR), (glow::TEXTURE_MAG_FILTER, glow::LINEAR), (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE), (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE)] {
                gl.tex_parameter_i32(glow::TEXTURE_2D, key, value as i32);
            }
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
            gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(texture), 0);
            chain.push(Level { fbo, texture, w: lw, h: lh });
        }
    }
}

unsafe fn pass(gl: &glow::Context, program: glow::Program, from: &Level, to: &Level) {
    unsafe {
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(to.fbo));
        gl.viewport(0, 0, to.w, to.h);
        gl.use_program(Some(program));
        gl.bind_texture(glow::TEXTURE_2D, Some(from.texture));
        gl.uniform_2_f32(gl.get_uniform_location(program, "h").as_ref(), 0.5 / to.w as f32, 0.5 / to.h as f32);
        gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
    }
}

/// The shape that blurs what is under `rect`.
// ponytail: the whole window is blurred for each surface; it is a few small passes, a scissor per surface if it ever shows.
pub fn frost(ctx: &egui::Context, rect: Rect, look: Look, opacity: f32) -> Shape {
    if matches!(PROGRAMS.get(), Some(None)) {
        return Shape::rect_filled(rect, look.radius, tok(ctx).raised.gamma_multiply(opacity));
    }
    let ctx = ctx.clone();
    let callback = egui_glow::CallbackFn::new(move |info, painter| {
        let gl = painter.gl();
        let Some(programs) = PROGRAMS.get_or_init(|| unsafe { compile(gl) }) else {
            return ctx.request_repaint();
        };
        let [sw, sh] = info.screen_size_px.map(|v| v as i32);
        let ppp = info.pixels_per_point;
        let mut chain = CHAIN.lock().unwrap();
        unsafe {
            let window = u32::try_from(gl.get_parameter_i32(glow::FRAMEBUFFER_BINDING)).ok().and_then(NonZeroU32::new).map(glow::NativeFramebuffer);
            fit_chain(gl, &mut chain, sw, sh);
            if chain.len() < LEVELS {
                gl.bind_framebuffer(glow::FRAMEBUFFER, window);
                return;
            }
            let passes = look.passes.clamp(1, LEVELS);
            gl.disable(glow::SCISSOR_TEST);
            gl.disable(glow::BLEND);
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, window);
            gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(chain[0].fbo));
            gl.blit_framebuffer(0, 0, sw, sh, 0, 0, chain[0].w, chain[0].h, glow::COLOR_BUFFER_BIT, glow::LINEAR);
            gl.bind_vertex_array(Some(programs.vao));
            gl.active_texture(glow::TEXTURE0);
            for i in 1..passes {
                pass(gl, programs.down, &chain[i - 1], &chain[i]);
            }
            for i in (1..passes).rev() {
                pass(gl, programs.up, &chain[i], &chain[i - 1]);
            }
            gl.bind_framebuffer(glow::FRAMEBUFFER, window);
            gl.viewport(0, 0, sw, sh);
            let area = rect.intersect(info.clip_rect);
            if area.is_positive() {
                let (left, top) = ((area.left() * ppp).floor() as i32, (area.top() * ppp).floor() as i32);
                let (right, bottom) = ((area.right() * ppp).ceil() as i32, (area.bottom() * ppp).ceil() as i32);
                gl.enable(glow::SCISSOR_TEST);
                gl.scissor(left, sh - bottom, right - left, bottom - top);
                gl.enable(glow::BLEND);
                let p = programs.show;
                gl.use_program(Some(p));
                gl.bind_texture(glow::TEXTURE_2D, Some(chain[0].texture));
                let center = rect.center();
                gl.uniform_2_f32(gl.get_uniform_location(p, "screen").as_ref(), sw as f32, sh as f32);
                gl.uniform_4_f32(gl.get_uniform_location(p, "box").as_ref(), center.x * ppp, sh as f32 - center.y * ppp, rect.width() * ppp * 0.5, rect.height() * ppp * 0.5);
                gl.uniform_1_f32(gl.get_uniform_location(p, "radius").as_ref(), look.radius * ppp);
                let [r, g, b, a] = look.tint.to_array().map(|v| v as f32 / 255.0);
                gl.uniform_4_f32(gl.get_uniform_location(p, "tint").as_ref(), r, g, b, a);
                gl.uniform_1_f32(gl.get_uniform_location(p, "opacity").as_ref(), opacity);
                gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
            }
        }
    });
    Shape::Callback(egui::PaintCallback { rect, callback: Arc::new(callback) })
}

fn surface(ui: &egui::Ui, rect: Rect, look: Look) -> [Shape; 3] {
    let t = tok(ui.ctx());
    let shadow = Shadow { offset: [0, 8], blur: 28, spread: 0, color: Color32::from_black_alpha(if ui.visuals().dark_mode { 90 } else { 34 }) };
    [
        shadow.as_shape(rect, look.radius).into(),
        frost(ui.ctx(), rect, look, ui.opacity()),
        Shape::rect_stroke(rect, look.radius, Stroke::new(1.0, t.hairline), egui::StrokeKind::Inside),
    ]
}

/// A glass surface at a place that is known beforehand.
pub fn panel(ui: &egui::Ui, rect: Rect, look: Look) {
    ui.painter().extend(surface(ui, rect, look));
}

/// A glass surface as big as what `add` draws on it.
pub fn card<R>(ui: &mut egui::Ui, look: Look, margin: Margin, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let slots = [ui.painter().add(Shape::Noop), ui.painter().add(Shape::Noop), ui.painter().add(Shape::Noop)];
    let inner = egui::Frame::NONE.inner_margin(margin).show(ui, add);
    for (slot, shape) in slots.into_iter().zip(surface(ui, inner.response.rect, look)) {
        ui.painter().set(slot, shape);
    }
    inner.inner
}
