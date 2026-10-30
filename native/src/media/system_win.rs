// "Any player" on Windows: the system media controls (the media flyout every
// player publishes to), the player's own volume in the Windows mixer, and
// beat analysis of the sound output through WASAPI loopback.
use super::analyzer::{Analyzer, N};
use super::{AudioFrame, Backend, Event, Reply, Sink, TrackState};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use windows::core::{Interface, HSTRING, PWSTR};
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session, GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use windows::Media::MediaPlaybackAutoRepeatMode as Repeat;
use windows::Storage::Streams::{DataReader, IRandomAccessStreamReference};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
use windows_future::IAsyncOperation;

enum Msg {
    Cmd(String, f64, Reply),
    Diag(std::sync::mpsc::Sender<String>),
}

pub struct System {
    tx: Sender<Msg>,
    audio: Arc<AtomicBool>,
}

impl System {
    pub fn start(sink: Sink) -> Self {
        let (tx, rx) = channel::<Msg>();
        let audio = Arc::new(AtomicBool::new(false));
        let s = sink.clone();
        std::thread::Builder::new()
            .name("moa-media".into())
            .spawn(move || {
                unsafe {
                    let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                }
                let manager = match Manager::RequestAsync().and_then(|op| op.join()) {
                    Ok(m) => m,
                    Err(e) => {
                        s(Event::Status { ready: false, error: Some(e.message().to_string()) });
                        return;
                    }
                };
                s(Event::Status { ready: true, error: None });
                let mut w = Worker::new(manager, s);
                loop {
                    w.poll();
                    let deadline = Instant::now() + Duration::from_millis(400);
                    loop {
                        let left = deadline.saturating_duration_since(Instant::now());
                        match rx.recv_timeout(left) {
                            Ok(Msg::Cmd(name, arg, reply)) => {
                                let v = w.run(&name, arg);
                                reply(v);
                            }
                            Ok(Msg::Diag(back)) => {
                                let _ = back.send(w.diagnostics());
                            }
                            Err(RecvTimeoutError::Timeout) => break,
                            Err(RecvTimeoutError::Disconnected) => return,
                        }
                    }
                }
            })
            .expect("media thread");

        let (a, s) = (audio.clone(), sink);
        std::thread::Builder::new()
            .name("moa-audio".into())
            .spawn(move || loopback(a, s))
            .expect("audio thread");
        Self { tx, audio }
    }
}

impl Backend for System {
    fn cmd(&self, name: &str, arg: f64, reply: Reply) {
        let _ = self.tx.send(Msg::Cmd(name.into(), arg, reply));
    }

    fn set_audio(&self, on: bool) {
        self.audio.store(on, Ordering::Relaxed);
    }

    fn diagnostics(&self) -> String {
        let (tx, rx) = channel();
        let _ = self.tx.send(Msg::Diag(tx));
        rx.recv_timeout(Duration::from_secs(3)).unwrap_or_else(|_| "media thread is not answering".into())
    }
}

// ---------------------------------------------------------------------------
// Reading the player

struct Worker {
    manager: Manager,
    sink: Sink,
    current: Option<String>,
    track_key: String,
    art_key: Option<String>,
    art_hash: u64,
    art_polls: u32,
    art_seq: u32,
    last_stable: String,
    last_sent: Instant,
    volume: VolumeCache,
}

impl Worker {
    fn new(manager: Manager, sink: Sink) -> Self {
        Self {
            manager,
            sink,
            current: None,
            track_key: String::new(),
            art_key: None,
            art_hash: 0,
            art_polls: 0,
            art_seq: 0,
            last_stable: String::new(),
            last_sent: Instant::now(),
            volume: VolumeCache::default(),
        }
    }

    fn sessions(&self) -> Vec<Session> {
        self.manager.GetSessions().map(|v| v.into_iter().collect()).unwrap_or_default()
    }

    /// Prefers a playing player, sticking with the one already followed.
    fn pick(&mut self) -> Option<Session> {
        let sessions = self.sessions();
        let id = |s: &Session| s.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default();
        let mut playing = None;
        for s in &sessions {
            if !is_playing(s) {
                continue;
            }
            if Some(id(s)) == self.current {
                return Some(s.clone());
            }
            playing.get_or_insert_with(|| s.clone());
        }
        let pick = playing
            .or_else(|| sessions.iter().find(|s| Some(id(s)) == self.current).cloned())
            .or_else(|| self.manager.GetCurrentSession().ok())
            .or_else(|| sessions.first().cloned());
        self.current = pick.as_ref().map(id);
        pick
    }

