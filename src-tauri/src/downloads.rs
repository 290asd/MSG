// Downloads: one file (the button, L), many files (Download all favorites) and the background job that keeps
// offline copies of the favorites and the saved pools. Files go to files::download_path(...).
use crate::files::{clean_file_name, download_path, safe_file_name};
use crate::{net, App};
use base64::Engine;
use serde::Serialize;
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::time::sleep;

#[derive(Default)]
pub struct Downloads {
    bulk_cancelled: AtomicBool,
    auto: Mutex<Auto>,
}

#[derive(Default)]
struct Auto {
    running: bool,
    again: bool,
    // Bumped by every schedule(): a waiting timer that isn't the latest does nothing (like clearTimeout).
    timer: u64,
    status: String,
}

#[derive(PartialEq)]
enum Outcome {
    Downloaded,
    Skipped,
}

/// Saves `url` to `path` (through `path.part`, so a stopped download never looks finished). Files already
/// there are skipped, so a rerun resumes.
async fn download_file(state: &App, url: &str, path: &Path, mut progress: impl FnMut(u64, Option<u64>)) -> Result<Outcome, String> {
    if std::fs::metadata(path).map(|m| m.len() > 0).unwrap_or(false) {
        return Ok(Outcome::Skipped);
    }
    let mut response = net::get(state, url).await?;
    let total = response.content_length();
    let part = PathBuf::from(format!("{}.part", path.to_string_lossy()));
    let mut file = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    let mut received = 0u64;
    while let Some(chunk) = response.chunk().await.map_err(net::describe)? {
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        received += chunk.len() as u64;
        progress(received, total);
    }
    drop(file);
    std::fs::rename(&part, path).map_err(|e| e.to_string())?;
    Ok(Outcome::Downloaded)
}

// chrome.downloads.download: saves to <download folder>/downloads/<name>, overwriting.
#[tauri::command]
pub async fn download(app: AppHandle, state: State<'_, App>, url: String) -> Result<(), String> {
    let name = safe_file_name(&url);
    let target = download_path(&app, &state, &format!("downloads/{name}"))?;
    let _ = std::fs::remove_file(&target);
    tauri::async_runtime::spawn(async move {
        let state = app.state::<App>();
        // Progress for the corner box (Settings → Developer): its own channel, so it doesn't
        // overwrite the background download's status.
        let mut last = Instant::now();
        let outcome = download_file(&state, &url, &target, |received, total| {
            if last.elapsed() > Duration::from_millis(250) {
                last = Instant::now();
                let percent = total.filter(|t| *t > 0).map(|t| format!(": {}%", received * 100 / t)).unwrap_or_default();
                let _ = app.emit("download-progress", format!("Downloading {name}{percent}"));
            }
        })
        .await;
        let _ = app.emit("download-progress", if outcome.is_ok() { format!("Saved {name}") } else { format!("Download failed: {name}") });
    });
    Ok(())
}

#[derive(Serialize, Clone, Default)]
pub struct BulkResult {
    total: usize,
    downloaded: usize,
    skipped: usize,
    failed: usize,
    cancelled: bool,
    folder: String,
}

// Bulk download (Download all favorites in the favorites page's settings): 3 files at a time.
#[tauri::command]
pub async fn download_many(app: AppHandle, state: State<'_, App>, urls: Vec<String>, folder: String) -> Result<BulkResult, String> {
    state.downloads.bulk_cancelled.store(false, Ordering::SeqCst);
    let dir = download_path(&app, &state, &folder)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let result = std::sync::Arc::new(Mutex::new(BulkResult { total: urls.len(), folder: dir.to_string_lossy().into_owned(), ..Default::default() }));
    let urls = std::sync::Arc::new(urls);
    let next = std::sync::Arc::new(AtomicUsize::new(0));

    let workers: Vec<_> = (0..3)
        .map(|_| {
            let (app, result, urls, next, dir) = (app.clone(), result.clone(), urls.clone(), next.clone(), dir.clone());
            tauri::async_runtime::spawn(async move {
                let state = app.state::<App>();
                loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    if i >= urls.len() || state.downloads.bulk_cancelled.load(Ordering::SeqCst) {
                        break;
                    }
                    let outcome = download_file(&state, &urls[i], &dir.join(safe_file_name(&urls[i])), |_, _| {}).await;
                    let snapshot = {
                        let mut result = result.lock().unwrap();
                        match outcome {
                            Ok(Outcome::Downloaded) => result.downloaded += 1,
                            Ok(Outcome::Skipped) => result.skipped += 1,
                            Err(_) => result.failed += 1,
                        }
                        result.clone()
                    };
                    let _ = app.emit("download-many-progress", snapshot);
                }
            })
        })
        .collect();
    for worker in workers {
        let _ = worker.await;
    }
    let mut result = result.lock().unwrap().clone();
    result.cancelled = state.downloads.bulk_cancelled.load(Ordering::SeqCst);
    Ok(result)
}

