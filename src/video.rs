// Video, through libmpv. The library is loaded when the window starts (mpv-2.dll next to the program,
// or libmpv from the system), so the program itself starts without it; without the library a video
// slide says so. mpv draws each frame into a texture (its OpenGL render API), and that texture is drawn
// into the window where the picture would be. Only the calls that are needed are declared here.
use eframe::egui;
use eframe::glow::{self, HasContext};
use libloading::Library;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::{Arc, Mutex};

type Handle = *mut c_void;

#[repr(C)]
struct RenderParam {
    kind: c_int,
    data: *mut c_void,
}

#[repr(C)]
struct OpenglInit {
    get_proc_address: Option<unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void>,
    ctx: *mut c_void,
}

#[repr(C)]
struct OpenglFbo {
    fbo: c_int,
    w: c_int,
    h: c_int,
    internal_format: c_int,
}

const PARAM_INVALID: c_int = 0;
const PARAM_API_TYPE: c_int = 1;
const PARAM_INIT: c_int = 2;
const PARAM_FBO: c_int = 3;
const PARAM_FLIP_Y: c_int = 4;

struct Api {
    _lib: Library,
    create: unsafe extern "C" fn() -> Handle,
    initialize: unsafe extern "C" fn(Handle) -> c_int,
    terminate_destroy: unsafe extern "C" fn(Handle),
    set_option_string: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int,
    set_property_string: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int,
    get_property_string: unsafe extern "C" fn(Handle, *const c_char) -> *mut c_char,
    free: unsafe extern "C" fn(*mut c_void),
    command: unsafe extern "C" fn(Handle, *mut *const c_char) -> c_int,
    render_create: unsafe extern "C" fn(*mut Handle, Handle, *mut RenderParam) -> c_int,
    render: unsafe extern "C" fn(Handle, *mut RenderParam) -> c_int,
    render_free: unsafe extern "C" fn(Handle),
    render_set_update_callback: unsafe extern "C" fn(Handle, Option<unsafe extern "C" fn(*mut c_void)>, *mut c_void),
}

unsafe impl Send for Api {}
unsafe impl Sync for Api {}

fn library_names() -> &'static [&'static str] {
    if cfg!(windows) {
        &["mpv-2.dll", "libmpv-2.dll", "mpv-1.dll"]
    } else if cfg!(target_os = "macos") {
        &["libmpv.2.dylib", "libmpv.dylib", "/opt/homebrew/lib/libmpv.dylib", "/usr/local/lib/libmpv.dylib"]
    } else {
        &["libmpv.so.2", "libmpv.so.1", "libmpv.so"]
    }
}

impl Api {
    fn load() -> Result<Api, String> {
        // Next to the program first (that is where a packaged mpv-2.dll is).
        let mut candidates: Vec<String> = vec![];
        if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())) {
            candidates.extend(library_names().iter().map(|n| dir.join(n).to_string_lossy().into_owned()));
        }
        candidates.extend(library_names().iter().map(|n| n.to_string()));
        let lib = candidates.iter().find_map(|name| unsafe { Library::new(name).ok() }).ok_or_else(|| {
            format!("libmpv was not found (looked for {}). Videos need it; put it next to the program or install mpv.", library_names().join(", "))
        })?;
        unsafe {
            macro_rules! sym {
                ($name:literal) => {
                    *lib.get(concat!($name, "\0").as_bytes()).map_err(|e| format!("libmpv is missing {}: {e}", $name))?
                };
            }
            Ok(Api {
                create: sym!("mpv_create"),
                initialize: sym!("mpv_initialize"),
                terminate_destroy: sym!("mpv_terminate_destroy"),
                set_option_string: sym!("mpv_set_option_string"),
                set_property_string: sym!("mpv_set_property_string"),
                get_property_string: sym!("mpv_get_property_string"),
                free: sym!("mpv_free"),
                command: sym!("mpv_command"),
                render_create: sym!("mpv_render_context_create"),
                render: sym!("mpv_render_context_render"),
                render_free: sym!("mpv_render_context_free"),
                render_set_update_callback: sym!("mpv_render_context_set_update_callback"),
                _lib: lib,
            })
        }
    }
}

