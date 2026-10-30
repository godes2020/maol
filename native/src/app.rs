// Moa's brain: owns the windows, the sources and the settings, and routes
// everything between them. Lives on the UI thread.
use crate::art::{self, Art};
use crate::config::{Config, ACTIONS};
use crate::connector::{Connector, ExtEvent};
use crate::hotkeys::Hotkeys;
use crate::i18n::I18n as Strings;
use crate::media::{self, AudioFrame, Backend, Event, TrackState};
use crate::platform::{self, Rect};
use crate::{AppWindow, Cfg, HotkeyRow, Overlay};
use slint::winit_030::WinitWindowAccessor;
use slint::{ComponentHandle, Image, ModelRc, PhysicalPosition, SharedString, Timer, TimerMode, VecModel};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

thread_local! {
    static APP: RefCell<Option<Rc<App>>> = const { RefCell::new(None) };
}

/// Runs `f` on the UI thread with the app (safe to call from any thread).
pub fn post(f: impl FnOnce(&Rc<App>) + Send + 'static) {
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(app) = APP.with(|a| a.borrow().clone()) {
            f(&app);
        }
    });
}

const PAD: f32 = 18.0;

fn box_size(cover: bool) -> (f32, f32) {
    if cover {
        (96.0, 128.0)
    } else {
        (300.0, 168.0)
    }
}

#[derive(Default)]
struct Shown {
    energy: f32,
    bass: f32,
    pulse: f32,
    bands: [f32; 5],
}

struct Resize {
    start: (i32, i32),
    scale0: f32,
    rect0: Rect,
    right: bool,
    bottom: bool,
}

pub struct App {
    cfg: RefCell<Config>,
    t: Strings,
    overlay: Overlay,
    appwin: RefCell<Option<AppWindow>>,
    system: Box<dyn Backend>,
    connector: Connector,
    hotkeys: RefCell<Hotkeys>,
    timers: RefCell<Vec<Timer>>,
    frame_timer: Timer,

    sys_state: RefCell<Option<TrackState>>,
    ext_state: RefCell<Option<TrackState>>,
    ext_connected: Cell<bool>,
    ext_error: RefCell<Option<String>>,
    media_status: RefCell<(bool, Option<String>)>,
    state: RefCell<TrackState>,
    state_at: Cell<Instant>,
    art_bytes: RefCell<HashMap<String, Option<Vec<u8>>>>,
    art_key: RefCell<Option<String>>,
    art: RefCell<Option<Art>>,
    pending: RefCell<HashMap<u64, Box<dyn FnOnce(Option<serde_json::Value>)>>>,

    target: RefCell<AudioFrame>,
    shown: RefCell<Shown>,
    beat_period: Cell<f32>,
    clock: Cell<f64>,
    last_bands: Cell<[f32; 5]>,
    last_frame: Cell<Instant>,
    audio_on: Cell<bool>,

    editing: Cell<bool>,
    hot: Cell<bool>,
    click_through: Cell<bool>,
    last_pos: Cell<(i32, i32)>,
    resize: RefCell<Option<Resize>>,
    capturing: RefCell<String>,
    hk_msg: RefCell<String>,
    restart_note: Cell<bool>,
    diag_copied: Cell<bool>,
    toast_timer: Timer,
    hide_timer: Timer,
}

impl App {
    pub fn start(hidden: bool) -> Rc<App> {
        let cfg = Config::load();
        let t = Strings::new(&cfg.general.language);
        let overlay = Overlay::new().expect("overlay window");
        overlay.set_keyed(platform::overlay_keyed());
        let system = media::system(std::sync::Arc::new(|ev| post(move |app| app.on_system(ev))));
        let connector = Connector::start(|ev| post(move |app| app.on_extension(ev)));
        let hotkeys = Hotkeys::new(|action| post(move |app| app.run_action(&action)));

        let app = Rc::new(App {
            cfg: RefCell::new(cfg),
            t,
            overlay,
            appwin: RefCell::new(None),
            system,
            connector,
            hotkeys: RefCell::new(hotkeys),
            timers: RefCell::new(Vec::new()),
            frame_timer: Timer::default(),
            sys_state: RefCell::new(None),
            ext_state: RefCell::new(None),
            ext_connected: Cell::new(false),
            ext_error: RefCell::new(None),
            media_status: RefCell::new((false, None)),
            state: RefCell::new(TrackState::default()),
            state_at: Cell::new(Instant::now()),
            art_bytes: RefCell::new(HashMap::new()),
            art_key: RefCell::new(None),
            art: RefCell::new(None),
            pending: RefCell::new(HashMap::new()),
            target: RefCell::new(AudioFrame::default()),
            shown: RefCell::new(Shown::default()),
            beat_period: Cell::new(0.5),
            clock: Cell::new(0.0),
            last_bands: Cell::new([0.0; 5]),
            last_frame: Cell::new(Instant::now()),
            audio_on: Cell::new(false),
            editing: Cell::new(false),
            hot: Cell::new(false),
            click_through: Cell::new(false),
            last_pos: Cell::new((i32::MIN, i32::MIN)),
            resize: RefCell::new(None),
            capturing: RefCell::new(String::new()),
            hk_msg: RefCell::new(String::new()),
            restart_note: Cell::new(false),
            diag_copied: Cell::new(false),
            toast_timer: Timer::default(),
            hide_timer: Timer::default(),
        });
        APP.with(|a| *a.borrow_mut() = Some(app.clone()));
        app.setup_overlay();
        app.register_hotkeys();
        app.start_timers();
        platform::set_autostart(app.cfg.borrow().general.autostart);

        let first_run = !app.cfg.borrow().general.onboarded;
        if first_run {
            app.cfg.borrow_mut().general.onboarded = true;
            app.save();
        }
        if first_run || !hidden {
            let tab = std::env::var("MOA_TAB").unwrap_or_else(|_| "home".into());
            app.open_app_window(Some(&tab));
        }
        app
    }

    fn save(&self) {
        self.cfg.borrow().save();
    }

    fn source(&self) -> String {
        self.cfg.borrow().general.source.clone()
    }

    // -----------------------------------------------------------------
    // Overlay window

