// Folders and file names: the user data folder, the download folder, and the "Your folders" listing.
// The same rules as src/files.rs (no leaving the download folder, names safe for Windows).
use super::net;
use super::sites::Config;
use super::slide::{MediaType, Slide};
use crate::store::Store;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

pub const IMAGE_EXTENSIONS: [&str; 7] = ["jpg", "jpeg", "png", "webp", "gif", "bmp", "avif"];
pub const VIDEO_EXTENSIONS: [&str; 5] = ["webm", "mp4", "m4v", "mov", "ogv"];
const MAX_LOCAL_FILES: usize = 50_000; // per folder, so a big one doesn't crowd out the others

pub fn extension(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase()
}

/// <config>/MSG, the same folder the web version keeps settings.json in.
pub fn user_data() -> PathBuf {
    dirs_config().join("MSG")
}

/// The folder used before the rename to MSG.
pub fn legacy_dir() -> PathBuf {
    dirs_config().join("booruslideshowelectron")
}

fn dirs_config() -> PathBuf {
    #[cfg(windows)]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"));
    #[cfg(all(unix, not(target_os = "macos")))]
    let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")));
    base.unwrap_or_default()
}

fn default_download_dir() -> PathBuf {
    #[cfg(windows)]
    let home = std::env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let home = std::env::var_os("HOME");
    home.map(|h| PathBuf::from(h).join("Downloads")).unwrap_or_default().join("MSG")
}

/// Where downloads go: the folder chosen in Settings → Folders, or <Downloads>/MSG.
/// `relative` may start with "MSG/" (that part means the folder); a name that tries to leave the folder is refused.
pub fn download_path(store: &Store, relative: &str) -> Result<PathBuf, String> {
    let base = store.string("downloadFolder").map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(default_download_dir);
    let relative = relative.strip_prefix("MSG").map(|r| r.strip_prefix('/').unwrap_or(r)).unwrap_or(relative);
    let mut target = base;
    for part in Path::new(relative).components() {
        match part {
            Component::Normal(name) => target.push(name),
            Component::CurDir => {}
            _ => return Err(format!("Download outside the download folder: {relative}")),
        }
    }
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

pub fn clean_file_name(name: &str) -> String {
    let replaced: String = name.chars().map(|c| if "<>:\"/\\|?*".contains(c) || (c as u32) < 32 { '_' } else { c }).collect();
    replaced.trim_start_matches('.').trim().chars().take(200).collect()
}

pub struct LocalFile {
    pub abs: String,
    // Starts with the chosen folder's own name, so folder names can be searched and shown as tags.
    pub relative: String,
    pub modified_ms: i64,
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
        } else {
            let ext = extension(&full);
            if !IMAGE_EXTENSIONS.contains(&ext.as_str()) && !VIDEO_EXTENSIONS.contains(&ext.as_str()) {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            let modified_ms = meta.modified().ok().and_then(|m| m.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64).unwrap_or(0);
            let relative = full.strip_prefix(root.parent().unwrap_or(root)).unwrap_or(&full);
            out.push(LocalFile { abs: full.to_string_lossy().into_owned(), relative: relative.to_string_lossy().into_owned(), modified_ms });
            *count += 1;
        }
    }
}

pub fn list_local_media(folders: &[String]) -> Vec<LocalFile> {
    let mut files = Vec::new();
    for folder in folders {
        let mut count = 0;
        walk(Path::new(folder), Path::new(folder), &mut files, &mut count);
    }
    files
}

/// The folders "Your folders" reads: those ticked in Settings → Folders, plus the downloaded copies offline.
pub fn local_folders(store: &Store) -> Vec<String> {
    let list = |key: &str| -> Vec<String> {
        store.get(key).and_then(|v| v.as_array().cloned()).unwrap_or_default().into_iter().filter_map(|v| v.as_str().map(String::from)).collect()
    };
    let off = list("localFoldersOff");
    let mut folders: Vec<String> = list("localFolders").into_iter().filter(|f| !off.contains(f)).collect();
    if store.flag("offlineMode") {
        folders.extend(["MSG/favorites", "MSG/pools"].iter().filter_map(|f| download_path(store, f).ok()).filter(|d| d.exists()).map(|d| d.to_string_lossy().into_owned()));
    }
    folders
}