// The texture mpv draws into and what it takes to put it on the screen.
#[derive(Default)]
struct Target {
    fbo: Option<glow::Framebuffer>,
    texture: Option<glow::Texture>,
    size: (i32, i32),
    program: Option<glow::Program>,
    vao: Option<glow::VertexArray>,
}

#[derive(Clone, Copy)]
struct Raw(*mut c_void);
unsafe impl Send for Raw {}
unsafe impl Sync for Raw {}
impl Raw {
    // A method, so a closure captures the whole wrapper (which may cross threads), not the bare pointer.
    fn ptr(self) -> *mut c_void {
        self.0
    }
}

pub struct Player {
    api: Arc<Api>,
    handle: Raw,
    render: Raw,
    target: Arc<Mutex<Target>>,
    /// The address being played (empty: nothing).
    pub url: String,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Status {
    pub position: f64,
    pub duration: f64,
    pub paused: bool,
    pub ended: bool,
}

unsafe extern "C" fn get_proc(ctx: *mut c_void, name: *const c_char) -> *mut c_void {
    let lookup = unsafe { &*(ctx as *const &dyn Fn(&CStr) -> *const c_void) };
    lookup(unsafe { CStr::from_ptr(name) }) as *mut c_void
}

unsafe extern "C" fn on_update(ctx: *mut c_void) {
    unsafe { &*(ctx as *const egui::Context) }.request_repaint();
}

impl Player {
    pub fn new(get_proc_address: &dyn Fn(&CStr) -> *const c_void, ctx: egui::Context) -> Result<Player, String> {
        let api = Arc::new(Api::load()?);
        unsafe {
            #[cfg(unix)]
            {
                // mpv refuses to start unless numbers are read the C way.
                extern "C" {
                    fn setlocale(category: c_int, locale: *const c_char) -> *mut c_char;
                }
                setlocale(if cfg!(target_os = "macos") { 4 } else { 1 }, c"C".as_ptr());
            }
            let handle = (api.create)();
            if handle.is_null() {
                return Err("mpv could not be created".into());
            }
            for (name, value) in [
                ("vo", "libmpv"), ("hwdec", "no"),("keep-open", "yes"), ("idle", "yes"), ("terminal", "no"),
                ("input-default-bindings", "no"), ("input-vo-keyboard", "no"), ("osc", "no"), ("ytdl", "no"),
                ("audio-display", "no"), ("user-agent", concat!("MSG/", env!("CARGO_PKG_VERSION"), " (booru slideshow)")),
            ] {
                let (n, v) = (CString::new(name).unwrap(), CString::new(value).unwrap());
                (api.set_option_string)(handle, n.as_ptr(), v.as_ptr());
            }
            if (api.initialize)(handle) < 0 {
                (api.terminate_destroy)(handle);
                return Err("mpv could not start".into());
            }
            let lookup: &dyn Fn(&CStr) -> *const c_void = get_proc_address;
            let mut init = OpenglInit { get_proc_address: Some(get_proc), ctx: &lookup as *const _ as *mut c_void };
            let mut api_type = *b"opengl\0";
            let mut params = [
                RenderParam { kind: PARAM_API_TYPE, data: api_type.as_mut_ptr() as *mut c_void },
                RenderParam { kind: PARAM_INIT, data: &mut init as *mut _ as *mut c_void },
                RenderParam { kind: PARAM_INVALID, data: std::ptr::null_mut() },
            ];
            let mut render: Handle = std::ptr::null_mut();
            if (api.render_create)(&mut render, handle, params.as_mut_ptr()) < 0 {
                (api.terminate_destroy)(handle);
                return Err("mpv could not draw into the window".into());
            }
            // A new frame is ready: ask for a repaint (the box lives as long as the program).
            (api.render_set_update_callback)(render, Some(on_update), Box::into_raw(Box::new(ctx)) as *mut c_void);
            Ok(Player { api, handle: Raw(handle), render: Raw(render), target: Arc::default(), url: String::new() })
        }
    }

    fn set(&self, name: &str, value: &str) {
        let (n, v) = (CString::new(name).unwrap(), CString::new(value).unwrap());
        unsafe { (self.api.set_property_string)(self.handle.0, n.as_ptr(), v.as_ptr()) };
    }

