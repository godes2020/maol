// OS integration: window styles for the overlay, screen geometry, autostart,
// clipboard, locale. Windows gets the real thing; elsewhere sensible stand-ins.
#[cfg(windows)]
mod win;
#[cfg(windows)]
pub use win::*;

#[cfg(not(windows))]
mod other;
#[cfg(not(windows))]
pub use other::*;

/// A rectangle in physical screen pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
}

/// What the glow under the overlay should look like.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glow {
    /// The overlay window, physical screen pixels.
    pub win: Rect,
    /// The card inside it, physical pixels relative to the window.
    pub card: Rect,
    pub radius: i32,
    pub c1: (u8, u8, u8),
    pub c2: (u8, u8, u8),
    /// 0..1, drives the layer's opacity every frame.
    pub strength: f32,
}