/// The "Your folders" site. Search words filter by folder and file name (`-word` leaves out); empty shows everything.
pub fn local_slides(store: &Store, cfg: &Config, search: &str) -> Vec<Slide> {
    let files = list_local_media(&local_folders(store));
    // Offline, downloaded favorites get their tags from the favorites list (by md5).
    let mut favorite_tags: HashMap<String, String> = HashMap::new();
    if store.flag("offlineMode") {
        if let Some(Value::Array(items)) = store.get("personalListItems") {
            for item in items {
                if let (Some(md5), Some(tags)) = (item.get("md5").and_then(Value::as_str), item.get("tags").and_then(Value::as_str)) {
                    favorite_tags.insert(md5.to_string(), tags.to_lowercase());
                }
            }
        }
    }
    let pool = super::sites::pool_id(search);
    let sort_term = regex::Regex::new(r"(?i)(^|\s)(?:order|sort):\S+").unwrap();
    let cleaned = sort_term.replace_all(search, " ").to_lowercase().replace('_', " ");
    let words: Vec<&str> = if pool.is_some() { vec![] } else { cleaned.split_whitespace().filter(|w| *w != "*").collect() };
    let mut slides = Vec::new();
    for file in files {
        let parts: Vec<&str> = file.relative.split(['/', '\\']).collect();
        if let Some(id) = pool {
            if !parts[..parts.len() - 1].iter().any(|f| f.ends_with(&format!("({id})"))) {
                continue;
            }
        }
        let name = parts[parts.len() - 1];
        let stem = name.trim_start_matches(|c: char| c.is_ascii_digit()).trim_start().rsplit_once('.').map(|(s, _)| s).unwrap_or(name);
        let site_tags = favorite_tags.get(stem).cloned().unwrap_or_default();
        let searchable = format!("{} {site_tags}", parts.join(" ")).to_lowercase().replace(['_', '.', '-'], " ");
        let matches = words.iter().all(|w| match w.strip_prefix('-') {
            Some(w) => !searchable.contains(w),
            None => searchable.contains(w),
        });
        if !matches {
            continue;
        }
        // Tags: the folder names and the words of the file name.
        let file_stem = name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name);
        let mut tags: Vec<String> = parts[..parts.len() - 1].iter().map(|f| f.split_whitespace().collect::<Vec<_>>().join("_")).collect();
        tags.extend(file_stem.split(|c: char| c.is_whitespace() || c == '_' || c == '-').filter(|w| !w.is_empty()).map(String::from));
        let mut tags = tags.join(" ");
        if !site_tags.is_empty() {
            tags = format!("{tags} {site_tags}");
        }
        let kind = MediaType::from_path(&file.abs);
        if cfg.is_blacklisted(&tags) || !match kind {
            MediaType::Image => cfg.images,
            MediaType::Gif => cfg.gifs,
            _ => cfg.videos,
        } {
            continue;
        }
        slides.push(Slide {
            site_id: "LOCL".into(),
            id: Value::String(file.abs.clone()),
            preview_file_url: file.abs.clone(),
            viewable_website_post_url: file.abs.clone(),
            date: chrono::DateTime::from_timestamp_millis(file.modified_ms).map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)).unwrap_or_default(),
            media_type: kind,
            md5: file.abs.clone(),
            file_url: file.abs,
            tags,
            ..Default::default()
        });
    }
    if pool.is_some() {
        slides.sort_by(|a, b| a.file_url.cmp(&b.file_url));
    }
    slides
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_are_safe() {
        assert_eq!(clean_file_name("..a<b>:c.png"), "a_b__c.png");
        assert_eq!(safe_file_name("https://x.net/data/a%20b.jpg?x=1"), "a b.jpg");
    }

    #[test]
    fn download_path_stays_inside() {
        let store = Store::load(std::env::temp_dir().join("msg-native-test"), std::env::temp_dir().join("msg-native-test-legacy"));
        assert!(download_path(&store, "MSG/favorites/a.jpg").is_ok());
        assert!(download_path(&store, "MSG/../x").is_err());
        assert!(download_path(&store, "/etc/passwd").is_err());
    }
}