    fn poll(&mut self) {
        let Some(session) = self.pick() else {
            self.track_key.clear();
            self.art_key = None;
            self.send(TrackState::default(), false);
            return;
        };
        let app = session.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default();
        let props = session.TryGetMediaPropertiesAsync().and_then(|op| op.join()).ok();
        let info = session.GetPlaybackInfo().ok();
        let playing = info.as_ref().and_then(|i| i.PlaybackStatus().ok()) == Some(Status::Playing);
        let title = props.as_ref().and_then(|p| p.Title().ok()).map(|h| h.to_string()).unwrap_or_default();
        let mut artist = props.as_ref().and_then(|p| p.Artist().ok()).map(|h| h.to_string()).unwrap_or_default();
        if artist.is_empty() {
            artist = props.as_ref().and_then(|p| p.AlbumArtist().ok()).map(|h| h.to_string()).unwrap_or_default();
        }
        let album = props.as_ref().and_then(|p| p.AlbumTitle().ok()).map(|h| h.to_string()).unwrap_or_default();
        let (duration, position) = timeline(&session, playing);

        let key = format!("{app}\u{1}{title}\u{1}{artist}\u{1}{album}");
        if key != self.track_key {
            self.track_key = key;
            self.art_polls = 0;
        }
        // Players publish the new cover a moment after the new title, so the
        // first read can still return the previous one. Re-read it often for a
        // few seconds after a change, then now and then, and send it whenever
        // the picture actually changes.
        self.art_polls += 1;
        if let Some(p) = &props {
            if self.art_polls <= 15 || self.art_polls % 10 == 0 {
                let bytes = p.Thumbnail().ok().and_then(|t| read_thumbnail(&t));
                let hash = bytes.as_ref().map(|b| hash(b)).unwrap_or(0);
                if (bytes.is_some() || self.art_key.is_none()) && (hash != self.art_hash || self.art_key.is_none()) {
                    self.art_hash = hash;
                    self.art_seq += 1;
                    let k = format!("art{}", self.art_seq);
                    self.art_key = Some(k.clone());
                    (self.sink)(Event::Art { key: k, bytes });
                }
            }
        }

        let controls = info.as_ref().and_then(|i| i.Controls().ok());
        let repeat = match (&info, &controls) {
            (Some(i), Some(c)) if c.IsRepeatEnabled().unwrap_or(false) => i
                .AutoRepeatMode()
                .ok()
                .and_then(|r| r.Value().ok())
                .map(|r| match r {
                    Repeat::Track => "ONE",
                    Repeat::List => "ALL",
                    _ => "NONE",
                })
                .map(String::from),
            _ => None,
        };
        let vol = self.volume.read(&app);
        let state = TrackState {
            has_track: !title.is_empty(),
            playing,
            title,
            artist,
            album,
            app,
            artwork: self.art_key.clone(),
            duration,
            current_time: position,
            can_seek: Some(controls.as_ref().map(|c| c.IsPlaybackPositionEnabled().unwrap_or(false)).unwrap_or(false)),
            repeat,
            volume: vol.map(|v| (v.0 * 100.0).round() as f64),
            muted: vol.map(|v| v.1).unwrap_or(false),
        };
        self.send(state, playing);
    }

    // Sends on any change, and once a second while playing so progress stays in sync.
    fn send(&mut self, state: TrackState, playing: bool) {
        let stable = format!("{:?}", TrackState { current_time: 0.0, ..state.clone() });
        if stable == self.last_stable && (!playing || self.last_sent.elapsed() < Duration::from_millis(1000)) {
            return;
        }
        self.last_stable = stable;
        self.last_sent = Instant::now();
        (self.sink)(Event::State(state));
    }

