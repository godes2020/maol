// Local WebSocket server for the optional Moa Connector browser extension.
// Only extension origins on this machine are accepted.
use crate::media::{AudioFrame, TrackState};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tungstenite::Message;

pub const PORT: u16 = 47823;

pub enum ExtEvent {
    Connected(bool, Option<String>),
    State(TrackState),
    Audio(AudioFrame),
    Result(u64, serde_json::Value),
    Error(String),
}

#[derive(Clone)]
pub struct Connector {
    // Commands for the newest connected extension.
    out: Arc<Mutex<Option<(u64, Sender<String>)>>>,
    next: Arc<AtomicU64>,
}

impl Connector {
    pub fn start(on_event: impl Fn(ExtEvent) + Send + Sync + 'static) -> Self {
        let out: Arc<Mutex<Option<(u64, Sender<String>)>>> = Default::default();
        let on_event = Arc::new(on_event);
        let o = out.clone();
        std::thread::Builder::new()
            .name("moa-connector".into())
            .spawn(move || {
                let listener = match TcpListener::bind(("127.0.0.1", PORT)) {
                    Ok(l) => l,
                    Err(e) => {
                        on_event(ExtEvent::Error(format!("port {PORT}: {e}")));
                        return;
                    }
                };
                let mut conn = 0u64;
                for stream in listener.incoming().flatten() {
                    conn += 1;
                    let me = conn;
                    let (tx, rx) = channel();
                    let ev = on_event.clone();
                    let o2 = o.clone();
                    std::thread::spawn(move || {
                        if !serve(stream, rx, &*ev, || {
                            // The newest connection wins (a browser restart replaces the old one).
                            *o2.lock().unwrap() = Some((me, tx.clone()));
                        }) {
                            return;
                        }
                        let mut g = o2.lock().unwrap();
                        if g.as_ref().map(|(id, _)| *id == me).unwrap_or(false) {
                            *g = None;
                            drop(g);
                            ev(ExtEvent::Connected(false, None));
                        }
                    });
                }
            })
            .expect("connector thread");
        Self { out, next: Arc::new(AtomicU64::new(1)) }
    }

    /// Sends a command; the extension answers with a Result event carrying the id.
    pub fn cmd(&self, name: &str, arg: f64) -> Option<u64> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let msg = serde_json::json!({ "type": "cmd", "id": id, "name": name, "arg": arg }).to_string();
        let g = self.out.lock().unwrap();
        g.as_ref()?.1.send(msg).ok()?;
        Some(id)
    }
}

/// Returns false if the handshake failed (not an extension).
fn serve(stream: TcpStream, rx: Receiver<String>, on_event: &(dyn Fn(ExtEvent) + Send + Sync), connected: impl FnOnce()) -> bool {
    let check = |req: &Request, resp: Response| -> Result<Response, ErrorResponse> {
        let origin = req.headers().get("origin").and_then(|o| o.to_str().ok()).unwrap_or("");
        let ok = req.uri().path() == "/moa" && (origin.starts_with("chrome-extension://") || origin.starts_with("moz-extension://"));
        if ok {
            Ok(resp)
        } else {
            let mut r = ErrorResponse::new(None);
            *r.status_mut() = tungstenite::http::StatusCode::FORBIDDEN;
            Err(r)
        }
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(50)));
    let Ok(mut ws) = tungstenite::accept_hdr(stream, check) else { return false };
    connected();
    on_event(ExtEvent::Connected(true, None));
    loop {
        loop {
            match rx.try_recv() {
                Ok(m) => {
                    if ws.send(Message::text(m)).is_err() {
                        return true;
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                // Replaced by a newer connection.
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    let _ = ws.close(None);
                    return true;
                }
            }
        }
        match ws.read() {
            Ok(Message::Text(t)) => handle(&t, on_event),
            Ok(Message::Close(_)) => return true,
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            Err(_) => return true,
        }
    }
}

fn handle(text: &str, on_event: &(dyn Fn(ExtEvent) + Send + Sync)) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else { return };
    match v.get("type").and_then(|t| t.as_str()) {
        Some("hello") => on_event(ExtEvent::Connected(true, v.get("version").and_then(|x| x.as_str()).map(String::from))),
        Some("state") => {
            if let Ok(s) = serde_json::from_value(v["payload"].clone()) {
                on_event(ExtEvent::State(s));
            }
        }
        Some("audio") => {
            if let Ok(a) = serde_json::from_value(v["payload"].clone()) {
                on_event(ExtEvent::Audio(a));
            }
        }
        Some("result") => {
            if let Some(id) = v.get("id").and_then(|x| x.as_u64()) {
                on_event(ExtEvent::Result(id, v.get("value").cloned().unwrap_or_default()));
            }
        }
        _ => {}
    }
}
