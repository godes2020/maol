use super::Rect;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::WinitWindowAccessor;
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{HANDLE, HWND, POINT};
use windows::Win32::Globalization::GetUserDefaultLocaleName;
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Registry::{RegDeleteKeyValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ};
use windows::Win32::UI::WindowsAndMessaging::*;

pub fn locale() -> String {
    let mut buf = [0u16; 85];
    let n = unsafe { GetUserDefaultLocaleName(&mut buf) };
    String::from_utf16_lossy(&buf[..(n.max(1) - 1) as usize])
}

fn hwnd(window: &slint::Window) -> Option<HWND> {
    window
        .with_winit_window(|w| match w.window_handle().ok()?.as_raw() {
            RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as *mut _)),
            _ => None,
        })
        .flatten()
}

/// Work area (screen minus taskbar) of the monitor showing this window.
pub fn work_area(window: &slint::Window) -> Rect {
    let p = window.position();
    let s = window.size();
    let center = POINT { x: p.x + s.width as i32 / 2, y: p.y + s.height as i32 / 2 };
    unsafe {
        let mon = MonitorFromPoint(center, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if GetMonitorInfoW(mon, &mut info).as_bool() {
            let r = info.rcWork;
            return Rect { x: r.left, y: r.top, w: r.right - r.left, h: r.bottom - r.top };
        }
    }
    Rect { x: 0, y: 0, w: 1920, h: 1040 }
}

pub fn cursor_pos() -> Option<(i32, i32)> {
    let mut p = POINT::default();
    unsafe { GetCursorPos(&mut p).ok()? };
    Some((p.x, p.y))
}

/// The overlay never shows in the taskbar or Alt+Tab and never steals focus
/// from the game.
pub fn prepare_overlay(window: &slint::Window) {
    window.with_winit_window(|w| w.set_window_level(slint::winit_030::winit::window::WindowLevel::AlwaysOnTop));
    if let Some(h) = hwnd(window) {
        unsafe {
            let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
            let ex = (ex | WS_EX_TOOLWINDOW.0 as isize | WS_EX_NOACTIVATE.0 as isize) & !(WS_EX_APPWINDOW.0 as isize);
            SetWindowLongPtrW(h, GWL_EXSTYLE, ex);
        }
    }
    keep_on_top(window);
}

/// Games that go borderless-fullscreen can push other topmost windows down;
/// called now and then to stay above them.
pub fn keep_on_top(window: &slint::Window) {
    if let Some(h) = hwnd(window) {
        unsafe {
            let _ = SetWindowPos(h, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER);
        }
    }
}

pub fn set_click_through(window: &slint::Window, on: bool) {
    window.with_winit_window(|w| {
        let _ = w.set_cursor_hittest(!on);
    });
}

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

pub fn set_autostart(on: bool) {
    let key = HSTRING::from(RUN_KEY);
    let name = HSTRING::from("Moa");
    unsafe {
        if on {
            let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
            let value: Vec<u16> = format!("\"{exe}\" --hidden").encode_utf16().chain(Some(0)).collect();
            let _ = RegSetKeyValueW(
                HKEY_CURRENT_USER,
                PCWSTR(key.as_ptr()),
                PCWSTR(name.as_ptr()),
                REG_SZ.0,
                Some(value.as_ptr() as *const _),
                (value.len() * 2) as u32,
            );
        } else {
            let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, PCWSTR(key.as_ptr()), PCWSTR(name.as_ptr()));
        }
    }
}

pub fn copy_text(text: &str) {
    const CF_UNICODETEXT: u32 = 13;
    let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    unsafe {
        if OpenClipboard(None).is_err() {
            return;
        }
        let _ = EmptyClipboard();
        if let Ok(mem) = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2) {
            let ptr = GlobalLock(mem) as *mut u16;
            if !ptr.is_null() {
                std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
                let _ = GlobalUnlock(mem);
                let _ = SetClipboardData(CF_UNICODETEXT, Some(HANDLE(mem.0)));
            }
        }
        let _ = CloseClipboard();
    }
}