    fn run(&mut self, name: &str, arg: f64) -> Option<Value> {
        let session = self.pick();
        let app = session.as_ref().and_then(|s| s.SourceAppUserModelId().ok()).map(|h| h.to_string()).unwrap_or_default();
        let r = match name {
            "volumeBy" => self.volume.change(&app, Some(arg / 100.0), None).map(|v| json!(v)),
            "setVolume" => self.volume.change(&app, None, Some(arg / 100.0)).map(|v| json!(v)),
            "toggleMute" => self.volume.toggle_mute(&app).map(|m| json!(m)),
            _ => {
                let s = session?;
                let ok = |op: windows::core::Result<IAsyncOperation<bool>>| op.and_then(|o| o.join()).unwrap_or(false);
                match name {
                    "playPause" => Some(json!(ok(s.TryTogglePlayPauseAsync()))),
                    "next" => Some(json!(ok(s.TrySkipNextAsync()))),
                    "previous" => Some(json!(ok(s.TrySkipPreviousAsync()))),
                    "seekTo" => Some(json!(seek(&s, arg))),
                    "seekBy" => {
                        let playing = is_playing(&s);
                        Some(json!(seek(&s, timeline(&s, playing).1 + arg)))
                    }
                    "cycleRepeat" => {
                        let info = s.GetPlaybackInfo().ok()?;
                        if !info.Controls().and_then(|c| c.IsRepeatEnabled()).unwrap_or(false) {
                            return Some(json!("unsupported"));
                        }
                        let mode = info.AutoRepeatMode().ok().and_then(|r| r.Value().ok()).unwrap_or(Repeat::None);
                        let next = match mode {
                            Repeat::None => Repeat::List,
                            Repeat::List => Repeat::Track,
                            _ => Repeat::None,
                        };
                        if !ok(s.TryChangeAutoRepeatModeAsync(next)) {
                            return Some(json!("unsupported"));
                        }
                        Some(json!(match next {
                            Repeat::Track => "ONE",
                            Repeat::List => "ALL",
                            _ => "NONE",
                        }))
                    }
                    _ => None,
                }
            }
        };
        // Reflect the change right away.
        self.last_stable.clear();
        r
    }

    fn diagnostics(&mut self) -> String {
        let mut out = String::from("Media sessions:\n");
        for s in self.sessions() {
            let id = s.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default();
            let title = s
                .TryGetMediaPropertiesAsync()
                .and_then(|op| op.join())
                .and_then(|p| p.Title())
                .map(|h| h.to_string())
                .unwrap_or_default();
            out += &format!("  - {id}  playing={}  title={title:?}\n", is_playing(&s));
        }
        out += &format!("Followed: {:?}\n\nVolume mixer sessions:\n", self.current);
        for s in mixer_sessions() {
            out += &format!("  - pid={} process={:?} active={}\n", s.pid, s.name, s.active);
        }
        if let Some(app) = &self.current {
            let picked = pick_volume_sessions(app);
            out += &format!("\nVolume target for {app:?}: {:?}\n", picked.iter().map(|s| &s.name).collect::<Vec<_>>());
        }
        out
    }
}

fn is_playing(s: &Session) -> bool {
    s.GetPlaybackInfo().and_then(|i| i.PlaybackStatus()).ok() == Some(Status::Playing)
}

/// (duration, position) in seconds; the snapshot is moved forward while playing.
fn timeline(s: &Session, playing: bool) -> (f64, f64) {
    let Ok(tl) = s.GetTimelineProperties() else { return (0.0, 0.0) };
    let start = tl.StartTime().map(|t| t.Duration).unwrap_or(0);
    let end = tl.EndTime().map(|t| t.Duration).unwrap_or(0);
    let pos = tl.Position().map(|t| t.Duration).unwrap_or(0);
    let duration = (end - start) as f64 / 1e7;
    let mut position = (pos - start) as f64 / 1e7;
    if playing {
        if let Ok(updated) = tl.LastUpdatedTime() {
            let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as i64 / 100).unwrap_or(0) + 116_444_736_000_000_000;
            if updated.UniversalTime > 0 && now > updated.UniversalTime {
                position += (now - updated.UniversalTime) as f64 / 1e7;
            }
        }
    }
    if duration > 0.0 {
        (duration, position.clamp(0.0, duration))
    } else {
        (0.0, 0.0)
    }
}

fn seek(s: &Session, seconds: f64) -> bool {
    let Ok(tl) = s.GetTimelineProperties() else { return false };
    let start = tl.StartTime().map(|t| t.Duration).unwrap_or(0);
    let end = tl.EndTime().map(|t| t.Duration).unwrap_or(0);
    let mut target = start + (seconds.max(0.0) * 1e7) as i64;
    if end > start {
        target = target.min(end);
    }
    s.TryChangePlaybackPositionAsync(target).and_then(|o| o.join()).unwrap_or(false)
}

