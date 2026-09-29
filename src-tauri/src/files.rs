// Files on this computer: the dialogs, the background image, the "Your folders" listing and the
// downloaded copies. The pages get plain paths; js/tauri_shim.js turns them into URLs (the asset scheme).
use crate::{net, App};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

const MEDIA_EXTENSIONS: [&str; 12] = ["jpg", "jpeg", "png", "gif", "webp", "bmp", "avif", "webm", "mp4", "m4v", "mov", "ogv"];
const IMAGE_EXTENSIONS: [&str; 7] = ["jpg", "jpeg", "png", "webp", "gif", "bmp", "avif"];
const VIDEO_EXTENSIONS: [&str; 2] = ["webm", "mp4"];
const MAX_LOCAL_FILES: usize = 50_000; // per folder, so a big one doesn't crowd out the others

fn extension(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase()
}

/// The user data folder (settings.json, the background images): the same one Electron used.
pub fn user_data(app: &AppHandle) -> PathBuf {
    app.path().config_dir().unwrap_or_default().join("MSG")
}

/// Lets the pages load files from `dir` (asset scheme).
pub fn allow_dir(app: &AppHandle, dir: &Path) {
    let _ = app.asset_protocol_scope().allow_directory(dir, true);
}

/// Where downloads go: the folder chosen in Settings → Folders, or <Downloads>/MSG.
/// The pages ask for paths like "MSG/<file>" and "MSG/favorites"; the "MSG/" part means that folder.
/// A name that tries to leave the folder ("..", an absolute path) is refused.
pub fn download_path(app: &AppHandle, state: &App, relative: &str) -> Result<PathBuf, String> {
    let base = match state.store.string("downloadFolder").map(PathBuf::from).filter(|p| p.is_absolute()) {
        Some(chosen) => chosen,
        None => app.path().download_dir().map_err(|e| e.to_string())?.join("MSG"),
    };
    let relative = relative.strip_prefix("MSG").map(|r| r.strip_prefix('/').unwrap_or(r)).unwrap_or(relative);
    let mut target = base.clone();
    for part in Path::new(relative).components() {
        match part {
            Component::Normal(name) => target.push(name),
            Component::CurDir => {}
            _ => return Err(format!("Download outside the download folder: {relative}")),
        }
    }
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    allow_dir(app, &base);
    Ok(target)
}

/// A file name from a URL, safe for Windows: no folders, no reserved characters.
pub fn safe_file_name(url: &str) -> String {
    let name = net::web_url(url)
        .and_then(|u| u.path_segments().and_then(|s| s.last().map(str::to_string)))
        .and_then(|n| urlencoding::decode(&n).ok().map(|n| n.into_owned()))
        .unwrap_or_default();
    let name = clean_file_name(&name);
    if name.is_empty() {
        format!("file-{}", std::time::SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0))
    } else {
        name
    }
}

/// No folders, no characters Windows doesn't allow, no leading dots.
pub fn clean_file_name(name: &str) -> String {
    let replaced: String = name.chars().map(|c| if "<>:\"/\\|?*".contains(c) || (c as u32) < 32 { '_' } else { c }).collect();
    replaced.trim_start_matches('.').trim().chars().take(200).collect()
}

