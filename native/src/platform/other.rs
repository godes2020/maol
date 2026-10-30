use super::Rect;
use slint::winit_030::WinitWindowAccessor;

pub fn locale() -> String {
    std::env::var("LANG").unwrap_or_default()
}

/// Work area of the monitor showing this window.
pub fn work_area(window: &slint::Window) -> Rect {
    window
        .with_winit_window(|w| {
            w.current_monitor().map(|m| {
                let p = m.position();
                let s = m.size();
                Rect { x: p.x, y: p.y, w: s.width as i32, h: s.height as i32 }
            })
        })
        .flatten()
        .unwrap_or(Rect { x: 0, y: 0, w: 1920, h: 1080 })
}

pub fn cursor_pos() -> Option<(i32, i32)> {
    None
}

pub fn set_overlay_keyed(_on: bool) {}

pub fn overlay_keyed() -> bool {
    false
}

pub fn prepare_overlay(window: &slint::Window) {
    window.with_winit_window(|w| w.set_window_level(slint::winit_030::winit::window::WindowLevel::AlwaysOnTop));
}

pub fn keep_on_top(_window: &slint::Window) {}

pub fn set_click_through(window: &slint::Window, on: bool) {
    window.with_winit_window(|w| {
        let _ = w.set_cursor_hittest(!on);
    });
}

pub fn set_autostart(_on: bool) {}

pub fn copy_text(text: &str) {
    println!("{text}");
}

pub fn other_moa_running() -> bool {
    false
}

/// Elsewhere the overlay draws its glow itself (the window has real alpha).
pub fn set_glow(_glow: Option<super::Glow>) {}

pub fn glow_owner(_window: &slint::Window) {}

pub fn set_overlay_shape(_window: &slint::Window, _parts: &[(super::Rect, i32)]) {}
