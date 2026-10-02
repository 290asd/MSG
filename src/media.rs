// Pictures for the window: fetched (or read from the disk), decoded off the UI thread and turned into
// textures when they arrive. A GIF keeps all its frames; the window picks the frame by time.
use super::net;
use super::session::Engine;
use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

// Bigger pictures are shrunk: a GPU texture has a size limit (and 8K RGBA is 130 MB).
const MAX_SIDE: u32 = 4096;
const MAX_GIF_FRAMES: usize = 400;
const KEEP: usize = 90;
// At most this many downloads at once, so a row of thumbnails doesn't hammer a site's image server;
// the rest wait their turn in the order they were asked for (the picture on the screen comes first).
const AT_ONCE: usize = 6;

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
}

struct Decoded {
    frames: Vec<(ColorImage, Duration)>,
}

pub struct MediaCache {
    entries: HashMap<String, (State, u64)>,
    tx: Sender<(String, Result<Decoded, String>)>,
    rx: Receiver<(String, Result<Decoded, String>)>,
    tick: u64,
    limit: Arc<tokio::sync::Semaphore>,
}

impl MediaCache {
    pub fn new() -> MediaCache {
        let (tx, rx) = channel();
        MediaCache { entries: HashMap::new(), tx, rx, tick: 0, limit: Arc::new(tokio::sync::Semaphore::new(AT_ONCE)) }
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
        if url.is_empty() {
            return;
        }
        self.tick += 1;
        if let Some((_, used)) = self.entries.get_mut(url) {
            *used = self.tick;
            return;
        }
        self.entries.insert(url.to_string(), (State::Loading, self.tick));
        let (tx, engine, url, limit) = (self.tx.clone(), engine.clone(), url.to_string(), self.limit.clone());
        engine.rt.clone().spawn(async move {
            let bytes = if net::web_url(&url).is_some() {
                let _turn = limit.acquire().await;
                net::get_bytes(&engine.client, &engine.store, &url).await
            } else {
                tokio::fs::read(&url).await.map_err(|_| "file not found".to_string())
            };
            let result = match bytes {
                Ok(bytes) => tokio::task::spawn_blocking(move || decode(&bytes)).await.unwrap_or_else(|_| Err("decoding failed".into())),
                Err(e) => Err(e),
            };
            let _ = tx.send((url, result));
            engine.wake();
        });
    }

    /// Moves what has arrived into textures, and lets the oldest go when there are too many.
    pub fn poll(&mut self, ctx: &egui::Context) {
        while let Ok((url, result)) = self.rx.try_recv() {
            let state = match result {
                Ok(decoded) => {
                    let size = decoded.frames[0].0.size;
                    let frames: Vec<Frame> = decoded
                        .frames
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
        if self.entries.len() > KEEP {
            let mut by_use: Vec<_> = self.entries.iter().map(|(k, (_, used))| (*used, k.clone())).collect();
            by_use.sort();
            for (_, key) in by_use.into_iter().take(self.entries.len() - KEEP) {
                self.entries.remove(&key);
            }
        }
    }
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
            return Ok(Decoded { frames });
        }
    }
    let image = image::load_from_memory_with_format(bytes, format).map_err(|e| e.to_string())?;
    Ok(Decoded { frames: vec![(to_color(shrink(image)), Duration::ZERO)] })
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
