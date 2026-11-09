// Pictures for the window: fetched (or read from the disk), decoded off the UI thread and turned into
// textures when they arrive. A GIF keeps all its frames; the window picks the frame by time.
use super::net;
use super::session::Engine;
use super::video;
use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use eframe::glow::{self, HasContext};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

// Bigger pictures are shrunk: a GPU texture has a size limit (and 8K RGBA is 130 MB).
const MAX_SIDE: u32 = 4096;
const MAX_GIF_FRAMES: usize = 400;
const KEEP: usize = 90;
// At most this many downloads at once, so a row of thumbnails doesn't hammer a site's image server;
// the rest wait their turn in the order they were asked for (the picture on the screen comes first).
const AT_ONCE: usize = 6;
// A bigger video isn't fetched ahead: mpv reads it from the site as it plays.
const MAX_VIDEO: u64 = 200 << 20;

pub struct Frame {
    pub texture: TextureHandle,
    pub delay: Duration,
}

pub struct Picture {
    pub frames: Vec<Frame>,
    pub size: [usize; 2],
    total: Duration,
}

impl Picture {
    /// The frame to show `elapsed` after the picture came up (a still has one).
    pub fn frame_at(&self, elapsed: Duration) -> &Frame {
        if self.frames.len() < 2 || self.total.is_zero() {
            return &self.frames[0];
        }
        let mut t = Duration::from_nanos((elapsed.as_nanos() % self.total.as_nanos()) as u64);
        for frame in &self.frames {
            if t < frame.delay {
                return frame;
            }
            t -= frame.delay;
        }
        &self.frames[0]
    }

    pub fn next_frame_in(&self, elapsed: Duration) -> Option<Duration> {
        if self.frames.len() < 2 || self.total.is_zero() {
            return None;
        }
        let mut t = Duration::from_nanos((elapsed.as_nanos() % self.total.as_nanos()) as u64);
        for frame in &self.frames {
            if t < frame.delay {
                return Some(frame.delay - t);
            }
            t -= frame.delay;
        }
        None
    }
}

pub enum State {
    Loading,
    Ready(Picture),
    Failed(String),
    File(TempVideo),
}

/// A video fetched ahead, in the temporary folder. It goes when the cache lets go of it.
// ponytail: up to KEEP files stay until then; a limit in bytes if the temporary folder ever fills up.
pub struct TempVideo(PathBuf);

impl TempVideo {
    /// None if it is gone (a second window clears the folder when it starts).
    pub fn path(&self) -> Option<String> {
        self.0.exists().then(|| self.0.to_string_lossy().into_owned())
    }
}

impl Drop for TempVideo {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn video_dir() -> PathBuf {
    std::env::temp_dir().join("MSG-video")
}

/// What a crash left behind goes when the program starts.
pub fn clear_video_files() {
    let _ = std::fs::remove_dir_all(video_dir());
    let _ = std::fs::create_dir_all(video_dir());
}

enum Decoded {
    Frames(Vec<(ColorImage, Duration)>),
    Video(TempVideo),
}

pub struct MediaCache {
    entries: HashMap<String, (State, u64)>,
    // The downloads under way, to be called off when nobody asks for them any more.
    loading: HashMap<String, tokio::task::AbortHandle>,
    tx: Sender<(String, Result<Decoded, String>)>,
    rx: Receiver<(String, Result<Decoded, String>)>,
    tick: u64,
    // The tick at the last poll (the start of the last frame).
    frame: u64,
    limit: Arc<tokio::sync::Semaphore>,
}

impl MediaCache {
    pub fn new() -> MediaCache {
        let (tx, rx) = channel();
        MediaCache { entries: HashMap::new(), loading: HashMap::new(), tx, rx, tick: 0, frame: 0, limit: Arc::new(tokio::sync::Semaphore::new(AT_ONCE)) }
    }

    pub fn get(&mut self, url: &str) -> Option<&State> {
        self.tick += 1;
        let tick = self.tick;
        self.entries.get_mut(url).map(|(state, used)| {
            *used = tick;
            &*state
        })
    }

    /// Starts loading `url` (an address or a file path) unless it is loading or loaded.
    pub fn request(&mut self, engine: &Engine, url: &str) {
        self.start(engine, url, false);
    }

    /// Starts fetching a video that is coming up into a temporary file (`State::File`), so it starts at once.
    pub fn request_video(&mut self, engine: &Engine, url: &str) {
        self.start(engine, url, true);
    }

