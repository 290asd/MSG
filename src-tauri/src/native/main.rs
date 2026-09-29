// MSG with a native window (egui) instead of the web view. It reads and writes the same settings.json,
// so settings and favorites are shared with the web version.
#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

mod app;
mod downloads;
mod favorites;
mod files;
mod hotkeys;
mod media;
mod net;
mod session;
mod settings_ui;
mod sites;
mod slide;
#[path = "../store.rs"]
mod store;

fn main() -> eframe::Result {
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build().expect("async runtime");
    let store = store::Store::load(files::user_data(), files::legacy_dir());
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../../../frontend/img/msg_icon_white_256.png")).ok();
    let mut viewport = eframe::egui::ViewportBuilder::default().with_inner_size([1280.0, 900.0]).with_min_inner_size([640.0, 480.0]).with_title("MSG");
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }
    eframe::run_native(
        "MSG",
        eframe::NativeOptions { viewport, ..Default::default() },
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, rt, store)))),
    )
}
