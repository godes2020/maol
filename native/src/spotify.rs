// Spotify account: official sign-in through the browser (OAuth with PKCE).
//
// The user brings their own Client ID from developer.spotify.com with the
// redirect URI below. Moa opens the Spotify sign-in page in the default
// browser, catches the redirect on a loopback port and keeps only the
// refresh token (no client secret, no passwords, no cookies).
use crate::app::{self, App};
use crate::AppWindow;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine;
use std::cell::RefCell;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const PORT: u16 = 47825;
const REDIRECT: &str = "http://127.0.0.1:47825/callback";
const SCOPES: &str = "user-read-private playlist-read-private user-read-playback-state user-modify-playback-state";
/// How long the loopback port waits for the browser.
const WAIT: Duration = Duration::from_secs(300);

/// Bumped on every sign-in; an older listener sees it and quits.
static ATTEMPT: AtomicU64 = AtomicU64::new(0);

#[derive(Default)]
struct State {
    /// Display name of the signed-in account (fetched once per run).
    name: String,
    /// Status line under the account row.
    msg: String,
    access: Option<(String, Instant)>,
    profile_asked: bool,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

pub fn handle(app: &Rc<App>, what: &str, arg: &str) {
    match what {
        "login" => login(app, arg.trim()),
        "logout" => {
            ATTEMPT.fetch_add(1, Ordering::SeqCst);
            app.config().spotify.refresh_token.clear();
            app.save_config();
            STATE.with(|s| *s.borrow_mut() = State::default());
            app.push_app();
        }
        _ => {}
    }
}

pub fn push(app: &App, w: &AppWindow) {
    let (client_id, token) = {
        let c = app.config();
        (c.spotify.client_id.clone(), c.spotify.refresh_token.clone())
    };
    let connected = !token.is_empty();
    let (name, msg, ask) = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let ask = connected && !s.profile_asked;
        s.profile_asked |= ask;
        (s.name.clone(), s.msg.clone(), ask)
    });
    w.set_spotify_connected(connected);
    w.set_spotify_name(if name.is_empty() { app.strings().t("sp.connected") } else { name }.into());
    w.set_spotify_client_id(client_id.clone().into());
    w.set_spotify_msg(msg.into());
    if ask {
        std::thread::spawn(move || fetch_profile(client_id, token));
    }
}

fn set_msg(app: &App, msg: String) {
    STATE.with(|s| s.borrow_mut().msg = msg);
    app.push_app();
}

fn login(app: &Rc<App>, client_id: &str) {
    if client_id.len() < 16 || !client_id.chars().all(|c| c.is_ascii_alphanumeric()) {
        set_msg(app, app.strings().tv("sp.failed", &[("error", "Client ID")]));
        return;
    }
    app.config().spotify.client_id = client_id.to_string();
    app.save_config();

    let (Some(verifier), Some(state)) = (random_token(64), random_token(16)) else {
        set_msg(app, app.strings().tv("sp.failed", &[("error", "no random source")]));
        return;
    };
    let challenge = B64.encode(sha256(verifier.as_bytes()));
    let attempt = ATTEMPT.fetch_add(1, Ordering::SeqCst) + 1;

    let listener = match bind() {
        Ok(l) => l,
        Err(e) => {
            set_msg(app, app.strings().tv("sp.failed", &[("error", &format!("127.0.0.1:{PORT}: {e}"))]));
            return;
        }
    };
    let url = format!(
        "https://accounts.spotify.com/authorize?response_type=code&client_id={client_id}&scope={}&redirect_uri={}&state={state}&code_challenge_method=S256&code_challenge={challenge}",
        encode(SCOPES),
        encode(REDIRECT)
    );
    app::open_url(&url);
    set_msg(app, app.strings().t("sp.waiting"));

    let client_id = client_id.to_string();
    std::thread::spawn(move || {
        let result = wait_for_code(listener, attempt, &state).and_then(|code| exchange(&client_id, &code, &verifier));
        if ATTEMPT.load(Ordering::SeqCst) != attempt {
            return;
        }
        app::post(move |app| match result {
            Ok(tokens) => {
                app.config().spotify.refresh_token = tokens.refresh;
                app.save_config();
                STATE.with(|s| {
                    let mut s = s.borrow_mut();
                    *s = State::default();
                    s.access = Some((tokens.access, tokens.expires));
                });
                app.push_app();
            }
            Err(e) => set_msg(app, app.strings().tv("sp.failed", &[("error", &e)])),
        });
    });
}

/// The port may still be held for a moment by an attempt that is quitting.
fn bind() -> std::io::Result<TcpListener> {
    let mut tries = 0;
    loop {
        match TcpListener::bind(("127.0.0.1", PORT)) {
            Ok(l) => {
                l.set_nonblocking(true)?;
                return Ok(l);
            }
            Err(_) if tries < 10 => {
                tries += 1;
                std::thread::sleep(Duration::from_millis(150));
            }
            Err(e) => return Err(e),
        }
    }
}

