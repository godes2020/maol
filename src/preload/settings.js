const { contextBridge, ipcRenderer } = require('electron');

const listen = (channel) => (cb) => ipcRenderer.on(channel, (_e, data) => cb(data));

contextBridge.exposeInMainWorld('moa', {
  getInfo: () => ipcRenderer.invoke('settings:get'),
  getState: () => ipcRenderer.invoke('app:state'),
  patch: (partial) => ipcRenderer.invoke('settings:patch', partial),
  resetHotkeys: () => ipcRenderer.invoke('settings:reset-hotkeys'),
  suspendHotkeys: (on) => ipcRenderer.send('settings:suspend-hotkeys', Boolean(on)),
  command: (name, arg) => ipcRenderer.send('moa:command', name, arg),
  action: (name) => ipcRenderer.send('moa:action', name),
  resetOverlayPosition: () => ipcRenderer.send('settings:reset-overlay-position'),
  openYtm: () => ipcRenderer.send('settings:open-ytm'),
  openUrl: (url) => ipcRenderer.send('settings:open-url', url),
  openExtensionFolder: () => ipcRenderer.send('settings:open-extension-folder'),
  openGuide: () => ipcRenderer.send('settings:open-guide'),
  window: (what) => ipcRenderer.send('app:window', what),
  onInfo: listen('settings:info'),
  onState: listen('app:state'),
  onArt: listen('app:art'),
  onTab: listen('app:tab')
});
