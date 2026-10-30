fn main() {
    slint_build::compile("ui/main.slint").unwrap();
    // The exe icon (Explorer, taskbar, Alt+Tab).
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/moa.ico");
        res.set("FileDescription", "Moa — Music Overlay App");
        res.set("ProductName", "Moa");
        res.compile().expect("Windows resources");
    }
}
