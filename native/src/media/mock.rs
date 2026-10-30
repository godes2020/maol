// Stand-in "Any player" source for Linux and macOS, where there are no
// Windows media controls: a demo track and a synthetic 124 bpm beat, so the
// UI can be developed and screenshotted anywhere.
use super::{AudioFrame, Backend, Event, Reply, Sink, TrackState};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct Mock {
    state: Arc<Mutex<(TrackState, Instant)>>,
    audio: Arc<Mutex<bool>>,
    sink: Sink,
}

impl Mock {
    pub fn start(sink: Sink) -> Self {
        let idle = std::env::var_os("MOA_DEMO_IDLE").is_some();
        let s = TrackState {
            has_track: !idle,
            playing: !idle,
            title: "Midnight Arcade".into(),
            artist: "Neon Pixels".into(),
            app: "Chrome".into(),
            artwork: Some("demo".into()),
            duration: 170.0,
            current_time: 42.0,
            can_seek: Some(true),
            repeat: Some("NONE".into()),
            volume: Some(70.0),
            ..Default::default()
        };
        let state = Arc::new(Mutex::new((s, Instant::now())));
        let audio = Arc::new(Mutex::new(false));
        sink(Event::Status { ready: true, error: None });
        let cover = std::env::var_os("MOA_DEMO_COVER")
            .and_then(|p| std::fs::read(p).ok());
        sink(Event::Art { key: "demo".into(), bytes: cover });

        let (st, sk) = (state.clone(), sink.clone());
        std::thread::spawn(move || loop {
            let s = current(&st);
            sk(Event::State(s));
            std::thread::sleep(Duration::from_millis(1000));
        });
        let (au, sk) = (audio.clone(), sink.clone());
        std::thread::spawn(move || {
            let beat_ms = 60000.0 / 124.0;
            let start = Instant::now();
            let mut last = -1i64;
            loop {
                std::thread::sleep(Duration::from_millis(33));
                if !*au.lock().unwrap() {
                    continue;
                }
                let t = start.elapsed().as_secs_f32() * 1000.0;
                let phase = (t % beat_ms) / beat_ms;
                let kick = (-phase * 6.0).exp();
                let n = (t / beat_ms) as i64;
                let beat = n != last;
                last = n;
                let wob = |k: f32| 0.5 + 0.5 * (t / (180.0 + k * 70.0) + k).sin();
                sk(Event::Audio(AudioFrame {
                    energy: 0.35 + 0.35 * kick,
                    bass: 0.3 + 0.65 * kick,
                    beat,
                    bpm: 124.0,
                    bands: vec![0.25 + 0.6 * kick, 0.3 + 0.4 * wob(1.0), 0.25 + 0.4 * wob(2.0), 0.2 + 0.35 * wob(3.0), 0.15 + 0.3 * wob(4.0)],
                }));
            }
        });
        Self { state, audio, sink }
    }
}

fn current(st: &Arc<Mutex<(TrackState, Instant)>>) -> TrackState {
    let (s, at) = &*st.lock().unwrap();
    let mut s = s.clone();
    if s.playing {
        s.current_time = (s.current_time + at.elapsed().as_secs_f64()).min(s.duration);
    }
    s
}

impl Backend for Mock {
    fn cmd(&self, name: &str, arg: f64, reply: Reply) {
        let now = current(&self.state);
        let mut g = self.state.lock().unwrap();
        g.0.current_time = now.current_time;
        g.1 = Instant::now();
        let s = &mut g.0;
        let value = match name {
            "playPause" => {
                s.playing = !s.playing;
                serde_json::json!(true)
            }
            "seekTo" => {
                s.current_time = arg;
                serde_json::json!(true)
            }
            "seekBy" => {
                s.current_time = (s.current_time + arg).max(0.0);
                serde_json::json!(true)
            }
            "volumeBy" | "setVolume" => {
                let v = if name == "volumeBy" { s.volume.unwrap_or(50.0) + arg } else { arg };
                s.volume = Some(v.clamp(0.0, 100.0));
                serde_json::json!(s.volume)
            }
            "toggleMute" => {
                s.muted = !s.muted;
                serde_json::json!(s.muted)
            }
            "cycleRepeat" => {
                let next = match s.repeat.as_deref() { Some("NONE") => "ALL", Some("ALL") => "ONE", _ => "NONE" };
                s.repeat = Some(next.into());
                serde_json::json!(next)
            }
            _ => serde_json::json!(true),
        };
        let snapshot = s.clone();
        drop(g);
        (self.sink)(Event::State(snapshot));
        reply(Some(value));
    }

    fn set_audio(&self, on: bool) {
        *self.audio.lock().unwrap() = on;
        if !on {
            (self.sink)(Event::Audio(AudioFrame { bands: vec![0.0; 5], ..Default::default() }));
        }
    }

    fn diagnostics(&self) -> String {
        "Demo source (not Windows): no Windows media controls on this system.".into()
    }
}