    fn setup_overlay(self: &Rc<Self>) {
        let o = &self.overlay;
        self.wire_track(o.global::<crate::Track>(), o.global::<crate::I18n>());
        let w = Rc::downgrade(self);
        o.on_drag(move || {
            if let Some(app) = w.upgrade() {
                app.overlay.window().with_winit_window(|win| {
                    let _ = win.drag_window();
                });
            }
        });
        let w = Rc::downgrade(self);
        o.on_grip(move |on| {
            if let Some(app) = w.upgrade() {
                if on {
                    app.start_resize();
                } else {
                    app.end_resize();
                }
            }
        });
        self.apply_overlay_config();
        self.place_overlay_initial();
        self.update_overlay_visibility();
    }

    /// Connects a window's Track/I18n globals to the app.
    fn wire_track(self: &Rc<Self>, track: crate::Track<'_>, i18n: crate::I18n<'_>) {
        let w = Rc::downgrade(self);
        i18n.on_tr(move |k| w.upgrade().map(|a| a.t.t(&k)).unwrap_or_default().into());
        let w = Rc::downgrade(self);
        track.on_command(move |c| {
            if let Some(app) = w.upgrade() {
                let c = c.to_string();
                // The overlay's own ✕: hide it with a hint on how to get it back.
                if c == "toggleOverlay" && app.cfg.borrow().overlay.visible {
                    app.hide_with_hint();
                } else {
                    app.run_action(&c);
                }
            }
        });
        let w = Rc::downgrade(self);
        track.on_seek_to(move |r| {
            if let Some(app) = w.upgrade() {
                let dur = app.state.borrow().duration;
                if dur > 0.0 {
                    let to = r as f64 * dur;
                    app.player_cmd("seekTo", to, None);
                    let mut s = app.state.borrow_mut();
                    s.current_time = to;
                    drop(s);
                    app.state_at.set(Instant::now());
                    app.tick_progress();
                }
            }
        });
        let w = Rc::downgrade(self);
        track.on_set_volume(move |v| {
            if let Some(app) = w.upgrade() {
                app.player_cmd("setVolume", v as f64, None);
            }
        });
    }

    fn overlay_scale(&self) -> f32 {
        self.cfg.borrow().overlay.scale.clamp(0.6, 2.5)
    }

    /// Physical size of the overlay window for a scale.
    fn overlay_size(&self, scale: f32) -> (i32, i32) {
        let (bw, bh) = box_size(self.cfg.borrow().overlay.layout == "cover");
        let k = scale * self.overlay.window().scale_factor();
        (((bw + 2.0 * PAD) * k).round() as i32, ((bh + 2.0 * PAD) * k).round() as i32)
    }

    fn overlay_rect(&self) -> Rect {
        let p = self.overlay.window().position();
        let (w, h) = self.overlay_size(self.overlay_scale());
        Rect { x: p.x, y: p.y, w, h }
    }

    /// Keeps the visible box on screen; the transparent glow padding may hang over.
    fn fit(&self, r: Rect) -> Rect {
        let area = platform::work_area(self.overlay.window());
        let pad = (PAD * self.overlay_scale() * self.overlay.window().scale_factor()) as i32;
        Rect {
            x: r.x.clamp(area.x - pad, (area.x + area.w - r.w + pad).max(area.x - pad)),
            y: r.y.clamp(area.y - pad, (area.y + area.h - r.h + pad).max(area.y - pad)),
            ..r
        }
    }

    fn place_overlay_initial(&self) {
        let (x, y) = {
            let c = self.cfg.borrow();
            (c.overlay.x, c.overlay.y)
        };
        let area = platform::work_area(self.overlay.window());
        let (w, h) = self.overlay_size(self.overlay_scale());
        let r = Rect { x: x.unwrap_or(area.x + 12), y: y.unwrap_or(area.y + 12), w, h };
        let r = self.fit(r);
        self.overlay.window().set_position(PhysicalPosition::new(r.x, r.y));
        self.last_pos.set((r.x, r.y));
        self.update_anchors();
    }

    /// Which screen half the overlay is in decides where it grows: on the
    /// right it grows to the left, at the bottom it unfolds upwards.
    fn anchors(&self) -> (bool, bool) {
        let r = self.overlay_rect();
        let area = platform::work_area(self.overlay.window());
        (r.x + r.w / 2 > area.x + area.w / 2, r.y + r.h / 2 > area.y + area.h / 2)
    }

    fn update_anchors(&self) {
        let (right, bottom) = self.anchors();
        let side = self.cfg.borrow().overlay.side.clone();
        self.overlay.set_anchor_right(right);
        self.overlay.set_anchor_bottom(bottom);
        self.overlay.set_cover_right(match side.as_str() {
            "left" => false,
            "right" => true,
            _ => right,
        });
    }

    /// Resizes keeping the corner nearest to the screen edge in place.
    fn rescale(&self, scale: f32, from: Rect) {
        let (right, bottom) = {
            let area = platform::work_area(self.overlay.window());
            (from.x + from.w / 2 > area.x + area.w / 2, from.y + from.h / 2 > area.y + area.h / 2)
        };
        let (w, h) = self.overlay_size(scale);
        let r = self.fit(Rect {
            x: if right { from.x + from.w - w } else { from.x },
            y: if bottom { from.y + from.h - h } else { from.y },
            w,
            h,
        });
        self.overlay.set_scale(scale);
        self.overlay.window().set_position(PhysicalPosition::new(r.x, r.y));
        self.last_pos.set((r.x, r.y));
    }

    fn apply_overlay_config(&self) {
        let (o, edit_key) = {
            let c = self.cfg.borrow();
            (c.overlay.clone(), c.hotkey("editOverlay"))
        };
        let before = self.overlay_rect();
        let size_before = (before.w, before.h);
        self.overlay.set_layout_cover(o.layout == "cover");
        self.overlay.set_always_expanded(o.always_expanded);
        self.overlay.set_reactive(o.reactive);
        self.overlay.set_intensity(o.intensity);
        self.overlay.set_card_opacity(o.opacity);
        self.overlay.set_edit_key(edit_key.into());
        let (w, h) = self.overlay_size(o.scale);
        if (w, h) != size_before || (self.overlay.get_scale() - o.scale).abs() > 0.001 {
            self.rescale(o.scale, before);
            let (x, y) = self.last_pos.get();
            let mut c = self.cfg.borrow_mut();
            c.overlay.x = Some(x);
            c.overlay.y = Some(y);
        }
        self.update_anchors();
        self.update_overlay_visibility();
        self.update_audio();
        self.update_click_through();
    }

