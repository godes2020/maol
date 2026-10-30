// The built-in player window (WebView2) for Yandex Music, SoundCloud, VK and
// YouTube Music. Created only when opened.
pub fn open(_service: &str) {}
pub fn switch(_service: &str) {}
pub fn cmd(_name: &str, _arg: f64) {}
pub fn is_open() -> bool {
    false
}
