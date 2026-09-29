// MSG's native side: the settings file, the window, files and downloads. The pages in ../frontend talk to it
// through js/tauri_shim.js, which keeps the window.chrome / window.appInfo shape they were written for.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod files;
mod net;
mod store;

use serde_json::{Map, Value};
use std::sync::Arc;
use store::Store;
use tauri::{image::Image, webview::Color, AppHandle, Manager, State, Theme, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub struct App {
    pub store: Arc<Store>,
    pub client: reqwest::Client,
}

#[tauri::command]
fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
fn storage_get(state: State<App>, keys: Value) -> Value {
    let data = state.store.data.lock().unwrap();
    let mut result = Map::new();
    match keys {
        Value::Null => return Value::Object(data.clone()),
        Value::String(key) => {
            if let Some(v) = data.get(&key) {
                result.insert(key, v.clone());
            }
        }
        Value::Array(keys) => {
            for key in keys.iter().filter_map(Value::as_str) {
                if let Some(v) = data.get(key) {
                    result.insert(key.to_string(), v.clone());
                }
            }
        }
        // {key: default}
        Value::Object(defaults) => {
            for (key, default) in defaults {
                let v = data.get(&key).cloned().unwrap_or(default);
                result.insert(key, v);
            }
        }
        _ => {}
    }
    Value::Object(result)
}

#[tauri::command]
fn storage_set(app: AppHandle, state: State<App>, items: Map<String, Value>) {
    let folder_changed = items.contains_key("downloadFolder");
    state.store.data.lock().unwrap().extend(items);
    state.store.save();
    if folder_changed {
        let _ = files::download_path(&app, &state, "MSG/"); // lets the pages load from the new folder
    }
}

#[tauri::command]
fn storage_remove(state: State<App>, keys: Value) {
    {
        let mut data = state.store.data.lock().unwrap();
        match keys {
            Value::String(key) => {
                data.shift_remove(&key);
            }
            Value::Array(keys) => {
                for key in keys.iter().filter_map(Value::as_str) {
                    data.shift_remove(key);
                }
            }
            _ => {}
        }
    }
    state.store.save();
}

// The logo is white on the dark theme and black on the light one.
fn window_icon(dark: bool) -> Image<'static> {
    let bytes: &[u8] = if dark { include_bytes!("../../frontend/img/msg_icon_white_256.png") } else { include_bytes!("../../frontend/img/msg_icon_black_256.png") };
    Image::from_bytes(bytes).expect("window icon")
}

#[tauri::command]
fn theme_changed(window: WebviewWindow, dark: bool) {
    let _ = window.set_icon(window_icon(dark));
}

#[tauri::command]
fn toggle_fullscreen(window: WebviewWindow) {
    let _ = window.set_fullscreen(!window.is_fullscreen().unwrap_or(false));
}

#[tauri::command]
fn exit_fullscreen(window: WebviewWindow) {
    if window.is_fullscreen().unwrap_or(false) {
        let _ = window.set_fullscreen(false);
    }
}

#[tauri::command]
fn toggle_devtools(window: WebviewWindow) {
    if window.is_devtools_open() {
        window.close_devtools();
    } else {
        window.open_devtools();
    }
}

// Links and window.open go to the system browser; a file from your own folders ("open post", E) is shown in Explorer.
#[tauri::command]
fn open_external(url: String) {
    if net::web_url(&url).is_some() {
        let _ = tauri_plugin_opener::open_url(url, None::<&str>);
    }
}

#[tauri::command]
fn show_in_folder(path: String) {
    let _ = tauri_plugin_opener::reveal_item_in_dir(path);
}

fn create_window(app: &AppHandle) -> tauri::Result<()> {
    let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("slideshow.html".into()))
        .title("MSG")
        .inner_size(1280.0, 900.0)
        .background_color(Color(11, 12, 16, 255))
        // The window only shows the app's own pages; a link that would replace them opens in the browser.
        .on_navigation(|url| {
            let ours = matches!(url.scheme(), "tauri" | "about") || url.host_str() == Some("tauri.localhost");
            if !ours && matches!(url.scheme(), "http" | "https") {
                let _ = tauri_plugin_opener::open_url(url.as_str(), None::<&str>);
            }
            ours
        })
        .build()?;

    // The theme saved in the settings (app_settings.js), before the page has told us.
    let state = app.state::<App>();
    let dark = match state.store.string("appTheme").as_deref() {
        Some("light") => false,
        Some("system") => window.theme().map(|t| t == Theme::Dark).unwrap_or(true),
        _ => true,
    };
    window.set_icon(window_icon(dark))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .register_asynchronous_uri_scheme_protocol("msg-proxy", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                responder.respond(net::proxy(&app.state::<App>(), request).await);
            });
        })
        .setup(|app| {
            let handle = app.handle();
            let user_data = files::user_data(handle);
            let store = Store::load(user_data.clone(), user_data.with_file_name("booruslideshowelectron"));
            app.manage(App { store, client: net::client() });
            files::allow_dir(handle, &user_data);
            let state = app.state::<App>();
            let _ = files::download_path(handle, &state, "MSG/");
            create_window(handle)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_version,
            storage_get,
            storage_set,
            storage_remove,
            theme_changed,
            toggle_fullscreen,
            exit_fullscreen,
            toggle_devtools,
            open_external,
            show_in_folder,
            net::http_request,
            files::choose_background,
            files::clear_background,
            files::choose_folder,
            files::background_from_url,
            files::random_favorite_image,
            files::list_local_media,
            files::offline_folders,
            files::list_local_copies,
        ])
        .build(tauri::generate_context!())
        .expect("error while building MSG")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                if let Some(state) = app.try_state::<App>() {
                    state.store.flush();
                }
            }
        });
}