    fn update_overlay_visibility(&self) {
        let (visible, hide_idle) = {
            let c = self.cfg.borrow();
            (c.overlay.visible, c.overlay.hide_when_idle)
        };
        let idle = !self.state.borrow().has_track;
        let show = self.editing.get() || (visible && !(hide_idle && idle));
        let w = self.overlay.window();
        if show && !w.is_visible() {
            let _ = w.show();
            platform::prepare_overlay(w);
            self.click_through.set(false);
            self.update_click_through();
        } else if !show && w.is_visible() {
            let _ = w.hide();
        }
    }

    /// Locked: every click goes into the game. Only edit mode and hover
    /// (in "on hover" mode) let the mouse in.
    fn update_click_through(&self) {
        let through = !(self.editing.get() || self.hot.get());
        if through != self.click_through.get() && self.overlay.window().is_visible() {
            platform::set_click_through(self.overlay.window(), through);
            self.click_through.set(through);
        }
    }

    fn set_edit(&self, on: bool) {
        self.editing.set(on);
        self.overlay.set_editing(on);
        if !on {
            self.hot.set(false);
            self.overlay.set_hot(false);
            self.save_position();
        }
        self.update_overlay_visibility();
        self.update_click_through();
    }

    fn save_position(&self) {
        let p = self.overlay.window().position();
        let mut c = self.cfg.borrow_mut();
        c.overlay.x = Some(p.x);
        c.overlay.y = Some(p.y);
        drop(c);
        self.save();
    }

    fn start_resize(&self) {
        let start = platform::cursor_pos().unwrap_or((0, 0));
        let r = self.overlay_rect();
        let (right, bottom) = self.anchors();
        *self.resize.borrow_mut() = Some(Resize { start, scale0: self.overlay_scale(), rect0: r, right, bottom });
    }

    fn end_resize(&self) {
        if self.resize.borrow_mut().take().is_some() {
            let s = self.overlay.get_scale();
            self.cfg.borrow_mut().overlay.scale = s;
            self.save_position();
            self.push_app();
        }
    }

    fn tick_resize(&self) {
        let Some((start, scale0, rect0, right, bottom)) = self.resize.borrow().as_ref().map(|r| (r.start, r.scale0, r.rect0, r.right, r.bottom)) else { return };
        let Some(p) = platform::cursor_pos() else { return };
        let dx = (p.0 - start.0) as f32 * if right { -1.0 } else { 1.0 };
        let dy = (p.1 - start.1) as f32 * if bottom { -1.0 } else { 1.0 };
        let cover = self.cfg.borrow().overlay.layout == "cover";
        let (bw, _) = box_size(cover);
        let delta = if cover { (dx + dy) / 2.0 } else { dx };
        let base = bw * scale0 * self.overlay.window().scale_factor();
        let scale = ((scale0 * (1.0 + delta / base)).clamp(0.6, 2.5) * 100.0).round() / 100.0;
        if (scale - self.overlay.get_scale()).abs() > 0.001 {
            self.cfg.borrow_mut().overlay.scale = scale;
            self.rescale(scale, rect0);
        }
    }

    /// "On hover" mode: the overlay stays click-through and watches the
    /// cursor; when it is over the card, the overlay opens and takes the mouse.
    fn tick_hover(&self) {
        let hover_mode = self.cfg.borrow().overlay.interaction == "hover";
        if !hover_mode || self.editing.get() || !self.overlay.window().is_visible() {
            if self.hot.get() && !self.editing.get() {
                self.hot.set(false);
                self.overlay.set_hot(false);
                self.update_click_through();
            }
            return;
        }
        let Some((x, y)) = platform::cursor_pos() else { return };
        let r = self.overlay_rect();
        let pad = (PAD * self.overlay_scale() * self.overlay.window().scale_factor()) as i32;
        let inner = Rect { x: r.x + pad, y: r.y + pad, w: r.w - 2 * pad, h: r.h - 2 * pad };
        let over = inner.contains(x, y);
        if over != self.hot.get() {
            self.hot.set(over);
            self.overlay.set_hot(over);
            self.update_click_through();
        }
    }

    fn hide_with_hint(self: &Rc<Self>) {
        let key = self.cfg.borrow().hotkey("toggleOverlay");
        self.toast(self.t.tv("overlay.hiddenShort", &[("key", if key.is_empty() { "—" } else { &key })]));
        let w = Rc::downgrade(self);
        self.hide_timer.start(TimerMode::SingleShot, Duration::from_millis(1800), move || {
            if let Some(app) = w.upgrade() {
                if app.cfg.borrow().overlay.visible {
                    app.run_action("toggleOverlay");
                }
            }
        });
    }

    // -----------------------------------------------------------------
    // Timers

    fn start_timers(self: &Rc<Self>) {
        let mut timers = self.timers.borrow_mut();
        let w = Rc::downgrade(self);
        let progress = Timer::default();
        progress.start(TimerMode::Repeated, Duration::from_millis(250), move || {
            if let Some(app) = w.upgrade() {
                app.tick_progress();
            }
        });
        timers.push(progress);

        let w = Rc::downgrade(self);
        let fast = Timer::default();
        fast.start(TimerMode::Repeated, Duration::from_millis(16), move || {
            if let Some(app) = w.upgrade() {
                app.tick_resize();
            }
        });
        timers.push(fast);

        let w = Rc::downgrade(self);
        let pointer = Timer::default();
        pointer.start(TimerMode::Repeated, Duration::from_millis(60), move || {
            if let Some(app) = w.upgrade() {
                app.tick_hover();
                // Moved by dragging in edit mode.
                let p = app.overlay.window().position();
                if (p.x, p.y) != app.last_pos.get() && app.resize.borrow().is_none() {
                    app.last_pos.set((p.x, p.y));
                    app.update_anchors();
                }
            }
        });
        timers.push(pointer);

        // Keys someone else held may free up (the old Moa was closed).
        let w = Rc::downgrade(self);
        let keys = Timer::default();
        keys.start(TimerMode::Repeated, Duration::from_secs(5), move || {
            if let Some(app) = w.upgrade() {
                let waiting = app.capturing.borrow().is_empty() && !app.hotkeys.borrow().failed.is_empty();
                if waiting && app.hotkeys.borrow_mut().retry() {
                    app.push_app();
                }
            }
        });
        timers.push(keys);

        let w = Rc::downgrade(self);
        let top = Timer::default();
        top.start(TimerMode::Repeated, Duration::from_secs(3), move || {
            if let Some(app) = w.upgrade() {
                if app.overlay.window().is_visible() {
                    platform::keep_on_top(app.overlay.window());
                }
            }
        });
        timers.push(top);
        drop(timers);
        self.schedule_frame(false);
    }