    fn start(&mut self, engine: &Engine, url: &str, video: bool) {
        if url.is_empty() {
            return;
        }
        self.tick += 1;
        if let Some((_, used)) = self.entries.get_mut(url) {
            *used = self.tick;
            return;
        }
        self.entries.insert(url.to_string(), (State::Loading, self.tick));
        let key = url.to_string();
        let (tx, engine, url, limit) = (self.tx.clone(), engine.clone(), url.to_string(), self.limit.clone());
        let task = engine.rt.clone().spawn(async move {
            let result = if video {
                let _turn = limit.acquire().await;
                fetch_video(&engine, &url).await
            } else {
                let bytes = if net::web_url(&url).is_some() {
                    let _turn = limit.acquire().await;
                    net::get_bytes(&engine.client, &engine.store, &url).await
                } else {
                    tokio::fs::read(&url).await.map_err(|_| "file not found".to_string())
                };
                match bytes {
                    Ok(bytes) => tokio::task::spawn_blocking(move || decode(&bytes)).await.unwrap_or_else(|_| Err("decoding failed".into())),
                    Err(e) => Err(e),
                }
            };
            let _ = tx.send((url, result));
            engine.wake();
        });
        self.loading.insert(key, task.abort_handle());
    }

    /// Moves what has arrived into textures, and lets the oldest go when there are too many. What the last
    /// frame drew stays, however many (a screen full of pool covers), or it would be loaded again and again.
    pub fn poll(&mut self, ctx: &egui::Context) {
        while let Ok((url, result)) = self.rx.try_recv() {
            self.loading.remove(&url);
            let state = match result {
                Ok(Decoded::Video(file)) => State::File(file),
                Ok(Decoded::Frames(decoded)) => {
                    let size = decoded[0].0.size;
                    let frames: Vec<Frame> = decoded
                        .into_iter()
                        .enumerate()
                        .map(|(i, (image, delay))| Frame { texture: ctx.load_texture(format!("{url}#{i}"), image, TextureOptions::LINEAR), delay })
                        .collect();
                    let total = frames.iter().map(|f| f.delay).sum();
                    State::Ready(Picture { frames, size, total })
                }
                Err(e) => State::Failed(e),
            };
            if let Some(entry) = self.entries.get_mut(&url) {
                entry.0 = state;
            }
        }
        // Every picture still needed is asked for on every frame. The rest (slides that went by, an old search)
        // would only hold up the one on the screen, so they are called off, in the queue or halfway down.
        let (entries, frame) = (&mut self.entries, self.frame);
        self.loading.retain(|url, task| {
            let wanted = entries.get(url).is_some_and(|(_, used)| *used > frame);
            if !wanted {
                task.abort();
                entries.remove(url);
            }
            wanted
        });
        if self.entries.len() > KEEP {
            let mut by_use: Vec<_> = self.entries.iter().filter(|(_, (_, used))| *used <= self.frame).map(|(k, (_, used))| (*used, k.clone())).collect();
            by_use.sort();
            for (_, key) in by_use.into_iter().take(self.entries.len() - KEEP) {
                self.entries.remove(&key);
            }
        }
        self.frame = self.tick;
    }
}

// A shrunk picture: every texel under a window pixel, averaged. The GPU's own filter reads only the 4 nearest,
// so at under half the size it skips most of a thin line and the line comes out stair-stepped. (Mipmaps
// fix that too, but blur the picture.)
const SHRINK_SHADER: &str = "in vec2 uv;
uniform sampler2D tex;
uniform vec2 pixel;
uniform int taps;
uniform float radius;
uniform float alpha;
out vec4 color;
void main() {
    vec2 corner = vec2(uv.x, 1.0 - uv.y) - pixel * 0.5;
    vec4 sum = vec4(0.0);
    for (int y = 0; y < taps; y++)
        for (int x = 0; x < taps; x++)
            sum += texture(tex, corner + pixel * (vec2(x, y) + 0.5) / float(taps));
    vec2 size = 1.0 / pixel;
    vec2 q = abs(uv - 0.5) * size - size * 0.5 + radius;
    float cover = clamp(0.5 - (length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - radius), 0.0, 1.0);
    color = sum / float(taps * taps) * alpha * cover;
}";
static SHRINK: OnceLock<Option<(glow::Program, glow::VertexArray)>> = OnceLock::new();

/// Draws a picture into `rect`, as see-through as `alpha` and with corners of `radius` points.
pub fn paint(ui: &egui::Ui, rect: egui::Rect, texture: &TextureHandle, size: [usize; 2], alpha: f32, radius: f32) {
    let texels_per_pixel = size[0] as f32 / (rect.width() * ui.ctx().pixels_per_point());
    // Not shrunk, or the shader didn't compile: egui draws it.
    if texels_per_pixel <= 1.0 || matches!(SHRINK.get(), Some(None)) {
        let whole = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        ui.painter().add(egui::epaint::RectShape::filled(rect, radius, egui::Color32::WHITE.gamma_multiply(alpha)).with_texture(texture.id(), whole));
        return;
    }
    let (id, ctx) = (texture.id(), ui.ctx().clone());
    let callback = eframe::egui_glow::CallbackFn::new(move |info, painter| {
        let gl = painter.gl();
        let Some((program, vao)) = *SHRINK.get_or_init(|| unsafe { Some((video::quad_program(gl, SHRINK_SHADER)?, gl.create_vertex_array().ok()?)) }) else {
            return ctx.request_repaint();
        };
        let Some(texture) = painter.texture(id) else { return };
        // egui fits the viewport inside the window, which squeezes a picture that is partly outside it (the pools
        // page's bottom row): the whole rectangle is used, and the clip rectangle (scissor) cuts what is outside.
        let (r, ppp) = (info.viewport, info.pixels_per_point);
        let (w, h) = ((r.width() * ppp).round().max(1.0), (r.height() * ppp).round().max(1.0));
        let taps = (size[0] as f32 / w).max(size[1] as f32 / h).ceil().clamp(1.0, 16.0) as i32;
        unsafe {
            gl.viewport((r.left() * ppp).round() as i32, (info.screen_size_px[1] as f32 - r.bottom() * ppp).round() as i32, w as i32, h as i32);
            gl.use_program(Some(program));
            gl.bind_vertex_array(Some(vao));
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.uniform_2_f32(gl.get_uniform_location(program, "pixel").as_ref(), 1.0 / w, 1.0 / h);
            gl.uniform_1_i32(gl.get_uniform_location(program, "taps").as_ref(), taps);
            gl.uniform_1_f32(gl.get_uniform_location(program, "radius").as_ref(), radius * ppp);
            gl.uniform_1_f32(gl.get_uniform_location(program, "alpha").as_ref(), alpha);
            gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
        }
    });
    ui.painter().add(egui::PaintCallback { rect, callback: Arc::new(callback) });
}

fn decode(bytes: &[u8]) -> Result<Decoded, String> {
    use image::AnimationDecoder;
    let format = image::guess_format(bytes).map_err(|_| "not a picture".to_string())?;
    if format == image::ImageFormat::Gif {
        let decoder = image::codecs::gif::GifDecoder::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
        let mut frames = vec![];
        for frame in decoder.into_frames().take(MAX_GIF_FRAMES) {
            let frame = frame.map_err(|e| e.to_string())?;
            let (n, d) = frame.delay().numer_denom_ms();
            // Browsers treat 0 and 10 ms as 100 ms.
            let ms = if d == 0 || n / d <= 10 { 100 } else { n / d };
            frames.push((to_color(shrink(image::DynamicImage::ImageRgba8(frame.into_buffer()))), Duration::from_millis(ms as u64)));
        }
        if !frames.is_empty() {
            return Ok(Decoded::Frames(frames));
        }
    }
    let image = image::load_from_memory_with_format(bytes, format).map_err(|e| e.to_string())?;
    Ok(Decoded::Frames(vec![(to_color(shrink(image)), Duration::ZERO)]))
}

// The file's name is the address's hash, so nothing from the site ends up in a path.
async fn fetch_video(engine: &Engine, url: &str) -> Result<Decoded, String> {
    let response = net::get(&engine.client, &engine.store, url, Duration::from_secs(120)).await?;
    if response.content_length().is_some_and(|size| size > MAX_VIDEO) {
        return Err("too big".into());
    }
    let bytes = response.bytes().await.map_err(|_| "network error".to_string())?;
    let mut hash = DefaultHasher::new();
    url.hash(&mut hash);
    let path = video_dir().join(format!("{:x}", hash.finish()));
    tokio::fs::write(&path, &bytes).await.map_err(|_| "couldn't save".to_string())?;
    Ok(Decoded::Video(TempVideo(path)))
}

fn shrink(image: image::DynamicImage) -> image::DynamicImage {
    if image.width() > MAX_SIDE || image.height() > MAX_SIDE {
        image.resize(MAX_SIDE, MAX_SIDE, image::imageops::FilterType::Triangle)
    } else {
        image
    }
}

fn to_color(image: image::DynamicImage) -> ColorImage {
    let rgba = image.to_rgba8();
    ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_video_file_goes_with_its_entry() {
        let path = std::env::temp_dir().join("msg-native-test-video");
        std::fs::write(&path, b"x").unwrap();
        let video = TempVideo(path.clone());
        assert_eq!(video.path().as_deref(), path.to_str());
        drop(video);
        assert!(!path.exists());
    }

    #[test]
    fn a_download_nobody_asks_for_is_called_off() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let mut cache = MediaCache::new();
        let (passed, wanted) = (rt.spawn(std::future::pending::<()>()), rt.spawn(std::future::pending::<()>()));
        cache.entries.insert("passed".into(), (State::Loading, 1));
        cache.entries.insert("wanted".into(), (State::Loading, 2));
        cache.loading.insert("passed".into(), passed.abort_handle());
        cache.loading.insert("wanted".into(), wanted.abort_handle());
        cache.frame = 1; // The last frame asked for "wanted" only.
        cache.poll(&egui::Context::default());
        assert!(rt.block_on(passed).unwrap_err().is_cancelled());
        assert!(!wanted.is_finished());
        assert!(cache.entries.contains_key("wanted") && !cache.entries.contains_key("passed"));
    }
}