    fn get(&self, name: &str) -> Option<String> {
        let n = CString::new(name).unwrap();
        unsafe {
            let raw = (self.api.get_property_string)(self.handle.0, n.as_ptr());
            if raw.is_null() {
                return None;
            }
            let text = CStr::from_ptr(raw).to_string_lossy().into_owned();
            (self.api.free)(raw as *mut c_void);
            Some(text)
        }
    }

    fn command(&self, args: &[&str]) {
        let owned: Vec<CString> = args.iter().map(|a| CString::new(*a).unwrap()).collect();
        let mut pointers: Vec<*const c_char> = owned.iter().map(|a| a.as_ptr()).collect();
        pointers.push(std::ptr::null());
        unsafe { (self.api.command)(self.handle.0, pointers.as_mut_ptr()) };
    }

    /// Takes effect with the next file. The graphics card's decoder is lighter on the processor but some
    /// videos come out with streaks and dots in it, so the default is the processor (see the `videoHwdec` setting).
    pub fn set_hwdec(&self, hardware: bool) {
        self.set("hwdec", if hardware { "auto-copy-safe" } else { "no" });
    }

    /// Starts `url` (an address or a file). `looped`: play again at the end; `play`: start at once.
    pub fn load(&mut self, url: &str, referer: Option<&str>, looped: bool, play: bool) {
        self.url = url.to_string();
        self.set("referrer", referer.unwrap_or(""));
        self.set("loop-file", if looped { "inf" } else { "no" });
        self.set("pause", if play { "no" } else { "yes" });
        self.command(&["loadfile", url, "replace"]);
    }

    pub fn stop(&mut self) {
        if !self.url.is_empty() {
            self.url.clear();
            self.command(&["stop"]);
        }
    }

    pub fn status(&self) -> Status {
        let number = |name: &str| self.get(name).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
        Status {
            position: number("time-pos"),
            duration: number("duration"),
            paused: self.get("pause").as_deref() == Some("yes"),
            ended: self.get("eof-reached").as_deref() == Some("yes"),
        }
    }

    pub fn set_paused(&self, paused: bool) {
        self.set("pause", if paused { "yes" } else { "no" });
    }

    pub fn seek(&self, seconds: f64) {
        self.command(&["seek", &format!("{seconds:.2}"), "absolute"]);
    }

    /// 0.0–1.0, like the web version's saved videoVolume.
    pub fn set_volume(&self, volume: f64, muted: bool) {
        self.set("volume", &format!("{:.0}", volume.clamp(0.0, 1.0) * 100.0));
        self.set("mute", if muted { "yes" } else { "no" });
    }

    /// Puts the video in `rect` (points) with a paint callback.
    pub fn paint(&self, ui: &egui::Ui, rect: egui::Rect) {
        let (api, render, target) = (self.api.clone(), self.render, self.target.clone());
        // mpv keeps the last frame of the previous video until the new one has its first: that would show for a
        // moment, so until the new file has video parameters (they go with the first frame) the box stays black.
        let has_frame = !self.url.is_empty() && self.get("video-params/w").is_some();
        let callback = eframe::egui_glow::CallbackFn::new(move |info, painter| {
            let gl = painter.gl();
            let vp = info.viewport_in_pixels();
            let (w, h) = (vp.width_px.max(1), vp.height_px.max(1));
            let mut t = target.lock().unwrap();
            unsafe {
                if t.program.is_none() && !init_blit(gl, &mut t) {
                    return;
                }
                let previous = gl.get_parameter_i32(glow::FRAMEBUFFER_BINDING);
                if t.size != (w, h) {
                    resize_target(gl, &mut t, w, h);
                }
                let Some(fbo) = t.fbo else { return };
                gl.disable(glow::SCISSOR_TEST);
                let mut fbo_param = OpenglFbo { fbo: fbo.0.get() as c_int, w, h, internal_format: 0 };
                let mut flip: c_int = 1;
                let mut params = [
                    RenderParam { kind: PARAM_FBO, data: &mut fbo_param as *mut _ as *mut c_void },
                    RenderParam { kind: PARAM_FLIP_Y, data: &mut flip as *mut _ as *mut c_void },
                    RenderParam { kind: PARAM_INVALID, data: std::ptr::null_mut() },
                ];
                (api.render)(render.ptr(), params.as_mut_ptr());
                // Onto the window, into the rectangle egui set the viewport to.
                gl.bind_framebuffer(glow::FRAMEBUFFER, u32::try_from(previous).ok().and_then(std::num::NonZeroU32::new).map(glow::NativeFramebuffer));
                gl.viewport(vp.left_px, vp.from_bottom_px, vp.width_px, vp.height_px);
                gl.enable(glow::SCISSOR_TEST);
                gl.disable(glow::BLEND);
                if has_frame {
                    gl.use_program(t.program);
                    gl.bind_vertex_array(t.vao);
                    gl.active_texture(glow::TEXTURE0);
                    gl.bind_texture(glow::TEXTURE_2D, t.texture);
                    gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
                    gl.bind_texture(glow::TEXTURE_2D, None);
                } else {
                    // Only the video's box is cleared (mpv leaves the scissor box at its own size).
                    let clip = info.clip_rect_in_pixels();
                    gl.scissor(clip.left_px, clip.from_bottom_px, clip.width_px, clip.height_px);
                    gl.clear_color(0.0, 0.0, 0.0, 1.0);
                    gl.clear(glow::COLOR_BUFFER_BIT);
                }
            }
        });
        ui.painter().add(egui::PaintCallback { rect, callback: Arc::new(callback) });
    }

