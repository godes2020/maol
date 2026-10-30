// Moa — Music Overlay App.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

slint::include_modules!();

mod app;
mod art;
mod config;
mod connector;
mod guide;
mod hotkeys;
mod i18n;
mod keys;
mod media;
mod platform;
mod player;
mod spotify;
#[cfg(windows)]
mod tray;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

/// One Moa at a time: a second start asks the running one to open its window.
const INSTANCE_PORT: u16 = 47824;

fn main() {
    if let Ok(mut s) = TcpStream::connect(("127.0.0.1", INSTANCE_PORT)) {
        let _ = s.write_all(b"moa:show");
        return;
    }
    let listener = TcpListener::bind(("127.0.0.1", INSTANCE_PORT)).ok();

    // Always on the CPU: a GPU surface ignores the overlay's colour key, so
    // with hardware acceleration the overlay showed a black box around it.
    if std::env::var_os("SLINT_BACKEND").is_none() {
        let renderer = if cfg!(feature = "skia") { "skia-software" } else { "software" };
        let _ = slint::BackendSelector::new().backend_name("winit".into()).renderer_name(renderer.into()).select();
    }
    // Neither renderer gives the overlay real alpha on Windows, so the
    // background colour is keyed out instead.
    platform::set_overlay_keyed(true);

    let hidden = std::env::args().any(|a| a == "--hidden");
    let app = app::App::start(hidden);
    #[cfg(windows)]
    let _tray = tray::Tray::new(&app);

    if let Some(l) = listener {
        std::thread::spawn(move || {
            for mut s in l.incoming().flatten() {
                let mut buf = [0u8; 16];
                if s.read(&mut buf).map(|n| &buf[..n] == b"moa:show").unwrap_or(false) {
                    app::post(|a| a.open_app_window(None));
                }
            }
        });
    }

    slint::run_event_loop_until_quit().expect("event loop");
    drop(app);
}