    /// The glow animation runs at ~30 fps while something moves and sleeps
    /// (4 checks a second) when everything has faded out.
    fn schedule_frame(self: &Rc<Self>, idle: bool) {
        let w = Rc::downgrade(self);
        let ms = if idle { 250 } else { 33 };
        self.frame_timer.start(TimerMode::SingleShot, Duration::from_millis(ms), move || {
            if let Some(app) = w.upgrade() {
                let still = app.tick_frame();
                app.schedule_frame(still);
            }
        });
    }

    fn tick_progress(&self) {
        let s = self.state.borrow();
        let mut cur = s.current_time;
        if s.playing {
            cur += self.state_at.get().elapsed().as_secs_f64();
        }
        if s.duration > 0.0 {
            cur = cur.min(s.duration);
        }
        let p = if s.duration > 0.0 { (cur / s.duration) as f32 } else { 0.0 };
        let (cur_t, dur_t, rem_t) = (art::fmt_time(cur), art::fmt_time(s.duration), format!("-{}", art::fmt_time(s.duration - cur)));
        drop(s);
        let ring: SharedString = art::ring_path(p).into();
        self.each_track(|t| {
            t.set_progress(p);
            t.set_cur_text(cur_t.clone().into());
            t.set_dur_text(dur_t.clone().into());
            t.set_rem_text(rem_t.clone().into());
            t.set_ring_path(ring.clone());
        });
    }

