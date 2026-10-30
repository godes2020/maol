// Turning key presses in the Keys tab into accelerators ("Alt+Shift+K").

/// Accelerator key name for a Slint key event's text, or None for a bare
/// modifier. Letters and digits come from the physical key on Windows, so
/// they work with any keyboard layout (Alt+Shift+K stays K on a Russian layout).
pub fn key_name(text: &str) -> Option<String> {
    let c = text.chars().next()?;
    let named = match c {
        '\u{0008}' => Some("Backspace"),
        '\u{0009}' => Some("Tab"),
        '\u{000a}' => Some("Enter"),
        '\u{001b}' => Some("Escape"),
        '\u{007f}' => Some("Delete"),
        ' ' => Some("Space"),
        '\u{F700}' => Some("Up"),
        '\u{F701}' => Some("Down"),
        '\u{F702}' => Some("Left"),
        '\u{F703}' => Some("Right"),
        '\u{F727}' => Some("Insert"),
        '\u{F729}' => Some("Home"),
        '\u{F72B}' => Some("End"),
        '\u{F72C}' => Some("PageUp"),
        '\u{F72D}' => Some("PageDown"),
        _ => None,
    };
    if let Some(n) = named {
        return Some(n.into());
    }
    if ('\u{F704}'..='\u{F71B}').contains(&c) {
        return Some(format!("F{}", c as u32 - 0xF704 + 1));
    }
    if ('\u{0010}'..='\u{0018}').contains(&c) {
        return None; // Shift, Control, Alt, Meta…
    }
    if let Some(k) = physical_key() {
        return Some(k);
    }
    let up = c.to_ascii_uppercase();
    match up {
        'A'..='Z' | '0'..='9' => Some(up.to_string()),
        '-' | '=' | '[' | ']' | ';' | '\'' | '`' | '\\' | ',' | '.' | '/' => Some(up.to_string()),
        _ => None,
    }
}

#[cfg(windows)]
fn physical_key() -> Option<String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    let down = |vk: i32| unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 };
    for vk in 0x41..=0x5A {
        if down(vk) {
            return Some(((vk as u8) as char).to_string());
        }
    }
    for vk in 0x30..=0x39 {
        if down(vk) {
            return Some(((vk as u8) as char).to_string());
        }
    }
    for vk in 0x60..=0x69 {
        if down(vk) {
            return Some(format!("num{}", vk - 0x60));
        }
    }
    let oem = [
        (0xBD, "-"),
        (0xBB, "="),
        (0xDB, "["),
        (0xDD, "]"),
        (0xBA, ";"),
        (0xDE, "'"),
        (0xC0, "`"),
        (0xDC, "\\"),
        (0xBC, ","),
        (0xBE, "."),
        (0xBF, "/"),
        (0x6B, "numadd"),
        (0x6D, "numsub"),
        (0x6A, "nummult"),
        (0x6F, "numdiv"),
        (0x6E, "numdec"),
    ];
    oem.iter().find(|(vk, _)| down(*vk)).map(|(_, n)| n.to_string())
}

#[cfg(not(windows))]
fn physical_key() -> Option<String> {
    None
}

/// Keycap label.
pub fn pretty(k: &str) -> String {
    match k {
        "Up" => "↑".into(),
        "Down" => "↓".into(),
        "Left" => "←".into(),
        "Right" => "→".into(),
        "Super" => "Win".into(),
        "Space" => "␣".into(),
        other if other.starts_with("num") && other.len() > 3 => format!("Num {}", &other[3..]),
        other => other.into(),
    }
}
