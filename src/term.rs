// Pictures for the terminal: fetched (or read from the disk), decoded, fitted to the room they get and
// turned into text off the main thread: Sixel for Windows Terminal, half blocks for other terminals. Only the
// file's bytes and the text for the current size are kept, not the decoded pixels.
use super::net;
use super::session::Engine;
use image::{imageops, RgbaImage};
use std::collections::HashMap;
use std::fmt::Write;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

/// Windows Terminal (through ConPTY) places a Sixel image on a virtual 10×20 pixel cell and scales it to the font.
pub const CELL: (u32, u32) = (10, 20);
/// Pixel rows in one Sixel image. Windows Terminal colours an image with the palette it has at its end, so one
/// image has 256 colours; a picture is drawn as strips of 3 cells (10 bands of 6 rows), each with 256 of its own.
const STRIP: u32 = 60;
const KEEP: usize = 12;
// NeuQuant looks at every 10th pixel of a strip, as GIF encoders do (1 = every pixel, slowest).
const SAMPLE: i32 = 10;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Output {
    Sixel,
    Blocks,
}

/// Room for the picture, in cells; `fit` lets a small picture grow to fill it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Area {
    pub cols: u16,
    pub rows: u16,
    pub fit: bool,
}

/// The picture as text (a Sixel image per strip, or a line of half blocks per row), the rows from one line
/// to the next, and its size in cells.
pub struct Rendered {
    pub lines: Vec<String>,
    pub step: u16,
    pub cols: u16,
    pub rows: u16,
}

pub enum State {
    Loading,
    /// `out` is drawn for its area; `pending` is another area it is being drawn for (the window changed size).
    Ready { bytes: Arc<Vec<u8>>, out: (Area, Rendered), pending: Option<Area> },
    Failed(String),
}

type Done = (String, Area, Result<(Arc<Vec<u8>>, Rendered), String>);

pub struct Cache {
    output: Output,
    entries: HashMap<String, (State, u64)>,
    tx: Sender<Done>,
    rx: Receiver<Done>,
    tick: u64,
}

impl Cache {
    pub fn new(output: Output) -> Cache {
        let (tx, rx) = channel();
        Cache { output, entries: HashMap::new(), tx, rx, tick: 0 }
    }

    pub fn get(&mut self, url: &str) -> Option<&State> {
        self.tick += 1;
        let tick = self.tick;
        self.entries.get_mut(url).map(|(state, used)| {
            *used = tick;
            &*state
        })
    }

    /// Starts loading `url` (an address or a file path) and drawing it for `area`, unless that is done or under way.
    pub fn request(&mut self, engine: &Engine, url: &str, area: Area) {
        if url.is_empty() || area.cols == 0 || area.rows == 0 {
            return;
        }
        self.tick += 1;
        let bytes = match self.entries.get_mut(url) {
            None => {
                self.entries.insert(url.to_string(), (State::Loading, self.tick));
                None
            }
            Some((state, used)) => {
                *used = self.tick;
                match state {
                    State::Ready { bytes, out, pending } if out.0 != area && *pending != Some(area) => {
                        *pending = Some(area);
                        Some(bytes.clone())
                    }
                    _ => return,
                }
            }
        };
        let (tx, engine, url, output) = (self.tx.clone(), engine.clone(), url.to_string(), self.output);
        engine.rt.clone().spawn(async move {
            let bytes = match bytes {
                Some(bytes) => Ok(bytes),
                None if net::web_url(&url).is_some() => net::get_bytes(&engine.client, &engine.store, &url).await.map(Arc::new),
                None => tokio::fs::read(&url).await.map(Arc::new).map_err(|_| "file not found".to_string()),
            };
            let result = match bytes {
                Ok(bytes) => tokio::task::spawn_blocking(move || {
                    let drawn = render(&bytes, area, output);
                    drawn.map(|r| (bytes, r))
                })
                .await
                .unwrap_or_else(|_| Err("drawing failed".into())),
                Err(e) => Err(e),
            };
            let _ = tx.send((url, area, result));
            engine.wake();
        });
    }

