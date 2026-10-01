// Downloads: one file (the button, L), many files (Download all favorites) and the background job that keeps
// offline copies of the favorites and the saved pools. Ported from src/downloads.rs.
use super::files::{clean_file_name, download_path, list_local_media, safe_file_name};
use super::net;
use super::session::{Engine, Msg};
use crate::store::Store;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{mpsc::Sender, Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;

#[derive(PartialEq)]
enum Outcome {
    Downloaded,
    Skipped,
}

/// Saves `url` to `path` (through `path.part`, so a stopped download never looks finished). Files already
/// there are skipped, so a rerun resumes.
async fn download_file(engine: &Engine, url: &str, path: &Path) -> Result<Outcome, String> {
    if std::fs::metadata(path).map(|m| m.len() > 0).unwrap_or(false) {
        return Ok(Outcome::Skipped);
    }
    let mut response = net::get(&engine.client, &engine.store, url, Duration::from_secs(600)).await?;
    let part = PathBuf::from(format!("{}.part", path.to_string_lossy()));
    let mut file = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    while let Some(chunk) = response.chunk().await.map_err(|_| "network error".to_string())? {
        file.write_all(&chunk).map_err(|e| e.to_string())?;
    }
    drop(file);
    std::fs::rename(&part, path).map_err(|e| e.to_string())
        .map(|_| Outcome::Downloaded)
}

#[derive(Default)]
struct Auto {
    running: bool,
    again: bool,
    // Bumped by every schedule(): a waiting timer that isn't the latest does nothing.
    timer: u64,
}

pub struct Downloader {
    pub engine: Engine,
    tx: Sender<Msg>,
    bulk_cancelled: Arc<AtomicBool>,
    auto: Arc<Mutex<Auto>>,
    timer: Arc<AtomicU64>,
}

impl Downloader {
    pub fn new(engine: Engine, tx: Sender<Msg>) -> Downloader {
        Downloader { engine, tx, bulk_cancelled: Arc::new(AtomicBool::new(false)), auto: Arc::default(), timer: Arc::default() }
    }

    fn say(engine: &Engine, tx: &Sender<Msg>, text: String) {
        let _ = tx.send(Msg::Notice(text));
        engine.ctx.request_repaint();
    }

    /// <download folder>/downloads/<name>, overwriting.
    pub fn download_one(&self, url: &str) {
        let name = safe_file_name(url);
        let target = match download_path(&self.engine.store, &format!("downloads/{name}")) {
            Ok(t) => t,
            Err(e) => return Self::say(&self.engine, &self.tx, e),
        };
        if let Some(dir) = target.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::remove_file(&target);
        let (engine, tx, url) = (self.engine.clone(), self.tx.clone(), url.to_string());
        self.engine.rt.spawn(async move {
            let text = match download_file(&engine, &url, &target).await {
                Ok(_) => format!("Saved {name}"),
                Err(_) => format!("Download failed: {name}"),
            };
            Self::say(&engine, &tx, text);
        });
    }

    pub fn cancel_bulk(&self) {
        self.bulk_cancelled.store(true, Ordering::SeqCst);
    }

    /// Download all favorites: 3 files at a time, into `folder` (like "MSG/favorites").
    pub fn download_many(&self, urls: Vec<String>, folder: &str) {
        self.bulk_cancelled.store(false, Ordering::SeqCst);
        let dir = match download_path(&self.engine.store, folder) {
            Ok(d) => d,
            Err(e) => return Self::say(&self.engine, &self.tx, e),
        };
        if let Err(e) = std::fs::create_dir_all(&dir) {
            return Self::say(&self.engine, &self.tx, e.to_string());
        }
        let total = urls.len();
        let (urls, next, counts) = (Arc::new(urls), Arc::new(AtomicUsize::new(0)), Arc::new([AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0)]));
        let (engine, tx, cancelled) = (self.engine.clone(), self.tx.clone(), self.bulk_cancelled.clone());
        self.engine.rt.spawn(async move {
            let workers: Vec<_> = (0..3)
                .map(|_| {
                    let (engine, tx, urls, next, counts, dir, cancelled) = (engine.clone(), tx.clone(), urls.clone(), next.clone(), counts.clone(), dir.clone(), cancelled.clone());
                    tokio::spawn(async move {
                        loop {
                            let i = next.fetch_add(1, Ordering::SeqCst);
                            if i >= urls.len() || cancelled.load(Ordering::SeqCst) {
                                break;
                            }
                            let slot = match download_file(&engine, &urls[i], &dir.join(safe_file_name(&urls[i]))).await {
                                Ok(Outcome::Downloaded) => 0,
                                Ok(Outcome::Skipped) => 1,
                                Err(_) => 2,
                            };
                            counts[slot].fetch_add(1, Ordering::SeqCst);
                            let done: usize = counts.iter().map(|c| c.load(Ordering::SeqCst)).sum();
                            if done % 10 == 0 {
                                Self::say(&engine, &tx, format!("Downloading favorites: {done} / {}", urls.len()));
                            }
                        }
                    })
                })
                .collect();
            for worker in workers {
                let _ = worker.await;
            }
            let c = |i: usize| counts[i].load(Ordering::SeqCst);
            let stopped = if cancelled.load(Ordering::SeqCst) { " (stopped)" } else { "" };
            Self::say(&engine, &tx, format!("Downloaded {} of {total}, {} already there, {} failed{stopped}. Folder: {}", c(0), c(1), c(2), dir.display()));
        });
    }

    /// The background job: offline copies of the favorites and saved pools. `delay` lets a burst of changes settle.
    pub fn schedule(&self, delay: Duration) {
        let timer = self.timer.fetch_add(1, Ordering::SeqCst) + 1;
        let (engine, tx, auto, latest) = (self.engine.clone(), self.tx.clone(), self.auto.clone(), self.timer.clone());
        let this = Downloader { engine: self.engine.clone(), tx: self.tx.clone(), bulk_cancelled: self.bulk_cancelled.clone(), auto: self.auto.clone(), timer: self.timer.clone() };
        self.engine.rt.spawn(async move {
            sleep(delay).await;
            if latest.load(Ordering::SeqCst) != timer {
                return;
            }
            let store = engine.store.clone();
            if store.flag("offlineMode") || !(store.flag("autoDownloadFavorites") || store.flag("autoDownloadPools")) {
                return;
            }
            {
                let mut a = auto.lock().unwrap();
                if a.running {
                    a.again = true;
                    return;
                }
                a.running = true;
            }
            let mut new_files = 0;
            let status = match sync(&engine, &tx, &mut new_files).await {
                Ok(()) if new_files > 0 => format!("Offline copies: downloaded {new_files} new files."),
                Ok(()) => "Offline copies: everything is downloaded.".into(),
                Err(e) => {
                    this.schedule(Duration::from_secs(600));
                    format!("Offline copies stopped: {e}. Tries again later.")
                }
            };
            let again = {
                let mut a = auto.lock().unwrap();
                a.running = false;
                std::mem::take(&mut a.again)
            };
            let _ = tx.send(Msg::Downloaded(status));
            engine.ctx.request_repaint();
            if again {
                this.schedule(Duration::from_secs(1));
            }
        });
    }
}