fn read_thumbnail(thumb: &IRandomAccessStreamReference) -> Option<Vec<u8>> {
    let stream = thumb.OpenReadAsync().ok()?.join().ok()?;
    let size = stream.Size().ok()?;
    if size == 0 || size > 4_000_000 {
        return None;
    }
    let input = stream.GetInputStreamAt(0).ok()?;
    let reader = DataReader::CreateDataReader(&input).ok()?;
    let op: IAsyncOperation<u32> = reader.LoadAsync(size as u32).ok()?.cast().ok()?;
    op.join().ok()?;
    let mut buf = vec![0u8; size as usize];
    reader.ReadBytes(&mut buf).ok()?;
    Some(buf)
}

fn hash(b: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    b.hash(&mut h);
    h.finish()
}

// ---------------------------------------------------------------------------
// Player volume in the Windows mixer

/// Processes that play music and publish media controls.
const PLAYERS: &[&str] = &[
    "chrome", "msedge", "firefox", "zen", "opera", "opera_gx", "brave", "browser", "vivaldi", "librewolf", "floorp",
    "waterfox", "arc", "chromium", "thorium", "yandex", "spotify", "yandexmusic", "yandex music", "яндекс музыка",
    "soundcloud", "deezer", "tidal", "applemusic", "amazon music", "vk", "vk music", "vkmusic", "foobar2000", "aimp",
    "musicbee", "winamp", "vlc", "itunes", "wmplayer", "music.ui", "microsoft.media.player",
];

struct MixerSession {
    pid: u32,
    name: String,
    active: bool,
    volume: ISimpleAudioVolume,
}

fn mixer_sessions() -> Vec<MixerSession> {
    let mut out = Vec::new();
    unsafe {
        let Ok(enumerator) = CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL) else { return out };
        // All active output devices: the player may not use the default one.
        let Ok(devices) = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) else { return out };
        let count = devices.GetCount().unwrap_or(0);
        for d in 0..count {
            let Ok(device) = devices.Item(d) else { continue };
            let Ok(mgr) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else { continue };
            let Ok(list) = mgr.GetSessionEnumerator() else { continue };
            for i in 0..list.GetCount().unwrap_or(0) {
                let Ok(ctl) = list.GetSession(i) else { continue };
                let Ok(ctl2) = ctl.cast::<IAudioSessionControl2>() else { continue };
                let pid = ctl2.GetProcessId().unwrap_or(0);
                if pid == 0 {
                    continue;
                }
                let active = ctl.GetState().map(|s| s == AudioSessionStateActive).unwrap_or(false);
                let Ok(volume) = ctl.cast::<ISimpleAudioVolume>() else { continue };
                out.push(MixerSession { pid, name: process_name(pid), active, volume });
            }
        }
    }
    out
}

fn process_name(pid: u32) -> String {
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return String::new() };
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
        let _ = CloseHandle(h);
        if !ok {
            return String::new();
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        std::path::Path::new(&path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    }
}

/// The mixer sessions of the player with this AppUserModelID ("Chrome",
/// "MSEdge", "Spotify.exe", a hash for Firefox-based browsers…).
fn pick_volume_sessions(app: &str) -> Vec<MixerSession> {
    let all = mixer_sessions();
    let a = app.to_lowercase().replace(".exe", "");
    let own = std::process::id();
    let by_name: Vec<usize> = (0..all.len())
        .filter(|&i| !a.is_empty() && !all[i].name.is_empty() && (a.contains(&all[i].name) || all[i].name.contains(&a)))
        .collect();
    let known_active: Vec<usize> = (0..all.len()).filter(|&i| all[i].active && PLAYERS.contains(&all[i].name.as_str())).collect();
    let known: Vec<usize> = (0..all.len()).filter(|&i| PLAYERS.contains(&all[i].name.as_str())).collect();
    // Last resort: the one other app that is playing sound right now.
    let active_other: Vec<usize> = (0..all.len()).filter(|&i| all[i].active && all[i].pid != own && all[i].name != "audiodg").collect();
    let pick = if !by_name.is_empty() {
        by_name
    } else if !known_active.is_empty() {
        known_active
    } else if !known.is_empty() {
        known
    } else if active_other.len() == 1 {
        active_other
    } else {
        Vec::new()
    };
    let keep: std::collections::HashSet<usize> = pick.into_iter().collect();
    all.into_iter().enumerate().filter(|(i, _)| keep.contains(i)).map(|(_, s)| s).collect()
}

#[derive(Default)]
struct VolumeCache {
    at: Option<Instant>,
    app: String,
    value: Option<(f32, bool)>,
    names: HashMap<u32, String>,
}

