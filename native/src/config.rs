// Settings, stored as JSON next to the other per-user app data. The format
// matches the Electron versions of Moa, so existing settings carry over.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Hotkey actions in the order the Keys tab lists them.
pub const ACTIONS: &[(&str, &str)] = &[
    ("playPause", "Alt+Shift+K"),
    ("next", "Alt+Shift+Right"),
    ("previous", "Alt+Shift+Left"),
    ("seekForward", "Alt+Shift+L"),
    ("seekBackward", "Alt+Shift+J"),
    ("volumeUp", "Alt+Shift+Up"),
    ("volumeDown", "Alt+Shift+Down"),
    ("toggleMute", "Alt+Shift+0"),
    ("cycleRepeat", "Alt+Shift+R"),
    ("toggleOverlay", "Alt+Shift+O"),
    ("editOverlay", "Alt+Shift+M"),
    ("toggleLock", "Alt+Shift+U"),
    ("showApp", "Alt+Shift+Y"),
    ("openPlayer", ""),
];

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Overlay {
    pub visible: bool,
    /// "card" or "cover".
    pub layout: String,
    /// "auto", "left" or "right": which side the cover sits on in the card.
    pub side: String,
    /// "locked": click-through, buttons only in move mode. "hover": opens on hover.
    pub interaction: String,
    pub always_expanded: bool,
    pub hide_when_idle: bool,
    pub opacity: f32,
    pub scale: f32,
    pub reactive: bool,
    pub intensity: f32,
    pub x: Option<i32>,
    pub y: Option<i32>,
}

impl Default for Overlay {
    fn default() -> Self {
        Self {
            visible: true,
            layout: "card".into(),
            side: "auto".into(),
            interaction: "locked".into(),
            always_expanded: false,
            hide_when_idle: false,
            opacity: 1.0,
            scale: 1.0,
            reactive: true,
            intensity: 1.0,
            x: None,
            y: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Playback {
    pub seek_step: f64,
    pub volume_step: f64,
}

impl Default for Playback {
    fn default() -> Self {
        Self { seek_step: 5.0, volume_step: 5.0 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct General {
    /// "system": Windows media controls (any player). "browser": the Moa
    /// Connector extension. "player": the built-in player window.
    pub source: String,
    /// Service in the built-in player window.
    pub service: String,
    pub onboarded: bool,
    pub language: String,
    pub autostart: bool,
    pub close_to_tray: bool,
    /// Own key: Moa 0.3 kept "gpu" on by default, and carrying that over
    /// would start the native app on the heavier GPU renderer.
    #[serde(rename = "gpuNative")]
    pub gpu: bool,
}

impl Default for General {
    fn default() -> Self {
        Self {
            source: "system".into(),
            service: "yandex".into(),
            onboarded: false,
            language: "auto".into(),
            autostart: false,
            close_to_tray: true,
            gpu: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Spotify {
    pub client_id: String,
    pub refresh_token: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Config {
    pub hotkeys: serde_json::Map<String, serde_json::Value>,
    pub overlay: Overlay,
    pub playback: Playback,
    pub general: General,
    pub spotify: Spotify,
}

impl Default for Config {
    fn default() -> Self {
        let mut hotkeys = serde_json::Map::new();
        for (a, k) in ACTIONS {
            hotkeys.insert((*a).into(), (*k).into());
        }
        Self {
            hotkeys,
            overlay: Overlay::default(),
            playback: Playback::default(),
            general: General::default(),
            spotify: Spotify::default(),
        }
    }
}

impl Config {
    pub fn dir() -> PathBuf {
        #[cfg(windows)]
        let base = std::env::var_os("APPDATA").map(PathBuf::from);
        #[cfg(not(windows))]
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")));
        base.unwrap_or_else(std::env::temp_dir).join("Moa")
    }

    pub fn file() -> PathBuf {
        std::env::var_os("MOA_CONFIG").map(PathBuf::from).unwrap_or_else(|| Self::dir().join("config.json"))
    }

    pub fn load() -> Self {
        let mut c: Config = std::fs::read_to_string(Self::file())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        // Keep every known action, drop unknown ones (renamed in older versions).
        let mut keys = serde_json::Map::new();
        for (a, default) in ACTIONS {
            let v = c.hotkeys.get(*a).and_then(|v| v.as_str()).unwrap_or(default);
            keys.insert((*a).into(), v.into());
        }
        c.hotkeys = keys;
        // Older versions knew "browser" as the default and "app" for the built-in window.
        if c.general.source == "app" {
            c.general.source = "player".into();
        }
        if !matches!(c.general.source.as_str(), "system" | "browser" | "player") {
            c.general.source = "system".into();
        }
        c.overlay.scale = c.overlay.scale.clamp(0.6, 2.5);
        c
    }

    pub fn save(&self) {
        let file = Self::file();
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(file, s);
        }
    }

    pub fn hotkey(&self, action: &str) -> String {
        self.hotkeys.get(action).and_then(|v| v.as_str()).unwrap_or("").to_string()
    }
}
