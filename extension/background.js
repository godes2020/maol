// Keeps a WebSocket to the Moa desktop app and routes player state from
// YouTube Music tabs to it, and hotkey commands back to the right tab.
const MOA_URL = 'ws://127.0.0.1:47823/moa';
const VERSION = chrome.runtime.getManifest().version;

const tabs = new Map(); // tabId -> { port, state, seenAt }
let ws = null;
let activeTab = null;

function connectMoa() {
  if (ws && (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING)) return;
  try {
    ws = new WebSocket(MOA_URL);
  } catch {
    ws = null;
    return;
  }
  ws.onopen = () => {
    send({ type: 'hello', version: VERSION });
    const t = tabs.get(activeTab);
    if (t && t.state) send({ type: 'state', payload: t.state });
  };
  ws.onmessage = (event) => {
    let msg;
    try {
      msg = JSON.parse(event.data);
    } catch {
      return;
    }
    if (msg.type === 'cmd') handleCommand(msg);
  };
  ws.onclose = () => {
    ws = null;
  };
  ws.onerror = () => {};
}

function send(obj) {
  if (ws && ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify(obj));
}

// Prefer the tab that is playing; otherwise the most recently seen one with a track.
function pickActive() {
  let best = null;
  let bestScore = -1;
  for (const [id, t] of tabs) {
    const s = t.state || {};
    const score = (s.playing ? 2e13 : 0) + (s.hasTrack ? 1e13 : 0) + t.seenAt;
    if (score > bestScore) {
      bestScore = score;
      best = id;
    }
  }
  activeTab = best;
}

async function handleCommand(msg) {
  const reply = (value) => send({ type: 'result', id: msg.id, value: value ?? null });
  if (msg.name === 'focusPlayer') {
    const t = tabs.get(activeTab);
    if (t) {
      const tab = await chrome.tabs.update(activeTab, { active: true });
      await chrome.windows.update(tab.windowId, { focused: true });
    } else {
      await chrome.tabs.create({ url: 'https://music.youtube.com/' });
    }
    return reply(true);
  }
  const t = tabs.get(activeTab);
  if (!t) return reply(null);
  t.pending = t.pending || new Map();
  t.pending.set(msg.id, reply);
  t.port.postMessage({ type: 'cmd', id: msg.id, name: msg.name, arg: msg.arg });
  setTimeout(() => {
    if (t.pending.delete(msg.id)) reply(null);
  }, 2000);
}

chrome.runtime.onConnect.addListener((port) => {
  if (port.name !== 'ytm' || !port.sender || !port.sender.tab) return;
  const tabId = port.sender.tab.id;
  const entry = { port, state: null, seenAt: Date.now(), pending: new Map() };
  tabs.set(tabId, entry);
  connectMoa();

  port.onMessage.addListener((msg) => {
    if (msg.type === 'result') {
      const cb = entry.pending.get(msg.payload.id);
      if (cb) {
        entry.pending.delete(msg.payload.id);
        cb(msg.payload.value);
      }
      return;
    }
    if (msg.type === 'state') {
      entry.state = msg.payload;
      entry.seenAt = Date.now();
      pickActive();
    }
    if (tabId !== activeTab) return;
    if (!ws) connectMoa();
    send({ type: msg.type, payload: msg.payload });
  });

  port.onDisconnect.addListener(() => {
    tabs.delete(tabId);
    if (activeTab === tabId) {
      pickActive();
      if (activeTab == null) send({ type: 'state', payload: { hasTrack: false, playing: false } });
    }
  });
});

chrome.runtime.onMessage.addListener((msg, _sender, sendResponse) => {
  if (msg && msg.type === 'status') {
    connectMoa();
    sendResponse({ moa: Boolean(ws && ws.readyState === WebSocket.OPEN), tabs: tabs.size });
  }
});