    /// Audio frames are noisy; they only set targets, and this eases towards
    /// them so the glow breathes instead of flickering. Returns true when still.
    fn tick_frame(&self) -> bool {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame.get()).as_secs_f32().min(0.1);
        self.last_frame.set(now);
        let playing = self.state.borrow().playing;
        let tgt = self.target.borrow();
        let mut sh = self.shown.borrow_mut();
        let ease = |from: f32, to: f32, rate: f32| from + (to - from) * (1.0 - (-rate * dt).exp());
        let te = if playing { tgt.energy } else { 0.0 };
        let tb = if playing { tgt.bass } else { 0.0 };
        sh.energy = ease(sh.energy, te, if te > sh.energy { 10.0 } else { 3.0 });
        sh.bass = ease(sh.bass, tb, if tb > sh.bass { 12.0 } else { 4.0 });
        let decay = 2.6 / self.beat_period.get().max(0.25);
        sh.pulse = (sh.bass * 0.5).max(sh.pulse * (-decay * dt).exp());
        for i in 0..5 {
            let t = if playing { tgt.bands.get(i).copied().unwrap_or(0.0) } else { 0.0 };
            sh.bands[i] = ease(sh.bands[i], t, if t > sh.bands[i] { 18.0 } else { 6.0 });
        }
        let still = !playing && sh.energy < 0.002 && sh.pulse < 0.002 && sh.bands.iter().all(|b| *b < 0.002);
        let (e, p, bands) = (sh.energy, sh.pulse, sh.bands);
        drop(sh);
        drop(tgt);
        if playing {
            self.clock.set(self.clock.get() + dt as f64);
        }
        let clock = (self.clock.get() * 1000.0) as i64;
        // A new model always counts as a change; skip it when nothing moved
        // so an idle overlay is not redrawn.
        let model: Option<ModelRc<f32>> = (bands != self.last_bands.replace(bands)).then(|| ModelRc::new(VecModel::from(bands.to_vec())));
        self.each_track(|t| {
            t.set_energy(e);
            t.set_pulse(p);
            if let Some(m) = &model {
                t.set_bands(m.clone());
            }
            t.set_clock(clock);
        });
        still
    }

    fn on_audio(&self, a: AudioFrame) {
        let intensity = 1.0;
        let mut t = self.target.borrow_mut();
        t.energy = (a.energy * 1.6).min(1.0) * intensity;
        t.bass = ((a.bass - 0.25) * 1.5).clamp(0.0, 1.0);
        t.bands = a.bands.iter().map(|v| (v * 1.5).min(1.0)).collect();
        if (60.0..=200.0).contains(&a.bpm) {
            self.beat_period.set(60.0 / a.bpm);
        }
        if a.beat {
            let mut sh = self.shown.borrow_mut();
            sh.pulse = sh.pulse.max(0.55 + 0.45 * t.bass);
        }
    }

    fn each_track(&self, f: impl Fn(crate::Track<'_>)) {
        f(self.overlay.global::<crate::Track>());
        if let Some(a) = self.appwin.borrow().as_ref() {
            f(a.global::<crate::Track>());
        }
    }

    fn each_theme(&self, f: impl Fn(crate::Theme<'_>)) {
        f(self.overlay.global::<crate::Theme>());
        if let Some(a) = self.appwin.borrow().as_ref() {
            f(a.global::<crate::Theme>());
        }
    }

    // -----------------------------------------------------------------
    // Sources

    fn on_system(self: &Rc<Self>, ev: Event) {
        match ev {
            Event::State(s) => {
                *self.sys_state.borrow_mut() = Some(s);
                if self.source() == "system" {
                    self.sync_state();
                }
            }
            Event::Art { key, bytes } => {
                self.art_bytes.borrow_mut().insert(key.clone(), bytes);
                // Only the current and the previous cover are ever needed.
                let keep = self.art_key.borrow().clone();
                self.art_bytes.borrow_mut().retain(|k, _| *k == key || Some(k) == keep.as_ref());
                if self.state.borrow().artwork.as_deref() == Some(&key) {
                    self.load_art(Some(key));
                }
            }
            Event::Audio(a) => self.on_audio(a),
            Event::Status { ready, error } => {
                *self.media_status.borrow_mut() = (ready, error);
                self.update_audio();
                self.push_app();
            }
        }
    }

    fn on_extension(self: &Rc<Self>, ev: ExtEvent) {
        match ev {
            ExtEvent::Connected(on, _) => {
                self.ext_connected.set(on);
                if !on {
                    *self.ext_state.borrow_mut() = None;
                }
                self.sync_state();
                self.push_app();
            }
            ExtEvent::State(s) => {
                *self.ext_state.borrow_mut() = Some(s);
                self.sync_state();
            }
            ExtEvent::Audio(a) => {
                if self.via_extension() {
                    self.on_audio(a);
                }
            }
            ExtEvent::Result(id, v) => {
                if let Some(f) = self.pending.borrow_mut().remove(&id) {
                    f(Some(v));
                }
            }
            ExtEvent::Error(e) => {
                *self.ext_error.borrow_mut() = Some(e);
                self.push_app();
            }
        }
    }

    /// The extension knows a YouTube Music tab better than Windows does, so a
    /// connected extension with a track takes over, unless something else is
    /// playing while its tab is paused.
    fn via_extension(&self) -> bool {
        let src = self.source();
        if src == "browser" {
            return true;
        }
        if src != "system" || !self.ext_connected.get() {
            return false;
        }
        let ext = self.ext_state.borrow();
        let Some(e) = ext.as_ref().filter(|e| e.has_track) else { return false };
        e.playing || !self.sys_state.borrow().as_ref().map(|s| s.playing).unwrap_or(false)
    }

    pub fn sync_state(self: &Rc<Self>) {
        let s = if self.via_extension() {
            self.ext_state.borrow().clone()
        } else if self.source() == "system" {
            self.sys_state.borrow().clone()
        } else {
            crate::player::state()
        };
        self.apply_state(s.unwrap_or_default());
    }

    fn apply_state(self: &Rc<Self>, s: TrackState) {
        let was_track = self.state.borrow().has_track;
        let art_changed = self.state.borrow().artwork != s.artwork;
        let (name, icon) = source_badge(&s, self.via_extension(), &self.t);
        self.each_track(|t| {
            t.set_has_track(s.has_track);
            t.set_playing(s.playing);
            t.set_title(s.title.clone().into());
            t.set_artist(s.artist.clone().into());
            t.set_source_name(name.clone().into());
            t.set_source_icon(icon.clone());
            t.set_can_seek(s.can_seek.unwrap_or(true) && s.duration > 0.0);
            t.set_muted(s.muted);
            t.set_volume(s.volume.map(|v| v.round() as i32).unwrap_or(-1));
            t.set_repeat(s.repeat.clone().unwrap_or_else(|| "NONE".into()).into());
            t.set_repeat_supported(s.repeat.is_some());
        });
        *self.state.borrow_mut() = s.clone();
        self.state_at.set(Instant::now());
        self.tick_progress();
        if art_changed {
            self.load_art(s.artwork.clone());
        }
        if was_track != s.has_track {
            self.update_overlay_visibility();
        }
        self.update_audio();
    }

    fn load_art(self: &Rc<Self>, key: Option<String>) {
        *self.art_key.borrow_mut() = key.clone();
        let Some(k) = key else { return self.set_art(None) };
        if let Some(bytes) = self.art_bytes.borrow().get(&k).cloned() {
            return self.set_art(bytes.and_then(|b| art::decode(&b)));
        }
        if k.starts_with("data:") {
            return self.set_art(art::decode_data_url(&k));
        }
        if k.starts_with("https://") {
            // Fetched off the UI thread; decoded back on it.
            std::thread::spawn(move || {
                let bytes = ureq::get(&k).call().ok().and_then(|mut r| r.body_mut().with_config().limit(4_000_000).read_to_vec().ok());
                post(move |app| {
                    if app.art_key.borrow().as_deref() == Some(&k) {
                        app.art_bytes.borrow_mut().insert(k.clone(), bytes.clone());
                        app.set_art(bytes.and_then(|b| art::decode(&b)));
                    }
                });
            });
        }
    }

    fn set_art(&self, art: Option<Art>) {
        let (img, backdrop, has) = match &art {
            Some(a) => (a.image.clone(), a.backdrop.clone(), true),
            None => (Image::default(), Image::default(), false),
        };
        self.each_track(|t| {
            t.set_art(img.clone());
            t.set_backdrop(backdrop.clone());
            t.set_has_art(has);
        });
        let (c1, c2) = art
            .as_ref()
            .and_then(|a| a.colors)
            .unwrap_or((slint::Color::from_rgb_u8(139, 92, 246), slint::Color::from_rgb_u8(236, 72, 153)));
        self.each_theme(|th| {
            th.set_c1(c1);
            th.set_c2(c2);
        });
        *self.art.borrow_mut() = art;
    }

    /// The beat analysis listens to the speakers, so it works for every
    /// source; it runs only while music plays and the glow is on.
    fn update_audio(&self) {
        let on = self.cfg.borrow().overlay.reactive && self.state.borrow().playing;
        if on != self.audio_on.get() {
            self.audio_on.set(on);
            self.system.set_audio(on);
        }
    }

    /// Sends a playback command to whichever source is active.
    fn player_cmd(self: &Rc<Self>, name: &str, arg: f64, done: Option<Box<dyn FnOnce(Option<serde_json::Value>)>>) {
        if self.via_extension() {
            if let Some(id) = self.connector.cmd(name, arg) {
                if let Some(d) = done {
                    self.pending.borrow_mut().insert(id, d);
                }
            } else if let Some(d) = done {
                d(None);
            }
            return;
        }
        if self.source() == "system" {
            let mut done = done;
            let id = self.next_pending(done.take());
            self.system.cmd(
                name,
                arg,
                Box::new(move |v| {
                    post(move |app| {
                        if let Some(f) = app.pending.borrow_mut().remove(&id) {
                            f(v);
                        }
                    })
                }),
            );
            return;
        }
        let id = self.next_pending(done);
        crate::player::cmd(
            name,
            arg,
            Box::new(move |v| {
                post(move |app| {
                    if let Some(f) = app.pending.borrow_mut().remove(&id) {
                        f(v);
                    }
                })
            }),
        );
    }

    fn next_pending(&self, f: Option<Box<dyn FnOnce(Option<serde_json::Value>)>>) -> u64 {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1 << 40);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if let Some(f) = f {
            self.pending.borrow_mut().insert(id, f);
        }
        id
    }

    // -----------------------------------------------------------------
    // Actions (hotkeys, buttons, tray)

    pub fn run_action(self: &Rc<Self>, name: &str) {
        let step = self.cfg.borrow().playback.clone();
        match name {
            "playPause" | "next" | "previous" => self.player_cmd(name, 0.0, None),
            "seekForward" => self.player_cmd("seekBy", step.seek_step, None),
            "seekBackward" => self.player_cmd("seekBy", -step.seek_step, None),
            "volumeUp" | "volumeDown" => {
                let d = if name == "volumeUp" { step.volume_step } else { -step.volume_step };
                let w = Rc::downgrade(self);
                self.player_cmd(
                    "volumeBy",
                    d,
                    Some(Box::new(move |v| {
                        if let Some(app) = w.upgrade() {
                            match v.and_then(|v| v.as_f64()) {
                                Some(v) => app.toast(format!("{} {}%", app.t.t("overlay.volume"), v.round())),
                                None => app.toast(app.t.t("overlay.noVolume")),
                            }
                        }
                    })),
                );
            }
            "toggleMute" => {
                let w = Rc::downgrade(self);
                self.player_cmd(
                    "toggleMute",
                    0.0,
                    Some(Box::new(move |v| {
                        if let Some(app) = w.upgrade() {
                            match v.and_then(|v| v.as_bool()) {
                                Some(true) => app.toast(app.t.t("overlay.muted")),
                                Some(false) => {
                                    let vol = app.state.borrow().volume.map(|v| format!(" {}%", v.round())).unwrap_or_default();
                                    app.toast(format!("{}{}", app.t.t("overlay.volume"), vol));
                                }
                                None => app.toast(app.t.t("overlay.noVolume")),
                            }
                        }
                    })),
                );
            }
            "cycleRepeat" => {
                let w = Rc::downgrade(self);
                self.player_cmd(
                    "cycleRepeat",
                    0.0,
                    Some(Box::new(move |v| {
                        if let Some(app) = w.upgrade() {
                            let key = match v.as_ref().and_then(|v| v.as_str()) {
                                Some("ALL") => "overlay.repeatAll",
                                Some("ONE") => "overlay.repeatOne",
                                Some("NONE") => "overlay.repeatOff",
                                _ => "overlay.repeatUnsupported",
                            };
                            app.toast(app.t.t(key));
                        }
                    })),
                );
            }
            "toggleOverlay" => {
                let v = !self.cfg.borrow().overlay.visible;
                self.cfg.borrow_mut().overlay.visible = v;
                if !v && self.editing.get() {
                    self.set_edit(false);
                }
                self.save();
                self.update_overlay_visibility();
                self.push_app();
            }
            "editOverlay" => {
                self.set_edit(!self.editing.get());
            }
            "toggleLock" => {
                let next = if self.cfg.borrow().overlay.interaction == "locked" { "hover" } else { "locked" };
                self.cfg.borrow_mut().overlay.interaction = next.into();
                self.save();
                self.apply_overlay_config();
                self.toast(self.t.t(if next == "locked" { "overlay.locked" } else { "overlay.unlocked" }));
                self.push_app();
            }
            "flipSide" => {
                let right = self.overlay.get_cover_right();
                self.cfg.borrow_mut().overlay.side = if right { "left".into() } else { "right".into() };
                self.save();
                self.apply_overlay_config();
                self.push_app();
            }
            "toggleLayout" => {
                let next = if self.cfg.borrow().overlay.layout == "cover" { "card" } else { "cover" };
                self.cfg.borrow_mut().overlay.layout = next.into();
                self.save();
                self.apply_overlay_config();
                self.push_app();
            }
            "resetOverlay" => {
                {
                    let mut c = self.cfg.borrow_mut();
                    c.overlay.x = None;
                    c.overlay.y = None;
                    c.overlay.scale = 1.0;
                    c.overlay.side = "auto".into();
                }
                self.overlay.set_scale(1.0);
                self.place_overlay_initial();
                self.save_position();
                self.apply_overlay_config();
                self.push_app();
            }
            "showApp" => self.toggle_app_window(),
            "openPlayer" => {
                let service = self.cfg.borrow().general.service.clone();
                crate::player::open(&service);
                self.push_app();
            }
            "resetHotkeys" => {
                let mut c = self.cfg.borrow_mut();
                for (a, k) in ACTIONS {
                    c.hotkeys.insert((*a).into(), (*k).into());
                }
                drop(c);
                self.save();
                self.register_hotkeys();
                self.hk_msg.borrow_mut().clear();
                self.push_app();
            }
            "copyDiagnostics" => {
                let mut report = format!("Moa {}\nSource: {}\nExtension connected: {}\n\n", env!("CARGO_PKG_VERSION"), self.source(), self.ext_connected.get());
                report += &self.system.diagnostics();
                platform::copy_text(&report);
                self.diag_copied.set(true);
                self.push_app();
            }
            "openExtensionFolder" => open_path(&extension_dir()),
            _ => {}
        }
    }

    fn toast(self: &Rc<Self>, text: String) {
        self.each_track(|t| {
            t.set_toast(text.clone().into());
            t.set_toasting(true);
        });
        let w = Rc::downgrade(self);
        self.toast_timer.start(TimerMode::SingleShot, Duration::from_millis(1400), move || {
            if let Some(app) = w.upgrade() {
                app.each_track(|t| t.set_toasting(false));
            }
        });
    }

    fn register_hotkeys(&self) {
        let list: Vec<(String, String)> = {
            let c = self.cfg.borrow();
            ACTIONS.iter().map(|(a, _)| ((*a).to_string(), c.hotkey(a))).collect()
        };
        self.hotkeys.borrow_mut().register(&list);
    }

    // -----------------------------------------------------------------
    // The Moa window (created on demand, destroyed when closed)

    pub fn toggle_app_window(self: &Rc<Self>) {
        let open = self.appwin.borrow().as_ref().map(|w| w.window().is_visible()).unwrap_or(false);
        if open {
            self.close_app_window();
        } else {
            self.open_app_window(None);
        }
    }

    pub fn open_app_window(self: &Rc<Self>, tab: Option<&str>) {
        if let Some(w) = self.appwin.borrow().as_ref() {
            if let Some(t) = tab {
                w.set_tab(t.into());
            }
            w.window().with_winit_window(|win| {
                win.set_minimized(false);
                win.focus_window();
            });
            return;
        }
        let w = AppWindow::new().expect("Moa window");
        self.wire_track(w.global::<crate::Track>(), w.global::<crate::I18n>());
        w.set_version(env!("CARGO_PKG_VERSION").into());
        w.set_ext_dir(extension_dir().display().to_string().into());
        w.set_guide(ModelRc::new(VecModel::from(crate::guide::items(&self.t.lang))));
        if let Some(t) = tab {
            w.set_tab(t.into());
        }
        let me = Rc::downgrade(self);
        w.on_win(move |what| {
            if let Some(app) = me.upgrade() {
                if what == "close" {
                    app.close_app_window();
                } else if let Some(w) = app.appwin.borrow().as_ref() {
                    w.window().with_winit_window(|win| win.set_minimized(true));
                }
            }
        });
        let me = Rc::downgrade(self);
        w.on_drag(move || {
            if let Some(app) = me.upgrade() {
                if let Some(w) = app.appwin.borrow().as_ref() {
                    w.window().with_winit_window(|win| {
                        let _ = win.drag_window();
                    });
                }
            }
        });
        let me = Rc::downgrade(self);
        w.on_action(move |a| {
            if let Some(app) = me.upgrade() {
                app.run_action(&a);
            }
        });
        w.on_open_url(|u| open_url(&u));
        let me = Rc::downgrade(self);
        w.on_set_bool(move |k, v| {
            if let Some(app) = me.upgrade() {
                app.set_setting(&k, serde_json::json!(v));
            }
        });
        let me = Rc::downgrade(self);
        w.on_set_num(move |k, v| {
            if let Some(app) = me.upgrade() {
                app.set_setting(&k, serde_json::json!(v));
            }
        });
        let me = Rc::downgrade(self);
        w.on_set_str(move |k, v| {
            if let Some(app) = me.upgrade() {
                app.set_setting(&k, serde_json::json!(v.as_str()));
            }
        });
        let me = Rc::downgrade(self);
        w.on_capture(move |action| {
            if let Some(app) = me.upgrade() {
                app.start_capture(action.to_string());
            }
        });
        let me = Rc::downgrade(self);
        w.on_key(move |text, ctrl, alt, shift, meta| me.upgrade().map(|app| app.capture_key(&text, ctrl, alt, shift, meta)).unwrap_or(false));
        let me = Rc::downgrade(self);
        w.on_spotify(move |what, arg| {
            if let Some(app) = me.upgrade() {
                crate::spotify::handle(&app, &what, &arg);
            }
        });
        let _ = w.show();
        *self.appwin.borrow_mut() = Some(w);
        // Fill the new window with the current state.
        let s = self.state.borrow().clone();
        self.apply_state(s);
        let art = self.art.borrow_mut().take();
        self.set_art(art);
        self.push_app();
    }

    fn close_app_window(self: &Rc<Self>) {
        if let Some(w) = self.appwin.borrow_mut().take() {
            let _ = w.hide();
        }
        self.end_capture();
        if !self.cfg.borrow().general.close_to_tray {
            let _ = slint::quit_event_loop();
        }
    }

    /// Updates a setting from the Moa window ("overlay.scale" = 1.2…).
    fn set_setting(self: &Rc<Self>, key: &str, v: serde_json::Value) {
        let mut json = serde_json::to_value(&*self.cfg.borrow()).unwrap_or_default();
        let (section, field) = key.split_once('.').unwrap_or((key, ""));
        let old = json[section][field].clone();
        if old == v {
            return;
        }
        json[section][field] = v.clone();
        if let Ok(c) = serde_json::from_value::<Config>(json) {
            *self.cfg.borrow_mut() = c;
        }
        self.save();
        match section {
            "overlay" => self.apply_overlay_config(),
            "playback" => {}
            "general" => match field {
                "language" | "gpu" => self.restart_note.set(true),
                "autostart" => platform::set_autostart(v.as_bool().unwrap_or(false)),
                "source" => {
                    if self.source() != "player" {
                        crate::player::close();
                    }
                    self.sync_state();
                    self.push_app();
                }
                "service" => crate::player::switch(v.as_str().unwrap_or("yandex")),
                _ => {}
            },
            _ => {}
        }
        self.push_app();
    }

    fn start_capture(&self, action: String) {
        *self.capturing.borrow_mut() = action;
        // While capturing, Moa's own hotkeys must not fire.
        self.hotkeys.borrow_mut().unregister();
        self.push_app();
    }

    fn end_capture(&self) {
        if !self.capturing.borrow().is_empty() {
            self.capturing.borrow_mut().clear();
            self.register_hotkeys();
        }
    }

    fn capture_key(self: &Rc<Self>, text: &str, ctrl: bool, alt: bool, shift: bool, meta: bool) -> bool {
        let action = self.capturing.borrow().clone();
        if action.is_empty() {
            return false;
        }
        let none = !(ctrl || alt || shift || meta);
        let key = crate::keys::key_name(text);
        match key.as_deref() {
            Some("Escape") if none => {
                self.end_capture();
                self.push_app();
                return true;
            }
            Some("Backspace") | Some("Delete") if none => {
                self.cfg.borrow_mut().hotkeys.insert(action, "".into());
                self.save();
                self.end_capture();
                self.hk_msg.borrow_mut().clear();
                self.push_app();
                return true;
            }
            None => return true, // a modifier on its own: keep waiting
            _ => {}
        }
        let key = key.unwrap();
        if none && !(key.starts_with('F') && key.len() > 1) {
            *self.hk_msg.borrow_mut() = self.t.t("hk.needModifier");
            self.push_app();
            return true;
        }
        let mut parts = Vec::new();
        if ctrl {
            parts.push("Ctrl".to_string());
        }
        if alt {
            parts.push("Alt".into());
        }
        if shift {
            parts.push("Shift".into());
        }
        if meta {
            parts.push("Super".into());
        }
        parts.push(key);
        let accel = parts.join("+");
        let owner = {
            let c = self.cfg.borrow();
            ACTIONS.iter().find(|(a, _)| *a != action && c.hotkey(a).eq_ignore_ascii_case(&accel)).map(|(a, _)| *a)
        };
        if let Some(o) = owner {
            *self.hk_msg.borrow_mut() = self.t.tv("hk.duplicate", &[("action", &self.t.t(&format!("action.{o}")))]);
            self.push_app();
            return true;
        }
        self.cfg.borrow_mut().hotkeys.insert(action, accel.into());
        self.save();
        self.hk_msg.borrow_mut().clear();
        self.end_capture();
        self.apply_overlay_config();
        self.push_app();
        true
    }

    /// Pushes settings and status into the Moa window, if it is open.
    pub fn push_app(&self) {
        let guard = self.appwin.borrow();
        let Some(w) = guard.as_ref() else { return };
        let c = self.cfg.borrow();
        w.set_cfg(Cfg {
            overlay_visible: c.overlay.visible,
            layout: c.overlay.layout.clone().into(),
            side: c.overlay.side.clone().into(),
            interaction: c.overlay.interaction.clone().into(),
            always_expanded: c.overlay.always_expanded,
            hide_idle: c.overlay.hide_when_idle,
            reactive: c.overlay.reactive,
            intensity: c.overlay.intensity,
            opacity: c.overlay.opacity,
            scale: c.overlay.scale,
            seek_step: c.playback.seek_step as f32,
            volume_step: c.playback.volume_step as f32,
            source: c.general.source.clone().into(),
            service: c.general.service.clone().into(),
            language: c.general.language.clone().into(),
            autostart: c.general.autostart,
            close_to_tray: c.general.close_to_tray,
            gpu: c.general.gpu,
        });
        let failed = self.hotkeys.borrow().failed.clone();
        let rows: Vec<HotkeyRow> = ACTIONS
            .iter()
            .map(|(a, _)| {
                let accel = c.hotkey(a);
                let keys: Vec<SharedString> = if accel.is_empty() { vec![] } else { accel.split('+').map(|k| crate::keys::pretty(k).into()).collect() };
                HotkeyRow {
                    action: (*a).into(),
                    label: self.t.t(&format!("action.{a}")).into(),
                    keys: ModelRc::new(VecModel::from(keys)),
                    failed: !accel.is_empty() && failed.contains(&accel),
                }
            })
            .collect();
        w.set_hotkeys(ModelRc::new(VecModel::from(rows)));
        w.set_capturing(self.capturing.borrow().clone().into());
        let mut msg = self.hk_msg.borrow().clone();
        if msg.is_empty() && !failed.is_empty() {
            let keys = failed.join(", ");
            let error = self.hotkeys.borrow().error.clone();
            msg = if let Some(e) = error {
                self.t.tv("hk.error", &[("keys", &keys), ("error", &e)])
            } else if platform::other_moa_running() {
                self.t.tv("hk.oldMoa", &[("keys", &keys)])
            } else {
                self.t.tv("hk.failed", &[("keys", &keys)])
            };
        }
        w.set_hk_msg(msg.into());
        w.set_restart_note(self.restart_note.get());
        w.set_diag_copied(self.diag_copied.get());

        let (ok, text) = match c.general.source.as_str() {
            "system" => {
                let (ready, err) = self.media_status.borrow().clone();
                let mut t = if ready {
                    self.t.t("status.system")
                } else if let Some(e) = err {
                    format!("{}: {e}", self.t.t("status.systemError"))
                } else {
                    self.t.t("status.starting")
                };
                if self.ext_connected.get() {
                    t += &format!(" · {}", self.t.t("status.extension"));
                }
                (ready, t)
            }
            "browser" => {
                if let Some(e) = self.ext_error.borrow().as_ref() {
                    (false, self.t.tv("status.connectorError", &[("error", e)]))
                } else if self.ext_connected.get() {
                    (true, self.t.t("status.extension"))
                } else {
                    (false, self.t.t("status.noExtension"))
                }
            }
            _ => (crate::player::is_open(), self.t.t("status.player")),
        };
        w.set_status_ok(ok);
        w.set_status_text(text.into());
        drop(c);
        crate::spotify::push(self, w);
    }

    pub fn strings(&self) -> &Strings {
        &self.t
    }

    pub fn config(&self) -> std::cell::RefMut<'_, Config> {
        self.cfg.borrow_mut()
    }

    pub fn save_config(&self) {
        self.save();
    }
}

