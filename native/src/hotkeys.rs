// Global hotkeys (RegisterHotKey on Windows). Accelerators use the same
// format as before: "Alt+Shift+K", "Ctrl+Alt+Up", "num5"…
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

pub struct Hotkeys {
    manager: Option<GlobalHotKeyManager>,
    registered: Vec<HotKey>,
    ids: Arc<Mutex<HashMap<u32, String>>>,
    pub failed: Vec<String>,
}

impl Hotkeys {
    /// `on_action` runs on the hotkey thread; forward it to the UI thread.
    pub fn new(on_action: impl Fn(String) + Send + Sync + 'static) -> Self {
        let ids: Arc<Mutex<HashMap<u32, String>>> = Default::default();
        let map = ids.clone();
        GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
            if e.state == HotKeyState::Pressed {
                if let Some(a) = map.lock().unwrap().get(&e.id).cloned() {
                    on_action(a);
                }
            }
        }));
        Self { manager: GlobalHotKeyManager::new().ok(), registered: Vec::new(), ids, failed: Vec::new() }
    }

    pub fn register(&mut self, list: &[(String, String)]) {
        self.unregister();
        let Some(m) = &self.manager else { return };
        let mut ids = self.ids.lock().unwrap();
        for (action, accel) in list {
            if accel.is_empty() {
                continue;
            }
            match parse(accel) {
                Some(hk) if m.register(hk).is_ok() => {
                    ids.insert(hk.id(), action.clone());
                    self.registered.push(hk);
                }
                _ => self.failed.push(accel.clone()),
            }
        }
    }

    pub fn unregister(&mut self) {
        if let Some(m) = &self.manager {
            let _ = m.unregister_all(&self.registered);
        }
        self.registered.clear();
        self.ids.lock().unwrap().clear();
        self.failed.clear();
    }
}

pub fn parse(accel: &str) -> Option<HotKey> {
    let parts: Vec<String> = accel
        .split('+')
        .map(|p| match p.to_ascii_lowercase().as_str() {
            "ctrl" | "control" | "cmdorctrl" => "control".to_string(),
            "super" | "win" | "meta" => "super".to_string(),
            "numadd" => "NumpadAdd".to_string(),
            "numsub" => "NumpadSubtract".to_string(),
            "nummult" => "NumpadMultiply".to_string(),
            "numdiv" => "NumpadDivide".to_string(),
            "numdec" => "NumpadDecimal".to_string(),
            other => other.to_string(),
        })
        .collect();
    HotKey::from_str(&parts.join("+")).ok()
}
