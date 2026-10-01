// Hotkeys are saved as in the web version (settings.json → appHotkeys): action id → JS key codes, with
// 256 added for Ctrl, 512 for Shift and 1024 for Alt. (In the web version's own header 256 is Ctrl.)
use crate::store::Store;
use eframe::egui::{Event, Key, Modifiers};
use serde_json::Value;
use std::collections::HashMap;

pub const CTRL: u32 = 256;
pub const SHIFT: u32 = 512;
pub const ALT: u32 = 1024;

pub struct Action {
    pub id: &'static str,
    pub label: &'static str,
    pub defaults: &'static [u32],
}

pub const ACTIONS: [Action; 15] = [
    Action { id: "previous", label: "Previous", defaults: &[37, 65] },
    Action { id: "next", label: "Next", defaults: &[39, 68] },
    Action { id: "back10", label: "Back 10", defaults: &[87] },
    Action { id: "forward10", label: "Forward 10", defaults: &[83] },
    Action { id: "playPause", label: "Play / pause", defaults: &[32] },
    Action { id: "autoFit", label: "Toggle auto-fit", defaults: &[70] },
    Action { id: "openSource", label: "Open post in browser", defaults: &[69] },
    Action { id: "download", label: "Download the image", defaults: &[76] },
    Action { id: "favorite", label: "Fave / unfave", defaults: &[71] },
    Action { id: "toggleTags", label: "Show / hide tags", defaults: &[82] },
    Action { id: "openFavorites", label: "Favorites ⇄ slideshow", defaults: &[CTRL + 70] },
    Action { id: "openSettings", label: "Open / close settings", defaults: &[CTRL + 83] },
    Action { id: "setBackground", label: "Use the image as background", defaults: &[CTRL + 76] },
    Action { id: "showInterface", label: "Show / hide the controls", defaults: &[85] },
    Action { id: "addToAnalysis", label: "Add to the tag analysis", defaults: &[67] },
];

const KEYS: [(Key, u32); 62] = [
    (Key::A, 65), (Key::B, 66), (Key::C, 67), (Key::D, 68), (Key::E, 69), (Key::F, 70), (Key::G, 71), (Key::H, 72),
    (Key::I, 73), (Key::J, 74), (Key::K, 75), (Key::L, 76), (Key::M, 77), (Key::N, 78), (Key::O, 79), (Key::P, 80),
    (Key::Q, 81), (Key::R, 82), (Key::S, 83), (Key::T, 84), (Key::U, 85), (Key::V, 86), (Key::W, 87), (Key::X, 88),
    (Key::Y, 89), (Key::Z, 90),
    (Key::Num0, 48), (Key::Num1, 49), (Key::Num2, 50), (Key::Num3, 51), (Key::Num4, 52),
    (Key::Num5, 53), (Key::Num6, 54), (Key::Num7, 55), (Key::Num8, 56), (Key::Num9, 57),
    (Key::ArrowLeft, 37), (Key::ArrowUp, 38), (Key::ArrowRight, 39), (Key::ArrowDown, 40),
    (Key::Space, 32), (Key::Enter, 13), (Key::Tab, 9), (Key::Escape, 27),
    (Key::PageUp, 33), (Key::PageDown, 34), (Key::End, 35), (Key::Home, 36),
    (Key::F1, 112), (Key::F2, 113), (Key::F3, 114), (Key::F4, 115), (Key::F5, 116), (Key::F6, 117),
    (Key::F7, 118), (Key::F8, 119), (Key::F9, 120), (Key::F10, 121), (Key::F11, 122), (Key::F12, 123),
    (Key::Delete, 46), (Key::Insert, 45),
];

pub fn code_of(key: Key, m: Modifiers) -> Option<u32> {
    let base = KEYS.iter().find(|(k, _)| *k == key)?.1;
    Some(base + if m.ctrl || m.command { CTRL } else { 0 } + if m.shift { SHIFT } else { 0 } + if m.alt { ALT } else { 0 })
}

/// "Ctrl+F", "Left" ...
pub fn label(code: u32) -> String {
    let base = code & !(CTRL | SHIFT | ALT);
    let name = KEYS.iter().find(|(_, c)| *c == base).map(|(k, _)| k.name().to_string()).unwrap_or_else(|| format!("#{base}"));
    let mut text = String::new();
    for (bit, prefix) in [(CTRL, "Ctrl+"), (SHIFT, "Shift+"), (ALT, "Alt+")] {
        if code & bit != 0 {
            text += prefix;
        }
    }
    text + &name
}

pub struct Hotkeys {
    pub map: HashMap<String, Vec<u32>>,
}

impl Hotkeys {
    pub fn load(store: &Store) -> Hotkeys {
        let mut map: HashMap<String, Vec<u32>> = ACTIONS.iter().map(|a| (a.id.to_string(), a.defaults.to_vec())).collect();
        if let Some(Value::Object(saved)) = store.get("appHotkeys") {
            for (id, codes) in saved {
                if let (Some(slot), Value::Array(codes)) = (map.get_mut(&id), codes) {
                    *slot = codes.iter().filter_map(Value::as_u64).map(|c| c as u32).collect();
                }
            }
        }
        Hotkeys { map }
    }

    pub fn to_value(&self) -> Value {
        Value::Object(ACTIONS.iter().map(|a| (a.id.to_string(), Value::from(self.map[a.id].clone()))).collect())
    }

    /// The actions whose key went down in this frame's events.
    pub fn pressed(&self, events: &[Event], text_focus: bool) -> Vec<&'static str> {
        let mut out = vec![];
        for event in events {
            if let Event::Key { key, pressed: true, repeat: false, modifiers, .. } = event {
                let Some(code) = code_of(*key, *modifiers) else { continue };
                // Typing in a box: only keys with Ctrl/Alt count.
                if text_focus && code & (CTRL | ALT) == 0 {
                    continue;
                }
                for action in &ACTIONS {
                    if self.map[action.id].contains(&code) {
                        out.push(action.id);
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_labels() {
        assert_eq!(code_of(Key::F, Modifiers::CTRL), Some(CTRL + 70));
        assert_eq!(code_of(Key::ArrowLeft, Modifiers::NONE), Some(37));
        assert_eq!(label(CTRL + 70), "Ctrl+F");
        assert_eq!(label(37), Key::ArrowLeft.name());
    }
}