fn source_badge(s: &TrackState, via_ext: bool, t: &Strings) -> (String, Image) {
    let load = |svg: &[u8]| Image::load_from_svg_data(svg).unwrap_or_default();
    if via_ext || s.app.is_empty() {
        if !s.has_track {
            return (String::new(), Image::default());
        }
        return ("YouTube Music".into(), load(include_bytes!("../ui/icons/b-youtubemusic.svg")));
    }
    let id = s.app.to_lowercase();
    let pick: (&str, &[u8]) = if id.contains("spotify") {
        ("Spotify", include_bytes!("../ui/icons/b-spotify.svg"))
    } else if id.contains("yandex") && id.contains("music") {
        ("", include_bytes!("../ui/icons/music.svg"))
    } else if id.contains("msedge") {
        ("Microsoft Edge", include_bytes!("../ui/icons/globe.svg"))
    } else if id.contains("chrome") {
        ("Chrome", include_bytes!("../ui/icons/b-googlechrome.svg"))
    } else if id.contains("zen") {
        ("Zen", include_bytes!("../ui/icons/b-zenbrowser.svg"))
    } else if id.contains("firefox") || id.contains("308046b0af4a39cb") {
        ("Firefox", include_bytes!("../ui/icons/b-firefoxbrowser.svg"))
    } else if id.contains("brave") {
        ("Brave", include_bytes!("../ui/icons/b-brave.svg"))
    } else if id.contains("opera") {
        ("Opera", include_bytes!("../ui/icons/b-opera.svg"))
    } else if id.contains("vivaldi") {
        ("Vivaldi", include_bytes!("../ui/icons/b-vivaldi.svg"))
    } else if id.contains("yandex") || id.contains("browser") {
        ("", include_bytes!("../ui/icons/globe.svg"))
    } else {
        ("?", include_bytes!("../ui/icons/music.svg"))
    };
    let name = match pick.0 {
        "" if id.contains("music") => t.t("home.yandex"),
        "" => t.t("home.yandexBrowser"),
        "?" => s.app.trim_end_matches(".exe").rsplit(['\\', '/', '!']).next().unwrap_or("").chars().take(28).collect(),
        n => n.to_string(),
    };
    (name, load(pick.1))
}

pub fn extension_dir() -> std::path::PathBuf {
    let exe = std::env::current_exe().unwrap_or_default();
    let dir = exe.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let packaged = dir.join("extension");
    if packaged.exists() {
        packaged
    } else {
        dir.join("../../../extension")
    }
}

pub fn open_url(url: &str) {
    if url.starts_with("https://") {
        open_path(std::path::Path::new(url));
    }
}

pub fn open_path(p: &std::path::Path) {
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(p).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(p).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(p).spawn();
}