fn wait_for_code(listener: TcpListener, attempt: u64, state: &str) -> Result<String, String> {
    let deadline = Instant::now() + WAIT;
    while Instant::now() < deadline {
        if ATTEMPT.load(Ordering::SeqCst) != attempt {
            return Err("cancelled".into());
        }
        match listener.accept() {
            Ok((stream, _)) => {
                if let Some(r) = serve(stream, state) {
                    return r;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("timed out".into())
}

/// Answers one browser request. `None` for anything that is not the callback
/// (a favicon request, say).
fn serve(mut stream: TcpStream, state: &str) -> Option<Result<String, String>> {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf).ok()?;
    let req = String::from_utf8_lossy(&buf[..n]);
    let target = req.lines().next()?.split(' ').nth(1)?.to_string();
    let Some(query) = target.strip_prefix("/callback?") else {
        let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        return None;
    };
    let param = |k: &str| query.split('&').find_map(|p| p.strip_prefix(&format!("{k}="))).map(decode);
    let result = if param("state").as_deref() != Some(state) {
        Err("state mismatch".to_string())
    } else if let Some(code) = param("code") {
        Ok(code)
    } else {
        Err(param("error").unwrap_or_else(|| "no code".into()))
    };
    let (title, text) = match &result {
        Ok(_) => ("Moa", "Signed in. You can close this tab and return to Moa. / Вход выполнен, вкладку можно закрыть."),
        Err(_) => ("Moa", "Sign-in failed. Return to Moa and try again. / Не удалось войти, вернись в Moa и попробуй ещё раз."),
    };
    let body = format!(
        "<!doctype html><meta charset=utf-8><title>{title}</title><body style=\"font:16px system-ui;background:#111;color:#eee;display:grid;place-items:center;height:90vh\"><p>{text}</p>"
    );
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    Some(result)
}

struct Tokens {
    access: String,
    refresh: String,
    expires: Instant,
}

fn exchange(client_id: &str, code: &str, verifier: &str) -> Result<Tokens, String> {
    token_request(
        &[("grant_type", "authorization_code"), ("code", code), ("redirect_uri", REDIRECT), ("client_id", client_id), ("code_verifier", verifier)],
        None,
    )
}

fn refresh(client_id: &str, refresh_token: &str) -> Result<Tokens, String> {
    token_request(&[("grant_type", "refresh_token"), ("refresh_token", refresh_token), ("client_id", client_id)], Some(refresh_token))
}

fn token_request(form: &[(&str, &str)], old_refresh: Option<&str>) -> Result<Tokens, String> {
    let mut resp = ureq::post("https://accounts.spotify.com/api/token").send_form(form.iter().copied()).map_err(|e| e.to_string())?;
    let v: serde_json::Value = resp.body_mut().read_json().map_err(|e| e.to_string())?;
    let access = v["access_token"].as_str().ok_or("no access token")?.to_string();
    // Spotify may or may not rotate the refresh token.
    let refresh = v["refresh_token"].as_str().map(str::to_string).or(old_refresh.map(str::to_string)).ok_or("no refresh token")?;
    let secs = v["expires_in"].as_u64().unwrap_or(3600).saturating_sub(60);
    Ok(Tokens { access, refresh, expires: Instant::now() + Duration::from_secs(secs) })
}

/// Background: refresh the access token and read the account's display name.
fn fetch_profile(client_id: String, refresh_token: String) {
    let result = refresh(&client_id, &refresh_token).and_then(|t| {
        let mut resp = ureq::get("https://api.spotify.com/v1/me")
            .header("Authorization", &format!("Bearer {}", t.access))
            .call()
            .map_err(|e| e.to_string())?;
        let v: serde_json::Value = resp.body_mut().read_json().map_err(|e| e.to_string())?;
        let name = v["display_name"].as_str().filter(|s| !s.is_empty()).or(v["id"].as_str()).unwrap_or_default().to_string();
        Ok((t, name))
    });
    app::post(move |app| {
        // Signed out (or in again) while this was running.
        if app.config().spotify.refresh_token != refresh_token {
            return;
        }
        match result {
            Ok((t, name)) => {
                if t.refresh != refresh_token {
                    app.config().spotify.refresh_token = t.refresh.clone();
                    app.save_config();
                }
                STATE.with(|s| {
                    let mut s = s.borrow_mut();
                    s.name = name;
                    s.msg.clear();
                    s.access = Some((t.access, t.expires));
                });
            }
            Err(e) => STATE.with(|s| s.borrow_mut().msg = app.strings().tv("sp.failed", &[("error", &e)])),
        }
        app.push_app();
    });
}

fn random_token(bytes: usize) -> Option<String> {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).ok()?;
    Some(B64.encode(buf))
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                match u8::from_str_radix(&s[i + 1..i + 3], 16) {
                    Ok(v) => {
                        out.push(v);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// SHA-256 (FIPS 180-4), only for the PKCE code challenge.
fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
        0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7].wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [t1.wrapping_add(t2), v[0], v[1], v[2], v[3].wrapping_add(t1), v[4], v[5], v[6]];
        }
        for (a, b) in h.iter_mut().zip(v) {
            *a = a.wrapping_add(b);
        }
    }
    let mut out = [0u8; 32];
    for (i, x) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&x.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_vectors() {
        let hex = |b: [u8; 32]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
        assert_eq!(hex(sha256(b"")), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(hex(sha256(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn pkce_challenge() {
        // Cross-checked with Python's hashlib + urlsafe_b64encode.
        let c = B64.encode(sha256(b"dBjftJeZ4CVP-mB92K27uhbUJU9p1r_wW1gFWFOEjXk"));
        assert_eq!(c, "ESvXuZCi8KjdUICqbhM2WaYnWmB_hiSzVC7RvpGAC5c");
    }

    #[test]
    fn url_decode() {
        assert_eq!(decode("a%20b+c"), "a b c");
        assert_eq!(decode("100%"), "100%");
    }
}
