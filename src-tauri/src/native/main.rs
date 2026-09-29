// MSG with a native window (egui) instead of the web view. It reads and writes the same settings.json,
// so settings and favorites are shared with the web version.
#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

mod files;
mod net;
mod session;
mod sites;
mod slide;
#[path = "../store.rs"]
mod store;

fn main() {
    println!("MSG native");
}
