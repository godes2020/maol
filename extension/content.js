// Isolated-world relay between the page bridge (ytm-bridge.js) and the
// extension's background worker.
let port = null;

function connect() {
  port = chrome.runtime.connect({ name: 'ytm' });
  port.onMessage.addListener((msg) => {
    if (msg && msg.type === 'cmd') window.postMessage({ __moa: 2, type: 'cmd', id: msg.id, name: msg.name, arg: msg.arg }, '*');
  });
  port.onDisconnect.addListener(() => {
    port = null;
    setTimeout(connect, 1000);
  });
}
connect();

window.addEventListener('message', (event) => {
  if (event.source !== window) return;
  const d = event.data;
  if (!d || d.__moa !== 1 || !port) return;
  try {
    port.postMessage({ type: d.type, payload: d.payload });
  } catch {
    // Worker restarting; the next tick reconnects.
  }
});