#[tauri::command]
pub fn download_many_cancel(state: State<App>) {
    state.downloads.bulk_cancelled.store(true, Ordering::SeqCst);
}

// ---------- Offline copies (Settings → Favorites) ----------
// Favorites go to <download folder>/favorites/<file name>, saved pools to
// <download folder>/pools/<name> (<id>)/<page number> <file name>. This runs in the background, so it
// goes on while the pages change. Files already there are skipped.

pub const AUTO_DOWNLOAD_KEYS: [&str; 6] = ["personalListItems", "savedPools", "autoDownloadFavorites", "autoDownloadPools", "downloadFolder", "offlineMode"];

pub fn schedule(app: &AppHandle, delay_ms: u64) {
    let state = app.state::<App>();
    let timer = {
        let mut auto = state.downloads.auto.lock().unwrap();
        auto.timer += 1;
        auto.timer
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        sleep(Duration::from_millis(delay_ms)).await;
        let current = app.state::<App>().downloads.auto.lock().unwrap().timer;
        if current == timer {
            run(&app).await;
        }
    });
}

fn set_status(app: &AppHandle, text: String) {
    app.state::<App>().downloads.auto.lock().unwrap().status = text.clone();
    let _ = app.emit("auto-download-status", text);
}

#[tauri::command]
pub fn auto_download_status(state: State<App>) -> String {
    state.downloads.auto.lock().unwrap().status.clone()
}

// e621's API from here: its rate limit wants a pause between requests and a descriptive User-Agent.
async fn e621_json(app: &AppHandle, path_and_query: &str) -> Result<Value, String> {
    sleep(Duration::from_millis(700)).await;
    let state = app.state::<App>();
    let mut request = state.client.get(format!("https://e621.net{path_and_query}")).header("User-Agent", format!("MSG/{} (offline copies)", app.package_info().version));
    if let (Some(login), Some(key)) = (state.store.string("e621Login"), state.store.string("e621ApiKey")) {
        request = request.header("Authorization", format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!("{login}:{key}"))));
    }
    let response = request.send().await.map_err(net::describe)?;
    if !response.status().is_success() {
        return Err(format!("e621 answered {}", response.status().as_u16()));
    }
    serde_json::from_slice(&response.bytes().await.map_err(net::describe)?).map_err(|_| "e621 sent something that isn't JSON".to_string())
}

struct Job {
    url: String,
    path: PathBuf,
}

struct Batch {
    done: usize,
    failed: usize,
    stopped: bool, // the setting was turned off before every file was tried
}

