// The built-in player window (WebView2) for Yandex Music, SoundCloud, VK and
// YouTube Music. Created only when opened: until then Moa carries no browser
// engine at all. Closing the window hides it and the music keeps playing; the
// view is freed when the source changes away from the player.
use crate::media::{Reply, TrackState};
use std::cell::RefCell;

pub fn url(service: &str) -> &'static str {
    match service {
        "soundcloud" => "https://soundcloud.com/",
        "vk" => "https://vk.com/audio",
        "youtube" => "https://music.youtube.com/",
        _ => "https://music.yandex.ru/",
    }
}

thread_local! {
    static STATE: RefCell<Option<TrackState>> = const { RefCell::new(None) };
}

/// The last track the player page reported.
pub fn state() -> Option<TrackState> {
    STATE.with(|s| s.borrow().clone())
}

#[cfg(not(windows))]
mod imp {
    use crate::media::Reply;
    pub fn open(service: &str) {
        crate::app::open_url(super::url(service));
    }
    pub fn switch(_service: &str) {}
    pub fn close() {}
    pub fn cmd(_name: &str, _arg: f64, reply: Reply) {
        reply(None);
    }
    pub fn is_open() -> bool {
        false
    }
}

#[cfg(windows)]
mod imp {
    use super::STATE;
    use crate::app;
    use crate::media::{Reply, TrackState};
    use crate::PlayerWindow;
    use slint::ComponentHandle;
    use std::cell::RefCell;
    use std::sync::Mutex;
    use wry::dpi::{LogicalPosition, LogicalSize};
    use wry::{Rect, WebContext, WebView, WebViewBuilder};

    struct Player {
        win: PlayerWindow,
        view: Option<WebView>,
        ctx: WebContext,
        service: String,
    }

    thread_local! {
        static PLAYER: RefCell<Option<Player>> = const { RefCell::new(None) };
    }

    fn bounds(w: f32, h: f32) -> Rect {
        Rect { position: LogicalPosition::new(0.0, 0.0).into(), size: LogicalSize::new(w as f64, h as f64).into() }
    }

    pub fn open(service: &str) {
        let exists = PLAYER.with(|p| {
            let p = p.borrow();
            let Some(p) = p.as_ref() else { return false };
            let _ = p.win.show();
            if let Some(v) = &p.view {
                let _ = v.focus();
            }
            true
        });
        if exists {
            switch(service);
            return;
        }
        let Ok(win) = PlayerWindow::new() else { return };
        win.on_resized(|w, h| {
            PLAYER.with(|p| {
                if let Some(v) = p.borrow().as_ref().and_then(|p| p.view.as_ref()) {
                    let _ = v.set_bounds(bounds(w, h));
                }
            })
        });
        if win.show().is_err() {
            return;
        }
        // Cookies and sign-ins live next to the settings, not next to the exe.
        let dir = crate::config::Config::file().parent().map(|d| d.join("WebView2"));
        PLAYER.with(|p| *p.borrow_mut() = Some(Player { win, view: None, ctx: WebContext::new(dir), service: service.to_string() }));
        attach(0);
    }

    /// The native window appears a moment after `show`; retry until it does.
    fn attach(tries: u32) {
        let done = PLAYER.with(|p| {
            let mut guard = p.borrow_mut();
            let Some(p) = guard.as_mut() else { return true };
            let handle = p.win.window().window_handle();
            let size = p.win.window().size().to_logical(p.win.window().scale_factor());
            let result = WebViewBuilder::new_with_web_context(&mut p.ctx)
                .with_url(super::url(&p.service))
                .with_initialization_script(include_str!("player-bridge.js"))
                .with_ipc_handler(|req| on_message(req.body()))
                .with_background_color((3, 3, 3, 255))
                .with_bounds(bounds(size.width, size.height))
                .build_as_child(&handle);
            match result {
                Ok(v) => {
                    p.view = Some(v);
                    true
                }
                Err(_) => false,
            }
        });
        if !done && tries < 20 {
            slint::Timer::single_shot(std::time::Duration::from_millis(100), move || attach(tries + 1));
        }
    }

    fn on_message(body: &str) {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(body) else { return };
        if v["type"] == "state" {
            if let Ok(s) = serde_json::from_value::<TrackState>(v["payload"].clone()) {
                STATE.with(|st| *st.borrow_mut() = Some(s));
                app::post(|a| a.sync_state());
            }
        }
    }

    pub fn switch(service: &str) {
        PLAYER.with(|p| {
            let mut guard = p.borrow_mut();
            let Some(p) = guard.as_mut() else { return };
            if p.service != service {
                p.service = service.to_string();
                if let Some(v) = &p.view {
                    let _ = v.load_url(super::url(service));
                }
            }
        });
    }

    /// Frees the browser engine (the source moved away from the player).
    pub fn close() {
        let p = PLAYER.with(|p| p.borrow_mut().take());
        if let Some(p) = p {
            let _ = p.win.hide();
            drop(p.view);
        }
        STATE.with(|s| *s.borrow_mut() = None);
    }

    pub fn cmd(name: &str, arg: f64, reply: Reply) {
        let js = format!(
            "window.__moa ? window.__moa.cmd({}, {}) : null",
            serde_json::to_string(name).unwrap_or_default(),
            if arg.is_finite() { arg } else { 0.0 }
        );
        let reply = Mutex::new(Some(reply));
        PLAYER.with(|p| {
            if let Some(v) = p.borrow().as_ref().and_then(|p| p.view.as_ref()) {
                let _ = v.evaluate_script_with_callback(&js, move |out| {
                    if let Some(r) = reply.lock().ok().and_then(|mut r| r.take()) {
                        r(serde_json::from_str(&out).ok().filter(|v: &serde_json::Value| !v.is_null()));
                    }
                });
            } else if let Some(r) = reply.lock().ok().and_then(|mut r| r.take()) {
                r(None);
            }
        });
    }

    pub fn is_open() -> bool {
        PLAYER.with(|p| p.borrow().as_ref().map(|p| p.win.window().is_visible()).unwrap_or(false))
    }
}

pub use imp::{close, is_open, open, switch};

/// Runs a playback command in the player page; `reply` gets its result.
pub fn cmd(name: &str, arg: f64, reply: Reply) {
    imp::cmd(name, arg, reply)
}
