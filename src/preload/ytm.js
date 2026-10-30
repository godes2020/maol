// Preload for the YouTube Music window. Forwards messages posted by the
// injected main-world bridge (src/inject/ytm-bridge.js) to the main process.
const { ipcRenderer } = require('electron');

window.addEventListener('message', (event) => {
  if (event.source !== window) return;
  const data = event.data;
  if (!data || data.__moa !== 1) return;
  if (data.type === 'state') ipcRenderer.send('moa:state', data.payload);
  else if (data.type === 'audio') ipcRenderer.send('moa:audio', data.payload);
});
