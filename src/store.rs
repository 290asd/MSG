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
}

impl Store {
    /// `dir` is the user data folder (<config>/MSG).
    /// `legacy_dir` is the folder used before the rename to MSG; its settings.json is copied over on the first start.
    pub fn load(dir: PathBuf, legacy_dir: PathBuf) -> Arc<Store> {
        let file = dir.join("settings.json");
        let (data, fresh) = match read_json(&file) {
            Some(data) => (data, false),
            None => (read_json(&legacy_dir.join("settings.json")).unwrap_or_default(), true),
        };
        let store = Arc::new(Store { data: Mutex::new(data), file, pending: AtomicBool::new(false) });
        if fresh {
            store.save();
        }
        store
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
        if std::fs::write(&tmp, json).is_ok() {
            if let Err(e) = std::fs::rename(&tmp, &self.file) {
                eprintln!("Saving the settings failed: {e}");
            }
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

fn read_json(file: &std::path::Path) -> Option<Map<String, Value>> {
    match serde_json::from_slice(&std::fs::read(file).ok()?).ok()? {
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
}