fn remove_background_images(app: &AppHandle) {
    let Ok(entries) = std::fs::read_dir(user_data(app)) else { return };
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().starts_with("background-") {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

// A new name each time, so the page doesn't show a cached old one.
fn background_target(app: &AppHandle, ext: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let dir = user_data(app);
    let _ = std::fs::create_dir_all(&dir);
    allow_dir(app, &dir);
    dir.join(format!("background-{stamp}.{ext}"))
}

#[tauri::command]
pub async fn choose_background(app: AppHandle) -> Result<Option<String>, String> {
    let picked = app
        .dialog()
        .file()
        .set_title("Choose a background image or video")
        .add_filter("Images and videos", &["jpg", "jpeg", "png", "webp", "gif", "bmp", "avif", "webm", "mp4"])
        .blocking_pick_file()
        .and_then(|f| f.into_path().ok());
    let Some(source) = picked else { return Ok(None) };
    remove_background_images(&app);
    let target = background_target(&app, &extension(&source));
    std::fs::copy(&source, &target).map_err(|e| e.to_string())?;
    Ok(Some(target.to_string_lossy().into_owned()))
}

#[tauri::command]
pub fn clear_background(app: AppHandle) {
    remove_background_images(&app);
}

#[tauri::command]
pub async fn choose_folder(app: AppHandle, title: String) -> Option<String> {
    let folder = app.dialog().file().set_title(title).blocking_pick_folder().and_then(|f| f.into_path().ok())?;
    allow_dir(&app, &folder);
    Some(folder.to_string_lossy().into_owned())
}

// Ctrl+L: the image being shown becomes the background image.
#[tauri::command]
pub async fn background_from_url(app: AppHandle, state: State<'_, App>, url: String) -> Result<String, String> {
    let response = net::get(&state, &url).await?;
    let ext = net::web_url(&url).map(|u| extension(Path::new(u.path()))).filter(|e| IMAGE_EXTENSIONS.contains(&e.as_str()) || VIDEO_EXTENSIONS.contains(&e.as_str())).unwrap_or_else(|| "jpg".into());
    let bytes = response.bytes().await.map_err(net::describe)?;
    remove_background_images(&app);
    let target = background_target(&app, &ext);
    std::fs::write(&target, bytes).map_err(|e| e.to_string())?;
    Ok(target.to_string_lossy().into_owned())
}

// Settings → Appearance: a random image among the downloaded favorites, as the background at start.
// Still images always; GIFs and videos only when asked for.
#[tauri::command]
pub fn random_favorite_image(app: AppHandle, state: State<App>, gifs: bool, videos: bool) -> Option<String> {
    let dir = download_path(&app, &state, "MSG/favorites").ok()?;
    let allowed = |ext: &str| (IMAGE_EXTENSIONS.contains(&ext) && (ext != "gif" || gifs)) || (videos && VIDEO_EXTENSIONS.contains(&ext));
    let images: Vec<PathBuf> = std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|p| allowed(&extension(p))).collect();
    if images.is_empty() {
        return None;
    }
    Some(images[rand::random::<usize>() % images.len()].to_string_lossy().into_owned())
}

#[derive(Serialize)]
pub struct LocalFile {
    path_abs: String,
    // Starts with the chosen folder's own name, so folder names can be searched and shown as tags.
    path: String,
    modified: f64,
}

fn walk(dir: &Path, root: &Path, out: &mut Vec<LocalFile>, count: &mut usize) {
    let Ok(entries) = std::fs::read_dir(dir) else { return }; // unreadable or removed folder
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        if *count >= MAX_LOCAL_FILES || entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let full = entry.path();
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            walk(&full, root, out, count);
        } else if MEDIA_EXTENSIONS.contains(&extension(&full).as_str()) {
            let Ok(meta) = entry.metadata() else { continue };
            let modified = meta.modified().ok().and_then(|m| m.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0);
            let relative = full.strip_prefix(root.parent().unwrap_or(root)).unwrap_or(&full);
            out.push(LocalFile { path_abs: full.to_string_lossy().into_owned(), path: relative.to_string_lossy().into_owned(), modified });
            *count += 1;
        }
    }
}

// The "Your folders" site: the images and videos in the chosen folders and their subfolders.
#[tauri::command]
pub async fn list_local_media(app: AppHandle, folders: Vec<String>) -> Vec<LocalFile> {
    for folder in &folders {
        allow_dir(&app, Path::new(folder));
    }
    tauri::async_runtime::spawn_blocking(move || {
        let mut files = Vec::new();
        for folder in folders {
            let mut count = 0;
            walk(Path::new(&folder), Path::new(&folder), &mut files, &mut count);
        }
        files
    })
    .await
    .unwrap_or_default()
}

// Offline mode: the downloaded favorites and pools are browsed like your own folders.
#[tauri::command]
pub fn offline_folders(app: AppHandle, state: State<App>) -> Vec<String> {
    ["MSG/favorites", "MSG/pools"].iter().filter_map(|f| download_path(&app, &state, f).ok()).filter(|d| d.exists()).map(|d| d.to_string_lossy().into_owned()).collect()
}

// Downloaded copies, by the name of the file on the site: pool pages have their page number in front.
#[tauri::command]
pub async fn list_local_copies(app: AppHandle, state: State<'_, App>) -> Result<HashMap<String, String>, String> {
    let base = download_path(&app, &state, "MSG/")?;
    Ok(tauri::async_runtime::spawn_blocking(move || {
        fn add(dir: &Path, copies: &mut HashMap<String, String>) {
            let Ok(entries) = std::fs::read_dir(dir) else { return };
            for entry in entries.flatten() {
                let path = entry.path();
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    add(&path, copies);
                } else if MEDIA_EXTENSIONS.contains(&extension(&path).as_str()) {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    let digits = name.chars().take_while(char::is_ascii_digit).count();
                    let key = if digits > 0 && name[digits..].starts_with(' ') { name[digits + 1..].to_string() } else { name };
                    copies.insert(key, path.to_string_lossy().into_owned());
                }
            }
        }
        let mut copies = HashMap::new();
        add(&base.join("favorites"), &mut copies);
        add(&base.join("pools"), &mut copies);
        copies
    })
    .await
    .unwrap_or_default())
}
