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
fn apply_style(h: HWND) -> bool {
    let mut fixed = false;
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
                fixed = true;
            }
        }
    }
    fixed
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
/// True when the style or key had to be put back: the window must then be
/// repainted in full.
pub fn ensure_overlay_style(window: &slint::Window) -> bool {
    hwnd(window).map(apply_style).unwrap_or(false)
}

/// Games that go borderless-fullscreen can push other topmost windows down;
/// called now and then to stay above them.
pub fn keep_on_top(window: &slint::Window) {
    if let Some(h) = hwnd(window) {
        let _ = apply_style(h);
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

// ---------------------------------------------------------------------------
// Glow under the overlay
//
// The colour key gives the overlay only fully clear or fully solid pixels, so
// its soft glow lives in a second window right below it: a layered window
// with real per-pixel alpha, painted here once per shape and faded with the
// beat through its overall opacity.

struct GlowWin {
    hwnd: HWND,
    dc: windows::Win32::Graphics::Gdi::HDC,
    bitmap: windows::Win32::Graphics::Gdi::HBITMAP,
    /// Shape the bitmap holds (strength and position left out).
    shape: Option<super::Glow>,
    shown: Option<(i32, i32, u8)>,
}

thread_local! {
    static GLOW: std::cell::RefCell<Option<GlowWin>> = const { std::cell::RefCell::new(None) };
    static OVERLAY: std::cell::Cell<Option<isize>> = const { std::cell::Cell::new(None) };
}

/// Remembers the overlay window the glow sits under.
pub fn glow_owner(window: &slint::Window) {
    OVERLAY.with(|o| o.set(hwnd(window).map(|h| h.0 as isize)));
}

unsafe extern "system" fn glow_proc(h: HWND, msg: u32, w: windows::Win32::Foundation::WPARAM, l: windows::Win32::Foundation::LPARAM) -> windows::Win32::Foundation::LRESULT {
    unsafe { DefWindowProcW(h, msg, w, l) }
}

fn create_glow() -> Option<GlowWin> {
    use windows::Win32::Graphics::Gdi::{CreateCompatibleDC, HBITMAP};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    unsafe {
        let inst = GetModuleHandleW(None).ok()?;
        let class = windows::core::w!("MoaGlow");
        let wc = WNDCLASSW { lpfnWndProc: Some(glow_proc), hInstance: inst.into(), lpszClassName: class, ..Default::default() };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
            class,
            windows::core::w!("Moa Glow"),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(inst.into()),
            None,
        )
        .ok()?;
        let dc = CreateCompatibleDC(None);
        Some(GlowWin { hwnd, dc, bitmap: HBITMAP::default(), shape: None, shown: None })
    }
}

/// Paints the glow into a premultiplied BGRA bitmap: full colour at the card
/// edge, fading out over the padding around it.
fn paint_glow(g: &mut GlowWin, s: &super::Glow) {
    use windows::Win32::Graphics::Gdi::*;
    let (w, h) = (s.win.w.max(1), s.win.h.max(1));
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
    unsafe {
        let Ok(bmp) = CreateDIBSection(Some(g.dc), &info, DIB_RGB_COLORS, &mut bits, None, 0) else { return };
        if bits.is_null() {
            let _ = DeleteObject(bmp.into());
            return;
        }
        let px = std::slice::from_raw_parts_mut(bits as *mut u32, (w * h) as usize);
        let c = s.card;
        let r = s.radius as f32;
        // Room between the card and the window edge.
        let reach = (c.x.min(c.y).min(s.win.w - c.x - c.w).min(s.win.h - c.y - c.h)).max(4) as f32;
        let (cx, cy) = (c.x as f32 + c.w as f32 / 2.0, c.y as f32 + c.h as f32 / 2.0 + reach * 0.1);
        let (hx, hy) = (c.w as f32 / 2.0 - r, c.h as f32 / 2.0 - r);
        let cy0 = c.y as f32 + c.h as f32 / 2.0;
        for y in 0..h {
            for x in 0..w {
                // Distance from the card's rounded rectangle.
                let dx = ((x as f32 + 0.5 - cx).abs() - hx).max(0.0);
                let dy = ((y as f32 + 0.5 - cy).abs() - hy).max(0.0);
                let d = ((dx * dx + dy * dy).sqrt() - r).max(0.0);
                let t = (d / reach).min(1.0);
                // Nothing under the card itself: should the glow end up above
                // the overlay it must not wash the card out.
                let ex = ((x as f32 + 0.5 - cx).abs() - hx).max(0.0);
                let ey = ((y as f32 + 0.5 - cy0).abs() - hy).max(0.0);
                let outside = ((ex * ex + ey * ey).sqrt() - r + 0.5).clamp(0.0, 1.0);
                let a = 0.85 * (1.0 - t) * (1.0 - t) * outside;
                let mix = |p: u8, q: u8| p as f32 + (q as f32 - p as f32) * t;
                let (rr, gg, bb) = (mix(s.c1.0, s.c2.0), mix(s.c1.1, s.c2.1), mix(s.c1.2, s.c2.2));
                let pa = |v: f32| (v * a).round() as u32;
                px[(y * w + x) as usize] = ((a * 255.0).round() as u32) << 24 | pa(rr) << 16 | pa(gg) << 8 | pa(bb);
            }
        }
        SelectObject(g.dc, bmp.into());
        if !g.bitmap.is_invalid() {
            let _ = DeleteObject(g.bitmap.into());
        }
        g.bitmap = bmp;
    }
    g.shape = Some(super::Glow { strength: 0.0, win: super::Rect { x: 0, y: 0, ..s.win }, ..*s });
    g.shown = None;
}

/// Shows, moves and fades the glow; `None` hides it.
pub fn set_glow(glow: Option<super::Glow>) {
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::{AC_SRC_ALPHA, AC_SRC_OVER, BLENDFUNCTION};
    GLOW.with(|cell| {
        let mut cell = cell.borrow_mut();
        let Some(s) = glow else {
            if let Some(g) = cell.as_mut() {
                if g.shown.take().is_some() {
                    unsafe {
                        let _ = ShowWindow(g.hwnd, SW_HIDE);
                    }
                }
            }
            return;
        };
        if cell.is_none() {
            *cell = create_glow();
        }
        let Some(g) = cell.as_mut() else { return };
        let shape = super::Glow { strength: 0.0, win: super::Rect { x: 0, y: 0, ..s.win }, ..s };
        if g.shape != Some(shape) {
            paint_glow(g, &s);
        }
        let alpha = (s.strength.clamp(0.0, 1.0) * 255.0).round() as u8;
        let was_hidden = g.shown.is_none();
        if g.shown == Some((s.win.x, s.win.y, alpha)) {
            return;
        }
        g.shown = Some((s.win.x, s.win.y, alpha));
        let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: alpha, AlphaFormat: AC_SRC_ALPHA as u8 };
        let dst = POINT { x: s.win.x, y: s.win.y };
        let size = SIZE { cx: s.win.w, cy: s.win.h };
        let src = POINT::default();
        unsafe {
            let _ = UpdateLayeredWindow(g.hwnd, None, Some(&dst), Some(&size), Some(g.dc), Some(&src), COLORREF(0), Some(&blend), ULW_ALPHA);
            // Just under the overlay, above everything else.
            let after = OVERLAY.with(|o| o.get()).map(|h| HWND(h as *mut _)).unwrap_or(HWND_TOPMOST);
            let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER;
            let _ = SetWindowPos(g.hwnd, Some(after), 0, 0, 0, 0, if was_hidden { flags | SWP_SHOWWINDOW } else { flags });
            // Lift the overlay back over it, in case it had lost its topmost
            // place and the glow went on top.
            if after != HWND_TOPMOST {
                let _ = SetWindowPos(after, Some(HWND_TOPMOST), 0, 0, 0, 0, flags);
            }
        }
    });
}