    /// Takes in what has been drawn, and lets the oldest go when there are too many.
    pub fn poll(&mut self) {
        while let Ok((url, area, result)) = self.rx.try_recv() {
            if let Some(entry) = self.entries.get_mut(&url) {
                entry.0 = match result {
                    Ok((bytes, rendered)) => State::Ready { bytes, out: (area, rendered), pending: None },
                    Err(e) => State::Failed(e),
                };
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

/// Decodes the file (a GIF gives its first frame) and draws it as text that fits `area`.
pub fn render(bytes: &[u8], area: Area, output: Output) -> Result<Rendered, String> {
    let picture = image::load_from_memory(bytes).map_err(|_| "not a picture".to_string())?;
    let (cell_w, cell_h) = match output {
        Output::Sixel => CELL,
        Output::Blocks => (1, 2),
    };
    // Sixel draws 6 pixel rows at a time: the height stays whole bands, inside the area.
    let (room_w, room_h) = (area.cols as u32 * cell_w, area.rows as u32 * cell_h);
    let room_h = if output == Output::Sixel { room_h / 6 * 6 } else { room_h };
    let (w, h) = (picture.width().max(1), picture.height().max(1));
    let mut scale = (room_w as f64 / w as f64).min(room_h as f64 / h as f64);
    if !area.fit {
        scale = scale.min(1.0);
    }
    let (w, h) = (((w as f64 * scale) as u32).clamp(1, room_w.max(1)), ((h as f64 * scale) as u32).clamp(1, room_h.max(1)));
    let mut pixels = picture.resize_exact(w, h, imageops::FilterType::Triangle).to_rgba8();
    // Transparent parts over the black background.
    for p in pixels.pixels_mut() {
        let a = p[3] as u32;
        for c in 0..3 {
            p[c] = (p[c] as u32 * a / 255) as u8;
        }
        p[3] = 255;
    }
    Ok(match output {
        Output::Sixel => Rendered { lines: sixel(&pixels), step: (STRIP / cell_h) as u16, cols: w.div_ceil(cell_w) as u16, rows: h.div_ceil(cell_h) as u16 },
        Output::Blocks => Rendered { lines: blocks(&pixels), step: 1, cols: w as u16, rows: h.div_ceil(2) as u16 },
    })
}

/// The picture as Sixel images, one per strip of 60 rows.
pub fn sixel(pixels: &RgbaImage) -> Vec<String> {
    let (w, h) = pixels.dimensions();
    (0..h).step_by(STRIP as usize).map(|top| sixel_image(imageops::crop_imm(pixels, 0, top, w, STRIP.min(h - top)).to_image())).collect()
}

/// One Sixel image with up to 256 colours (the format has 101 levels per channel).
fn sixel_image(pixels: RgbaImage) -> String {
    let (w, h) = pixels.dimensions();
    let width = w as usize;
    let (palette, indices) = colors(pixels);
    // P2=1: what the image doesn't draw stays as it is.
    let mut out = format!("\x1bP0;1q\"1;1;{w};{h}");
    let pct = |v: u8| (v as u32 * 100 + 127) / 255;
    for (i, c) in palette.iter().enumerate() {
        let _ = write!(out, "#{i};2;{};{};{}", pct(c[0]), pct(c[1]), pct(c[2]));
    }
    // For each colour, a column of 6 bits per x.
    let mut bits = vec![0u8; palette.len() * width];
    for (n, band) in indices.chunks(6 * width).enumerate() {
        if n > 0 {
            out.push('-');
        }
        let mut used = [false; 256];
        for (k, &i) in band.iter().enumerate() {
            bits[i as usize * width + k % width] |= 1 << (k / width);
            used[i as usize] = true;
        }
        let mut first = true;
        for i in (0..palette.len()).filter(|&i| used[i]) {
            if !first {
                out.push('$');
            }
            first = false;
            let _ = write!(out, "#{i}");
            let row = &mut bits[i * width..(i + 1) * width];
            let end = row.iter().rposition(|&b| b != 0).map_or(0, |p| p + 1);
            let mut x = 0;
            while x < end {
                let b = row[x];
                let run = row[x..end].iter().take_while(|&&v| v == b).count();
                let ch = (63 + b) as char;
                if run > 3 {
                    let _ = write!(out, "!{run}{ch}");
                } else {
                    (0..run).for_each(|_| out.push(ch));
                }
                x += run;
            }
            row.fill(0);
        }
    }
    out.push_str("\x1b\\");
    out
}

/// The colours (at most 256) and each pixel's colour: the exact ones when there are few enough,
/// else NeuQuant's 256 with Floyd–Steinberg dithering.
fn colors(mut pixels: RgbaImage) -> (Vec<[u8; 3]>, Vec<u8>) {
    let mut palette: Vec<[u8; 3]> = vec![];
    let mut seen: HashMap<[u8; 3], u8> = HashMap::new();
    let mut indices = Vec::with_capacity((pixels.width() * pixels.height()) as usize);
    let exact = pixels.pixels().all(|p| {
        let c = [p[0], p[1], p[2]];
        let i = match seen.get(&c) {
            Some(&i) => i,
            None if palette.len() < 256 => {
                seen.insert(c, palette.len() as u8);
                palette.push(c);
                (palette.len() - 1) as u8
            }
            None => return false,
        };
        indices.push(i);
        true
    });
    if exact {
        return (palette, indices);
    }
    let quant = color_quant::NeuQuant::new(SAMPLE, 256, pixels.as_raw());
    imageops::dither(&mut pixels, &quant);
    let indices = imageops::index_colors(&pixels, &quant).into_raw();
    let palette = quant.color_map_rgb().chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
    (palette, indices)
}

/// Half blocks in 24-bit colour: the upper pixel is the letter's colour, the lower one the background.
pub fn blocks(pixels: &RgbaImage) -> Vec<String> {
    let (w, h) = pixels.dimensions();
    (0..h)
        .step_by(2)
        .map(|y| {
            let mut line = String::new();
            for x in 0..w {
                let t = pixels.get_pixel(x, y).0;
                let b = if y + 1 < h { pixels.get_pixel(x, y + 1).0 } else { [0, 0, 0, 255] };
                let _ = write!(line, "\x1b[38;2;{};{};{};48;2;{};{};{}m▀", t[0], t[1], t[2], b[0], b[1], b[2]);
            }
            line.push_str("\x1b[0m");
            line
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn picture(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 3]) -> RgbaImage {
        RgbaImage::from_fn(w, h, |x, y| {
            let [r, g, b] = f(x, y);
            Rgba([r, g, b, 255])
        })
    }

    #[test]
    fn sixel_colours_bands_and_runs() {
        let two = picture(2, 1, |x, _| if x == 0 { [255, 0, 0] } else { [0, 0, 255] });
        assert_eq!(sixel(&two), ["\x1bP0;1q\"1;1;2;1#0;2;100;0;0#1;2;0;0;100#0@$#1?@\x1b\\"]);
        let row = picture(10, 1, |_, _| [255, 0, 0]);
        assert_eq!(sixel(&row), ["\x1bP0;1q\"1;1;10;1#0;2;100;0;0#0!10@\x1b\\"]);
        let two_bands = picture(1, 12, |_, y| if y < 6 { [255, 0, 0] } else { [0, 0, 255] });
        assert_eq!(sixel(&two_bands), ["\x1bP0;1q\"1;1;1;12#0;2;100;0;0#1;2;0;0;100#0~-#1~\x1b\\"]);
    }

    #[test]
    fn each_strip_has_its_palette() {
        // 61 rows: a strip of 60 and one of 1, each a Sixel image with its own colours.
        let strips = sixel(&picture(1, 61, |_, y| if y < 60 { [255, 0, 0] } else { [0, 0, 255] }));
        assert_eq!(strips.len(), 2);
        assert!(strips[1].starts_with("\x1bP0;1q\"1;1;1;1#0;2;0;0;100"));
        // 600 colours: quantized, at most 256 registers set.
        let gradient = sixel(&picture(600, 1, |x, _| [(x % 256) as u8, (x / 3) as u8, 7]));
        assert!(gradient[0].matches(";2;").count() <= 256 && gradient[0].ends_with("\x1b\\"));
    }

    #[test]
    fn half_blocks() {
        let lines = blocks(&picture(3, 3, |_, _| [1, 2, 3]));
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|l| l.matches('▀').count() == 3));
        // The missing lower row of an odd height is black.
        assert!(lines[1].contains("48;2;0;0;0m"));
    }

    #[test]
    fn fits_the_area() {
        let mut png = vec![];
        picture(100, 50, |_, _| [9, 9, 9]).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let area = Area { cols: 10, rows: 5, fit: true };
        // Sixel: room 100×96, the picture stays 100×50.
        let r = render(&png, area, Output::Sixel).unwrap();
        assert_eq!((r.cols, r.rows), (10, 3));
        // Half blocks: room 10×10, the picture becomes 10×5.
        let r = render(&png, area, Output::Blocks).unwrap();
        assert_eq!((r.cols, r.rows), (10, 3));
        // Not fitted: a small picture is not enlarged.
        assert_eq!(render(&png, Area { cols: 40, rows: 20, fit: false }, Output::Sixel).unwrap().cols, 10);
    }
}
