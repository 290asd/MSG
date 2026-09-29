// MSG's native side: the settings file, the window, files and downloads. The pages in ../frontend talk to it
// through js/tauri_shim.js, which keeps the window.chrome / window.appInfo shape they were written for.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod store;

use serde_json::{Map, Value};
use std::path::PathBuf;
use std::sync::Arc;
use store::Store;
use tauri::{Manager, State};

struct App {
    store: Arc<Store>,
}

#[tauri::command]
fn app_version(app: tauri::AppHandle) -> String {
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
fn storage_set(state: State<App>, items: Map<String, Value>) {
    state.store.data.lock().unwrap().extend(items);
    state.store.save();
}

#[tauri::command]
fn storage_remove(state: State<App>, keys: Value) {
    {
        let mut data = state.store.data.lock().unwrap();
        match keys {
            Value::String(key) => {
                data.remove(&key);
            }
            Value::Array(keys) => {
                for key in keys.iter().filter_map(Value::as_str) {
                    data.remove(key);
                }
            }
            _ => {}
        }
    }
    state.store.save();
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let config: PathBuf = app.path().config_dir()?;
            app.manage(App { store: Store::load(config.join("MSG"), config.join("booruslideshowelectron")) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![app_version, storage_get, storage_set, storage_remove])
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
