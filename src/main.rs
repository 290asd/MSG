// MSG: a slideshow for booru sites, drawn with a native window (egui).
#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

mod app;
mod downloads;
mod favorites;
mod files;
mod glass;
mod hotkeys;
mod logo;
mod media;
mod net;
mod session;
mod settings_ui;
mod sites;
mod slide;
mod theme;
mod video;
mod store;

/// The window has no console, so a panic would vanish: it is added to crash.txt in the data folder, and
/// one that closes the window (on the main thread) is also shown. A panic elsewhere only stops that task.
fn report_panics() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_hook(info);
        let thread = std::thread::current().name().unwrap_or("?").to_string();
        let text = format!("{} MSG {} ({thread}): {info}\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), env!("CARGO_PKG_VERSION"));
        let file = files::user_data().join("crash.txt");
        let _ = std::fs::OpenOptions::new().create(true).append(true).open(&file).and_then(|mut f| std::io::Write::write_all(&mut f, text.as_bytes()));
        if thread == "main" {
            let message = format!("MSG stopped because of an error. It was written to {}.\n\n{info}", file.display());
            rfd::MessageDialog::new().set_title("MSG").set_description(message).set_level(rfd::MessageLevel::Error).show();
        }
    }));
}

fn main() -> eframe::Result {
    report_panics();
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build().expect("async runtime");
    let store = match store::Store::load(files::user_data(), files::legacy_dir()) {
        Ok(store) => store,
        Err(e) => {
            rfd::MessageDialog::new().set_title("MSG").set_description(e).set_level(rfd::MessageLevel::Error).show();
            return Ok(());
        }
    };
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../img/msg_icon_256.png")).ok();
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
