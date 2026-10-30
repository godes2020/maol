use super::Rect;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::WinitWindowAccessor;
use windows::core::{HSTRING, PCWSTR};
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::{COLORREF, HANDLE, HWND, POINT};
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

/// Set at startup: true when the overlay is drawn on the CPU. That path
/// presents through GDI without alpha, so the overlay instead paints its empty
/// area with `KEY` and Windows cuts that colour out.
static KEYED: AtomicBool = AtomicBool::new(false);
/// RGB(1, 2, 3): close to black, so anti-aliased card edges blend into it
/// invisibly, and practically never present in a cover.
const KEY: COLORREF = COLORREF(0x0003_0201);
static CLICK_THROUGH: AtomicBool = AtomicBool::new(false);

pub fn set_overlay_keyed(on: bool) {
    KEYED.store(on, Ordering::Relaxed);
}

pub fn overlay_keyed() -> bool {
    KEYED.load(Ordering::Relaxed)
}

/// The overlay never shows in the taskbar or Alt+Tab and never steals focus
/// from the game.
pub fn prepare_overlay(window: &slint::Window) {
    use slint::winit_030::winit::platform::windows::WindowExtWindows;
    window.with_winit_window(|w| {
        w.set_window_level(slint::winit_030::winit::window::WindowLevel::AlwaysOnTop);
        w.set_skip_taskbar(true);
    });
    if let Some(h) = hwnd(window) {
        guard_style(h);
    }
    keep_on_top(window);
}

/// The extended style the overlay must have, given what is there now.
fn wanted_ex(ex: isize) -> isize {
    let mut want = (ex | WS_EX_TOOLWINDOW.0 as isize | WS_EX_NOACTIVATE.0 as isize) & !(WS_EX_APPWINDOW.0 as isize);
    want &= !(WS_EX_TRANSPARENT.0 as isize | WS_EX_LAYERED.0 as isize);
    if CLICK_THROUGH.load(Ordering::Relaxed) {
        want |= WS_EX_TRANSPARENT.0 as isize | WS_EX_LAYERED.0 as isize;
    }
    if overlay_keyed() {
        want |= WS_EX_LAYERED.0 as isize;
    }
    want
}

/// Our extended styles. winit rewrites the whole extended style whenever one
/// of its own flags changes (show, hide, focus, window level), which drops
/// WS_EX_LAYERED and with it the colour key: the overlay turned black. The
/// overlay's window procedure now vetoes that (`overlay_proc`); this puts
/// the styles and the key back should they still go missing.
fn apply_style(h: HWND) {
    unsafe {
        let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
        let want = wanted_ex(ex);
        if want != ex {
            SetWindowLongPtrW(h, GWL_EXSTYLE, want);
        }
        if overlay_keyed() {
            let (mut key, mut alpha, mut flags) = (COLORREF(0), 0u8, LAYERED_WINDOW_ATTRIBUTES_FLAGS(0));
            let ok = GetLayeredWindowAttributes(h, Some(&mut key), Some(&mut alpha), Some(&mut flags)).is_ok();
            if want != ex || !ok || key != KEY || flags.0 & LWA_COLORKEY.0 == 0 {
                let _ = SetLayeredWindowAttributes(h, KEY, 255, LWA_COLORKEY);
                use windows::Win32::Graphics::Gdi::{RedrawWindow, RDW_ALLCHILDREN, RDW_FRAME, RDW_INVALIDATE};
                let _ = RedrawWindow(Some(h), None, None, RDW_INVALIDATE | RDW_FRAME | RDW_ALLCHILDREN);
            }
        }
    }
}

static OLD_PROC: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

/// Sits in front of winit's window procedure and keeps our extended styles
/// in every style change winit makes.
unsafe extern "system" fn overlay_proc(h: HWND, msg: u32, wp: windows::Win32::Foundation::WPARAM, lp: windows::Win32::Foundation::LPARAM) -> windows::Win32::Foundation::LRESULT {
    if msg == WM_STYLECHANGING && wp.0 as i32 == GWL_EXSTYLE.0 && lp.0 != 0 {
        let ss = &mut *(lp.0 as *mut STYLESTRUCT);
        ss.styleNew = wanted_ex(ss.styleNew as isize) as u32;
    }
    let old: WNDPROC = std::mem::transmute::<isize, WNDPROC>(OLD_PROC.load(Ordering::Relaxed));
    CallWindowProcW(old, h, msg, wp, lp)
}

fn guard_style(h: HWND) {
    if OLD_PROC.load(Ordering::Relaxed) != 0 {
        return;
    }
    unsafe {
        let old = SetWindowLongPtrW(h, GWLP_WNDPROC, overlay_proc as *const () as isize);
        OLD_PROC.store(old, Ordering::Relaxed);
    }
}

/// Called often (every pointer check) so a lost colour key never lasts.
pub fn ensure_overlay_style(window: &slint::Window) {
    if let Some(h) = hwnd(window) {
        apply_style(h);
    }
}

/// Games that go borderless-fullscreen can push other topmost windows down;
/// called now and then to stay above them.
pub fn keep_on_top(window: &slint::Window) {
    if let Some(h) = hwnd(window) {
        apply_style(h);
        unsafe {
            let _ = SetWindowPos(h, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER);
        }
    }
}

/// Done here rather than through winit, whose version drops the layered
/// style (and with it the colour key) when the mouse is let back in.
pub fn set_click_through(window: &slint::Window, on: bool) {
    CLICK_THROUGH.store(on, Ordering::Relaxed);
    if let Some(h) = hwnd(window) {
        apply_style(h);
    }
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

/// True when another Moa.exe runs: the Electron Moa 0.3 uses the same
/// default hotkeys and keeps them while it sits in the tray.
pub fn other_moa_running() -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::ProcessStatus::EnumProcesses;
    use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
    let own = std::process::id();
    let mut pids = vec![0u32; 4096];
    let mut bytes = 0u32;
    unsafe {
        if EnumProcesses(pids.as_mut_ptr(), (pids.len() * 4) as u32, &mut bytes).is_err() {
            return false;
        }
        for &pid in &pids[..bytes as usize / 4] {
            if pid == 0 || pid == own {
                continue;
            }
            let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { continue };
            let mut buf = [0u16; 520];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, windows::core::PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
            let _ = CloseHandle(h);
            if ok {
                let path = String::from_utf16_lossy(&buf[..len as usize]);
                let name = path.rsplit('\\').next().unwrap_or("").to_ascii_lowercase();
                if name == "moa.exe" {
                    return true;
                }
            }
        }
    }
    false
}