async fn download_all(app: &AppHandle, jobs: Vec<Job>, label: String) -> Batch {
    let jobs = std::sync::Arc::new(jobs);
    let next = std::sync::Arc::new(AtomicUsize::new(0));
    let done = std::sync::Arc::new(AtomicUsize::new(0));
    let failed = std::sync::Arc::new(AtomicUsize::new(0));

    let workers: Vec<_> = (0..3)
        .map(|_| {
            let (app, jobs, next, done, failed, label) = (app.clone(), jobs.clone(), next.clone(), done.clone(), failed.clone(), label.clone());
            tauri::async_runtime::spawn(async move {
                let state = app.state::<App>();
                while next.load(Ordering::SeqCst) < jobs.len() && (state.store.flag("autoDownloadFavorites") || state.store.flag("autoDownloadPools")) && !state.store.flag("offlineMode") {
                    let n = next.fetch_add(1, Ordering::SeqCst);
                    let Some(job) = jobs.get(n) else { break };
                    match download_file(&state, &job.url, &job.path, |_, _| {}).await {
                        Ok(Outcome::Downloaded) => {
                            done.fetch_add(1, Ordering::SeqCst);
                        }
                        Ok(Outcome::Skipped) => {}
                        Err(_) => {
                            failed.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                    if (n + 1) % 20 == 0 {
                        set_status(&app, format!("{label}: {} / {}", n + 1, jobs.len()));
                    }
                }
            })
        })
        .collect();
    for worker in workers {
        let _ = worker.await;
    }
    Batch { done: done.load(Ordering::SeqCst), failed: failed.load(Ordering::SeqCst), stopped: next.load(Ordering::SeqCst) < jobs.len() }
}

async fn run(app: &AppHandle) {
    let state = app.state::<App>();
    if state.store.flag("offlineMode") {
        return set_status(app, "Offline mode is on, so nothing is downloaded.".into());
    }
    {
        let mut auto = state.downloads.auto.lock().unwrap();
        if auto.running {
            auto.again = true;
            return;
        }
        auto.running = true;
    }

    let mut new_files = 0;
    match sync(app, &mut new_files).await {
        Ok(()) => set_status(app, if new_files > 0 { format!("Downloaded {new_files} new files.") } else { "Everything is downloaded.".into() }),
        Err(e) => {
            set_status(app, format!("Stopped: {e}. Tries again later."));
            schedule(app, 10 * 60 * 1000);
        }
    }

    let again = {
        let mut auto = state.downloads.auto.lock().unwrap();
        auto.running = false;
        std::mem::take(&mut auto.again)
    };
    if new_files > 0 {
        let _ = app.emit("local-copies-changed", ());
    }
    if again {
        schedule(app, 1000);
    }
}

fn is_web(url: &Value) -> Option<&str> {
    url.as_str().filter(|u| net::web_url(u).is_some())
}

async fn sync(app: &AppHandle, new_files: &mut usize) -> Result<(), String> {
    let state = app.state::<App>();

    if state.store.flag("autoDownloadFavorites") {
        let dir = download_path(app, &state, "MSG/favorites")?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let items = state.store.get("personalListItems");
        let jobs: Vec<Job> = items
            .as_ref()
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(|item| is_web(&item["fileUrl"])).map(|url| Job { url: url.to_string(), path: dir.join(safe_file_name(url)) }).filter(|job| !job.path.exists()).collect())
            .unwrap_or_default();
        if !jobs.is_empty() {
            *new_files += download_all(app, jobs, "Favorites".into()).await.done;
        }
    }

    if state.store.flag("autoDownloadPools") {
        let pools = state.store.get("savedPools").and_then(|p| p.as_array().cloned()).unwrap_or_default();
        for pool in pools {
            if !state.store.flag("autoDownloadPools") {
                break;
            }
            let id = pool["id"].as_u64().unwrap_or(0);
            let finished = state.store.get("downloadedPools").and_then(|f| f[id.to_string()].as_f64());
            if finished.is_some_and(|done| done >= pool["count"].as_f64().unwrap_or(f64::MAX)) {
                continue;
            }

            let info = e621_json(app, &format!("/pools/{id}.json")).await?;
            let ids: Vec<u64> = info["post_ids"].as_array().map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default();
            let mut urls = std::collections::HashMap::new();
            for chunk in ids.chunks(100) {
                let list: Vec<String> = chunk.iter().map(u64::to_string).collect();
                let data = e621_json(app, &format!("/posts.json?limit=100&tags=id:{}", list.join(","))).await?;
                for post in data["posts"].as_array().into_iter().flatten() {
                    if let (Some(post_id), Some(url)) = (post["id"].as_u64(), post["file"]["url"].as_str()) {
                        urls.insert(post_id, url.to_string());
                    }
                }
            }

            let name = info["name"].as_str().unwrap_or("pool").replace('_', " ");
            let folder = clean_file_name(&format!("{name} ({})", info["id"].as_u64().unwrap_or(id)));
            let dir = download_path(app, &state, &format!("MSG/pools/{folder}"))?;
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let width = ids.len().to_string().len().max(3);
            let jobs: Vec<Job> = ids
                .iter()
                .enumerate()
                .filter_map(|(i, post_id)| urls.get(post_id).map(|url| (i + 1, url)))
                .map(|(page, url)| Job { url: url.clone(), path: dir.join(format!("{page:0>width$} {}", safe_file_name(url))) })
                .filter(|job| !job.path.exists())
                .collect();

            let batch = if jobs.is_empty() { Batch { done: 0, failed: 0, stopped: false } } else { download_all(app, jobs, name).await };
            *new_files += batch.done;
            // Done when every page that has a file is here; a pool that grows later is checked again.
            if batch.failed == 0 && !batch.stopped {
                let mut data = state.store.data.lock().unwrap();
                let mut finished = data.get("downloadedPools").and_then(|f| f.as_object().cloned()).unwrap_or_default();
                finished.insert(id.to_string(), json!(ids.len()));
                data.insert("downloadedPools".into(), Value::Object(finished));
                drop(data);
                state.store.save();
            }
        }
    }
    Ok(())
}
