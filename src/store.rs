// Everything chrome.storage.sync/local held in the extension lives in one JSON file (settings.json).
// The file holds the favorites too (several MB), so writes are grouped: at most one every 300 ms, off the
// main thread, to a temporary file first so a crash mid-write can't corrupt it.
use serde_json::{Map, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct Store {
    pub data: Mutex<Map<String, Value>>,
    file: PathBuf,
    pending: AtomicBool,
    /// No settings.json of MSG's own was found at start (the onboarding page is shown).
    pub first_run: bool,
    /// On the first run, the old BooruSlideshow settings were found and copied over.
    pub imported: bool,
    /// settings.json couldn't be read, so it was moved here and MSG started empty.
    pub broken: Option<PathBuf>,
    /// Held while the program runs; Windows releases it when the process ends, even after a crash.
    _lock: std::fs::File,
}

impl Store {
    /// `dir` is the user data folder (<config>/MSG).
    /// `legacy_dir` is the folder used before the rename to MSG; its settings.json is copied over on the first start.
    /// Fails when another MSG or msg-cli already uses the folder (each writes its whole copy, so the
    /// other's changes would be lost), or when settings.json can't be read and can't be put aside.
    pub fn load(dir: PathBuf, legacy_dir: PathBuf) -> Result<Arc<Store>, String> {
        let _ = std::fs::create_dir_all(&dir);
        let lock = std::fs::File::create(dir.join("settings.lock")).map_err(|e| format!("{} can't be opened ({e}).", dir.display()))?;
        if lock.try_lock().is_err() {
            return Err("MSG or msg-cli is already running. Only one can use the settings at a time.".into());
        }
        let file = dir.join("settings.json");
        let mut broken = None;
        let (data, fresh, imported) = match std::fs::read(&file) {
            Ok(bytes) => match parse(&bytes) {
                Some(data) => (data, false, false),
                // A file that can't be read is never written over: it is put aside and MSG starts empty.
                None => {
                    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
                    let aside = dir.join(format!("settings.broken-{secs}.json"));
                    if let Err(e) = std::fs::rename(&file, &aside) {
                        return Err(format!("{} can't be read and couldn't be put aside ({e}). Fix or move it by hand.", file.display()));
                    }
                    broken = Some(aside);
                    (Map::new(), true, false)
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                match std::fs::read(legacy_dir.join("settings.json")).ok().and_then(|b| parse(&b)) {
                    Some(data) => (data, true, true),
                    None => (Map::new(), true, false),
                }
            }
            Err(e) => return Err(format!("{} can't be read ({e}).", file.display())),
        };
        let store = Arc::new(Store { data: Mutex::new(data), file, pending: AtomicBool::new(false), first_run: fresh, imported, broken, _lock: lock });
        if fresh {
            store.save();
        }
        Ok(store)
    }

    pub fn save(self: &Arc<Self>) {
        if self.pending.swap(true, Ordering::SeqCst) {
            return;
        }
        let store = Arc::clone(self);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            store.flush();
        });
    }

    /// Writes right away if a write is waiting; also called on quit.
    pub fn flush(&self) {
        if !self.pending.swap(false, Ordering::SeqCst) {
            return;
        }
        let json = serde_json::to_vec(&*self.data.lock().unwrap()).unwrap_or_default();
        if let Some(dir) = self.file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = self.file.with_extension("json.tmp");
        // sync_all before the rename, or a power cut can leave an empty settings.json behind.
        let written = std::fs::File::create(&tmp).and_then(|mut f| {
            std::io::Write::write_all(&mut f, &json)?;
            f.sync_all()
        });
        if let Err(e) = written.and_then(|_| std::fs::rename(&tmp, &self.file)) {
            eprintln!("Saving the settings failed: {e}");
        }
    }

    pub fn get(&self, key: &str) -> Option<Value> {
        self.data.lock().unwrap().get(key).cloned()
    }

    pub fn flag(&self, key: &str) -> bool {
        matches!(self.get(key), Some(Value::Bool(true)))
    }

    pub fn string(&self, key: &str) -> Option<String> {
        match self.get(key) {
            Some(Value::String(s)) if !s.is_empty() => Some(s),
            _ => None,
        }
    }
}

fn parse(bytes: &[u8]) -> Option<Map<String, Value>> {
    match serde_json::from_slice(bytes).ok()? {
        Value::Object(map) => Some(map),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    // The pages iterate some saved objects in order, so a save must not sort the keys.
    #[test]
    fn keeps_key_order() {
        let text = r#"{"b":1,"a":{"z":1,"y":2}}"#;
        let value: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(serde_json::to_string(&value).unwrap(), text);
    }

    // A settings.json that doesn't parse is put aside, never written over.
    #[test]
    fn keeps_a_broken_file() {
        let dir = std::env::temp_dir().join("msg-native-broken");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("settings.json"), b"{\"favorites\": [1, 2").unwrap();
        let store = super::Store::load(dir.clone(), dir.join("none")).unwrap();
        assert!(super::Store::load(dir.clone(), dir.join("none")).is_err(), "a second instance is refused");
        let aside = store.broken.clone().expect("put aside");
        assert_eq!(std::fs::read(&aside).unwrap(), b"{\"favorites\": [1, 2");
        assert!(store.first_run);
        store.flush();
        assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), b"{}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
