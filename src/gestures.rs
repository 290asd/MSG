//! gestures: records the touchpad's scrolling, gesture by gesture, for tuning the slide strip (`roll.rs`).
//! Each event is a row in gestures-<time>.csv next to the exe: task, try, frame, ms, unit, dx, dy.
#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

use eframe::egui;
use std::time::{Duration, Instant};

const TASKS: [(&str, &str); 3] = [
    ("next", "Move to the next picture, as you would in the slideshow."),
    ("edge", "Peek at the next picture a little, then let it go back without moving on."),
    ("many", "Flick properly, to pass several pictures at once."),
];
const TRIES: usize = 5;
/// No event for this long ends a try (Windows' coasting comes as events, so it counts too).
const END: Duration = Duration::from_millis(500);

struct Rec {
    start: Instant,
    frame: u64,
    task: usize,
    tries: usize,
    last: Option<Instant>,
    /// How far this try has gone, in touchpad points, for the picture on screen.
    travel: f32,
    rows: Vec<String>,
    saved: Option<String>,
}

impl eframe::App for Rec {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.frame += 1;
        let now = Instant::now();
        if self.task < TASKS.len() {
            for event in ctx.input(|i| i.events.clone()) {
                let egui::Event::MouseWheel { unit, delta, .. } = event else { continue };
                let ms = (now - self.start).as_secs_f64() * 1000.0;
                self.rows.push(format!("{},{},{},{ms:.1},{unit:?},{},{}", TASKS[self.task].0, self.tries + 1, self.frame, delta.x, delta.y));
                self.travel -= delta.y * if unit == egui::MouseWheelUnit::Point { 1.0 } else { 100.0 };
                self.last = Some(now);
            }
            if self.last.is_some_and(|t| now - t > END) {
                self.last = None;
                self.travel = 0.0;
                self.tries += 1;
                if self.tries == TRIES {
                    self.tries = 0;
                    self.task += 1;
                }
                if self.task == TASKS.len() {
                    self.saved = Some(self.write_csv());
                }
            }
            if self.last.is_some() {
                ctx.request_repaint_after(Duration::from_millis(50));
            }
        }
        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(saved) = &self.saved {
                ui.heading("Done, thank you.");
                ui.label(format!("Saved to {saved}"));
                return;
            }
            let (_, text) = TASKS[self.task];
            ui.heading(text);
            ui.label(format!("Task {} of {}, try {} of {TRIES}. Wait half a second between tries.", self.task + 1, TASKS.len(), self.tries + 1));
            // Two "pictures" one above the other, moved 1:1 by the touchpad, so the fingers have something to push.
            let rect = ui.available_rect_before_wrap().shrink(24.0);
            let page = rect.height() + 24.0;
            let painter = ui.painter_at(rect);
            for k in -1..=4 {
                let at = rect.translate(egui::vec2(0.0, k as f32 * page - self.travel));
                painter.rect_filled(at.shrink(8.0), 8.0, egui::Color32::from_gray(if k % 2 == 0 { 70 } else { 110 }));
                painter.text(at.center(), egui::Align2::CENTER_CENTER, format!("{}", k + 1), egui::FontId::proportional(64.0), egui::Color32::WHITE);
            }
        });
    }
}

impl Rec {
    fn write_csv(&self) -> String {
        let dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_default();
        let path = dir.join(format!("gestures-{}.csv", chrono::Local::now().format("%Y%m%d-%H%M%S")));
        let text = format!("task,try,frame,ms,unit,dx,dy\n{}\n", self.rows.join("\n"));
        match std::fs::write(&path, text) {
            Ok(()) => path.display().to_string(),
            Err(e) => format!("nowhere: {e}"),
        }
    }
}

fn main() -> eframe::Result {
    let viewport = egui::ViewportBuilder::default().with_inner_size([1000.0, 800.0]).with_title("MSG gestures");
    eframe::run_native(
        "MSG gestures",
        eframe::NativeOptions { viewport, ..Default::default() },
        Box::new(|_| {
            Ok(Box::new(Rec { start: Instant::now(), frame: 0, task: 0, tries: 0, last: None, travel: 0.0, rows: Vec::new(), saved: None }))
        }),
    )
}
