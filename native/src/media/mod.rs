// Music sources. Each runs on its own thread and reports through a Sink;
// commands come back with an optional result.
use serde::Deserialize;
use std::sync::Arc;

pub mod analyzer;
#[cfg(not(windows))]
pub mod mock;
#[cfg(windows)]
pub mod system_win;

#[derive(Deserialize, Clone, Default, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct TrackState {
    pub has_track: bool,
    pub playing: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    /// Player id (Windows AppUserModelID), when known.
    pub app: String,
    /// Cover: an id from the source ("art3"), an https URL or a data: URL.
    pub artwork: Option<String>,
    pub duration: f64,
    pub current_time: f64,
    pub can_seek: Option<bool>,
    /// "NONE", "ALL", "ONE", or None when the player has no repeat control.
    pub repeat: Option<String>,
    pub volume: Option<f64>,
    pub muted: bool,
}

#[derive(Deserialize, Clone, Default, Debug)]
#[serde(default)]
pub struct AudioFrame {
    pub energy: f32,
    pub bass: f32,
    pub beat: bool,
    pub bands: Vec<f32>,
    pub bpm: f32,
}

pub enum Event {
    State(TrackState),
    Art { key: String, bytes: Option<Vec<u8>> },
    Audio(AudioFrame),
    Status { ready: bool, error: Option<String> },
}

pub type Sink = Arc<dyn Fn(Event) + Send + Sync>;
pub type Reply = Box<dyn FnOnce(Option<serde_json::Value>) + Send>;

pub trait Backend {
    fn cmd(&self, name: &str, arg: f64, reply: Reply);
    /// Beat analysis of the sound output, only while needed.
    fn set_audio(&self, on: bool);
    /// Human-readable report for troubleshooting.
    fn diagnostics(&self) -> String;
}

/// The "Any player" source for this platform.
pub fn system(sink: Sink) -> Box<dyn Backend> {
    #[cfg(windows)]
    return Box::new(system_win::System::start(sink));
    #[cfg(not(windows))]
    return Box::new(mock::Mock::start(sink));
}