    /// Frees mpv; call with the window's GL context still alive.
    pub fn shutdown(&mut self, gl: Option<&glow::Context>) {
        unsafe {
            (self.api.render_free)(self.render.0);
            (self.api.terminate_destroy)(self.handle.0);
            if let Some(gl) = gl {
                let t = self.target.lock().unwrap();
                if let Some(f) = t.fbo {
                    gl.delete_framebuffer(f);
                }
                if let Some(x) = t.texture {
                    gl.delete_texture(x);
                }
            }
        }
        self.render = Raw(std::ptr::null_mut());
        self.handle = Raw(std::ptr::null_mut());
    }
}

unsafe fn init_blit(gl: &glow::Context, t: &mut Target) -> bool {
    unsafe {
        let header = if gl.version().is_embedded { "#version 300 es\nprecision mediump float;\n" } else { "#version 330 core\n" };
        let vertex = format!("{header}out vec2 uv;\nvoid main() {{ vec2 p = vec2(float(gl_VertexID & 1), float((gl_VertexID >> 1) & 1)); uv = p; gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0); }}");
        let fragment = format!("{header}in vec2 uv;\nuniform sampler2D tex;\nout vec4 color;\nvoid main() {{ color = vec4(texture(tex, uv).rgb, 1.0); }}");
        let Ok(program) = gl.create_program() else { return false };
        for (kind, source) in [(glow::VERTEX_SHADER, vertex), (glow::FRAGMENT_SHADER, fragment)] {
            let Ok(shader) = gl.create_shader(kind) else { return false };
            gl.shader_source(shader, &source);
            gl.compile_shader(shader);
            if !gl.get_shader_compile_status(shader) {
                eprintln!("video shader: {}", gl.get_shader_info_log(shader));
                return false;
            }
            gl.attach_shader(program, shader);
        }
        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            eprintln!("video shader link: {}", gl.get_program_info_log(program));
            return false;
        }
        t.program = Some(program);
        t.vao = gl.create_vertex_array().ok();
        true
    }
}

unsafe fn resize_target(gl: &glow::Context, t: &mut Target, w: i32, h: i32) {
    unsafe {
        if let Some(f) = t.fbo.take() {
            gl.delete_framebuffer(f);
        }
        if let Some(x) = t.texture.take() {
            gl.delete_texture(x);
        }
        let texture = gl.create_texture().ok();
        gl.bind_texture(glow::TEXTURE_2D, texture);
        gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::RGBA8 as i32, w, h, 0, glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelUnpackData::Slice(None));
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
        let fbo = gl.create_framebuffer().ok();
        gl.bind_framebuffer(glow::FRAMEBUFFER, fbo);
        gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, texture, 0);
        t.fbo = fbo;
        t.texture = texture;
        t.size = (w, h);
    }
}
