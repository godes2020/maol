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
    /// Accelerators Windows refused; retried now and then, so they start
    /// working once whoever holds them lets go.
    pending: Vec<(String, String)>,
    pub failed: Vec<String>,
    /// Set when the refusal was not "taken by someone else".
    pub error: Option<String>,
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
        Self {
            manager: GlobalHotKeyManager::new().ok(),
            registered: Vec::new(),
            ids,
            pending: Vec::new(),
            failed: Vec::new(),
            error: None,
        }
    }

    pub fn register(&mut self, list: &[(String, String)]) {
        self.unregister();
        self.pending = list.iter().filter(|(_, accel)| !accel.is_empty()).cloned().collect();
        self.retry();
    }

    /// Tries the accelerators that are not registered yet. Returns true when
    /// the set of failed ones changed.
    pub fn retry(&mut self) -> bool {
        let Some(m) = &self.manager else { return false };
        let before = std::mem::take(&mut self.failed);
        let mut ids = self.ids.lock().unwrap();
        let mut left = Vec::new();
        self.error = None;
        for (action, accel) in std::mem::take(&mut self.pending) {
            let result = match parse(&accel) {
                Some(hk) => m.register(hk).map(|_| hk),
                None => Err(global_hotkey::Error::FailedToRegister(accel.clone())),
            };
            match result {
                Ok(hk) => {
                    ids.insert(hk.id(), action);
                    self.registered.push(hk);
                }
                Err(e) => {
                    if !matches!(e, global_hotkey::Error::AlreadyRegistered(_)) {
                        self.error.get_or_insert_with(|| e.to_string());
                    }
                    self.failed.push(accel.clone());
                    left.push((action, accel));
                }
            }
        }
        self.pending = left;
        before != self.failed
    }

    pub fn unregister(&mut self) {
        if let Some(m) = &self.manager {
            let _ = m.unregister_all(&self.registered);
        }
        self.registered.clear();
        self.ids.lock().unwrap().clear();
        self.pending.clear();
        self.failed.clear();
        self.error = None;
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