impl VolumeCache {
    fn read(&mut self, app: &str) -> Option<(f32, bool)> {
        // The mixer is cheap to read but not free; the UI only needs it now and then.
        if self.app == app && self.at.map(|t| t.elapsed() < Duration::from_millis(1500)).unwrap_or(false) {
            return self.value;
        }
        let s = pick_volume_sessions(app);
        self.value = s.first().map(|s| unsafe { (s.volume.GetMasterVolume().unwrap_or(0.0), s.volume.GetMute().map(|b| b.as_bool()).unwrap_or(false)) });
        self.app = app.into();
        self.at = Some(Instant::now());
        let _ = &self.names;
        self.value
    }

    fn change(&mut self, app: &str, delta: Option<f64>, absolute: Option<f64>) -> Option<f64> {
        let s = pick_volume_sessions(app);
        let first = s.first()?;
        let cur = unsafe { first.volume.GetMasterVolume().unwrap_or(0.0) } as f64;
        let next = absolute.unwrap_or(cur + delta.unwrap_or(0.0)).clamp(0.0, 1.0) as f32;
        for v in &s {
            unsafe {
                let _ = v.volume.SetMasterVolume(next, std::ptr::null());
                if next > 0.0 {
                    let _ = v.volume.SetMute(false, std::ptr::null());
                }
            }
        }
        self.at = None;
        Some((next as f64 * 100.0).round())
    }

    fn toggle_mute(&mut self, app: &str) -> Option<bool> {
        let s = pick_volume_sessions(app);
        let first = s.first()?;
        let muted = unsafe { first.volume.GetMute().map(|b| b.as_bool()).unwrap_or(false) };
        for v in &s {
            unsafe {
                let _ = v.volume.SetMute(!muted, std::ptr::null());
            }
        }
        self.at = None;
        Some(!muted)
    }
}

// ---------------------------------------------------------------------------
// Beat analysis (WASAPI loopback on the default output)

fn loopback(wanted: Arc<AtomicBool>, sink: Sink) {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    loop {
        if !wanted.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(250));
            continue;
        }
        let _ = unsafe { capture(&wanted, &sink) };
        // Device changed or unavailable: try again shortly.
        std::thread::sleep(Duration::from_millis(1500));
    }
}

unsafe fn capture(wanted: &AtomicBool, sink: &Sink) -> windows::core::Result<()> {
    let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
    let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
    let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
    let format = client.GetMixFormat()?;
    let channels = (*format).nChannels as usize;
    let rate = (*format).nSamplesPerSec;
    let bits = (*format).wBitsPerSample as usize;
    let init = client.Initialize(AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, 2_000_000, 0, format, None);
    CoTaskMemFree(Some(format as *const _));
    init?;
    let cap: IAudioCaptureClient = client.GetService()?;
    client.Start()?;

    let mut analyzer = Analyzer::new(rate);
    let mut ring = vec![0f32; N];
    let mut pos = 0usize;
    let mut last = Instant::now();
    let mut sent_silence = false;
    while wanted.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(10));
        loop {
            if cap.GetNextPacketSize()? == 0 {
                break;
            }
            let mut data: *mut u8 = std::ptr::null_mut();
            let mut frames = 0u32;
            let mut flags = 0u32;
            cap.GetBuffer(&mut data, &mut frames, &mut flags, None, None)?;
            let silent = flags & (AUDCLNT_BUFFERFLAGS_SILENT.0 as u32) != 0;
            for f in 0..frames as usize {
                let mut sum = 0f32;
                if !silent && !data.is_null() {
                    for ch in 0..channels {
                        let i = (f * channels + ch) * bits / 8;
                        sum += match bits {
                            32 => *(data.add(i) as *const f32),
                            16 => *(data.add(i) as *const i16) as f32 / 32768.0,
                            _ => 0.0,
                        };
                    }
                }
                ring[pos] = sum / channels.max(1) as f32;
                pos = (pos + 1) % N;
            }
            cap.ReleaseBuffer(frames)?;
        }
        if last.elapsed() < Duration::from_millis(33) {
            continue;
        }
        last = Instant::now();
        match analyzer.frame(&ring, pos) {
            Some(frame) => {
                sent_silence = false;
                sink(Event::Audio(frame));
            }
            None if !sent_silence => {
                sent_silence = true;
                sink(Event::Audio(AudioFrame { bands: vec![0.0; 5], ..Default::default() }));
            }
            None => {}
        }
    }
    let _ = client.Stop();
    let _ = HSTRING::new();
    Ok(())
}