// e621's API from here: its rate limit wants a pause between requests.
async fn e621_json(engine: &Engine, path_and_query: &str) -> Result<Value, String> {
    sleep(Duration::from_millis(700)).await;
    let mut request = engine.client.get(format!("https://e621.net{path_and_query}")).timeout(Duration::from_secs(30));
    if let (Some(login), Some(key)) = (engine.store.string("e621Login"), engine.store.string("e621ApiKey")) {
        request = request.basic_auth(login, Some(key));
    }
    let response = request.send().await.map_err(|_| "e621 didn't answer".to_string())?;
    if !response.status().is_success() {
        return Err(format!("e621 answered {}", response.status().as_u16()));
    }
    serde_json::from_slice(&response.bytes().await.map_err(|_| "e621 didn't answer".to_string())?).map_err(|_| "e621 sent something that isn't JSON".to_string())
}

struct Job {
    url: String,
    path: PathBuf,
}

fn wanted(store: &Store) -> bool {
    (store.flag("autoDownloadFavorites") || store.flag("autoDownloadPools")) && !store.flag("offlineMode")
}

async fn download_all(engine: &Engine, tx: &Sender<Msg>, jobs: Vec<Job>, label: String) -> (usize, usize, bool) {
    let jobs = Arc::new(jobs);
    let (next, done, failed) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
    let workers: Vec<_> = (0..3)
        .map(|_| {
            let (engine, tx, jobs, next, done, failed, label) = (engine.clone(), tx.clone(), jobs.clone(), next.clone(), done.clone(), failed.clone(), label.clone());
            tokio::spawn(async move {
                while next.load(Ordering::SeqCst) < jobs.len() && wanted(&engine.store) {
                    let n = next.fetch_add(1, Ordering::SeqCst);
                    let Some(job) = jobs.get(n) else { break };
                    match download_file(&engine, &job.url, &job.path).await {
                        Ok(Outcome::Downloaded) => {
                            done.fetch_add(1, Ordering::SeqCst);
                        }
                        Ok(Outcome::Skipped) => {}
                        Err(_) => {
                            failed.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                    if (n + 1) % 20 == 0 {
                        Downloader::say(&engine, &tx, format!("{label}: {} / {}", n + 1, jobs.len()));
                    }
                }
            })
        })
        .collect();
    for worker in workers {
        let _ = worker.await;
    }
    // "stopped": the setting was turned off before every file was tried.
    (done.load(Ordering::SeqCst), failed.load(Ordering::SeqCst), next.load(Ordering::SeqCst) < jobs.len())
}

async fn sync(engine: &Engine, tx: &Sender<Msg>, new_files: &mut usize) -> Result<(), String> {
    let store = &engine.store;
    if store.flag("autoDownloadFavorites") {
        let dir = download_path(store, "MSG/favorites")?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let jobs: Vec<Job> = store
            .get("personalListItems")
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(|item| item["fileUrl"].as_str().filter(|u| net::web_url(u).is_some()).map(String::from))
            .map(|url| Job { path: dir.join(safe_file_name(&url)), url })
            .filter(|job| !job.path.exists())
            .collect();
        if !jobs.is_empty() {
            *new_files += download_all(engine, tx, jobs, "Favorites".into()).await.0;
        }
    }
    if store.flag("autoDownloadPools") {
        for pool in store.get("savedPools").and_then(|p| p.as_array().cloned()).unwrap_or_default() {
            if !store.flag("autoDownloadPools") {
                break;
            }
            let id = pool["id"].as_u64().unwrap_or(0);
            let finished = store.get("downloadedPools").and_then(|f| f[id.to_string()].as_f64());
            if finished.is_some_and(|done| done >= pool["count"].as_f64().unwrap_or(f64::MAX)) {
                continue;
            }
            let info = e621_json(engine, &format!("/pools/{id}.json")).await?;
            let ids: Vec<u64> = info["post_ids"].as_array().map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default();
            let mut urls = HashMap::new();
            for chunk in ids.chunks(100) {
                let list: Vec<String> = chunk.iter().map(u64::to_string).collect();
                let data = e621_json(engine, &format!("/posts.json?limit=100&tags=id:{}", list.join(","))).await?;
                for post in data["posts"].as_array().into_iter().flatten() {
                    if let (Some(post_id), Some(url)) = (post["id"].as_u64(), post["file"]["url"].as_str()) {
                        urls.insert(post_id, url.to_string());
                    }
                }
            }
            let name = info["name"].as_str().unwrap_or("pool").replace('_', " ");
            let folder = clean_file_name(&format!("{name} ({})", info["id"].as_u64().unwrap_or(id)));
            let dir = download_path(store, &format!("MSG/pools/{folder}"))?;
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let width = ids.len().to_string().len().max(3);
            let jobs: Vec<Job> = ids
                .iter()
                .enumerate()
                .filter_map(|(i, post_id)| urls.get(post_id).map(|url| (i + 1, url)))
                .map(|(page, url)| Job { url: url.clone(), path: dir.join(format!("{page:0>width$} {}", safe_file_name(url))) })
                .filter(|job| !job.path.exists())
                .collect();
            let (done, failed, stopped) = if jobs.is_empty() { (0, 0, false) } else { download_all(engine, tx, jobs, name).await };
            *new_files += done;
            // Done when every page that has a file is here; a pool that grows later is checked again.
            if failed == 0 && !stopped {
                let mut data = store.data.lock().unwrap();
                let mut finished = data.get("downloadedPools").and_then(|f| f.as_object().cloned()).unwrap_or_default();
                finished.insert(id.to_string(), json!(ids.len()));
                data.insert("downloadedPools".into(), Value::Object(finished));
                drop(data);
                store.save();
            }
        }
    }
    Ok(())
}

/// The downloaded copies by site file name (the page number in front of a pool file is dropped), so a
/// picture that is on the disk is shown from there.
pub fn local_copies(store: &Store) -> HashMap<String, String> {
    let folders: Vec<String> = ["MSG/favorites", "MSG/pools"].iter().filter_map(|f| download_path(store, f).ok()).filter(|d| d.exists()).map(|d| d.to_string_lossy().into_owned()).collect();
    list_local_media(&folders)
        .into_iter()
        .map(|f| {
            let name = Path::new(&f.abs).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let plain = match name.split_once(' ') {
                Some((number, rest)) if !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()) => rest.to_string(),
                _ => name,
            };
            (plain, f.abs)
        })
        .collect()
}

/// The file that stands in for `url` when it is downloaded, else the address itself.
pub fn display_url(copies: &HashMap<String, String>, url: &str) -> String {
    if copies.is_empty() || net::web_url(url).is_none() {
        return url.to_string();
    }
    let name = url.split(['?', '#']).next().unwrap_or("").rsplit('/').next().unwrap_or("");
    let name = urlencoding::decode(name).map(|n| n.into_owned()).unwrap_or_else(|_| name.to_string());
    copies.get(&name).cloned().unwrap_or_else(|| url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downloaded_copy_replaces_url() {
        let copies: HashMap<String, String> = [("a b.jpg".to_string(), "C:/x/a b.jpg".to_string())].into();
        assert_eq!(display_url(&copies, "https://s/data/a%20b.jpg?x=1"), "C:/x/a b.jpg");
        assert_eq!(display_url(&copies, "https://s/other.jpg"), "https://s/other.jpg");
        assert_eq!(display_url(&HashMap::new(), "https://s/a.jpg"), "https://s/a.jpg");
    }
}
