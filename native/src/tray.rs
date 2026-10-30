// Tray icon and menu (Windows).
use crate::app::{post, App};
use std::rc::Rc;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

pub struct Tray {
    _icon: TrayIcon,
}

impl Tray {
    pub fn new(app: &Rc<App>) -> Option<Self> {
        let t = app.strings();
        let menu = Menu::new();
        let open = MenuItem::with_id("open", t.t("tray.open"), true, None);
        let overlay = MenuItem::with_id("overlay", t.t("tray.overlay"), true, None);
        let edit = MenuItem::with_id("edit", t.t("tray.editOverlay"), true, None);
        let quit = MenuItem::with_id("quit", t.t("tray.quit"), true, None);
        menu.append_items(&[&open, &PredefinedMenuItem::separator(), &overlay, &edit, &PredefinedMenuItem::separator(), &quit]).ok()?;

        let png = image::load_from_memory(include_bytes!("../../assets/tray.png")).ok()?.to_rgba8();
        let (w, h) = png.dimensions();
        let icon = tray_icon::Icon::from_rgba(png.into_raw(), w, h).ok()?;
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_tooltip("Moa — Music Overlay App")
            .with_icon(icon)
            .build()
            .ok()?;

        MenuEvent::set_event_handler(Some(|e: MenuEvent| {
            let id = e.id.0.clone();
            post(move |app| match id.as_str() {
                "open" => app.open_app_window(None),
                "overlay" => app.run_action("toggleOverlay"),
                "edit" => app.run_action("editOverlay"),
                "quit" => {
                    let _ = slint::quit_event_loop();
                }
                _ => {}
            });
        }));
        TrayIconEvent::set_event_handler(Some(|e: TrayIconEvent| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                post(|app| app.open_app_window(None));
            }
        }));
        Some(Self { _icon: tray })
    }
}
