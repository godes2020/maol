const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('moa', {
  getInfo: () => ipcRenderer.invoke('settings:get'),
  patch: (partial) => ipcRenderer.invoke('settings:patch', partial),
  resetHotkeys: () => ipcRenderer.invoke('settings:reset-hotkeys'),
  suspendHotkeys: (on) => ipcRenderer.send('settings:suspend-hotkeys', Boolean(on)),
  editOverlay: () => ipcRenderer.send('moa:action', 'editOverlay'),
  resetOverlayPosition: () => ipcRenderer.send('settings:reset-overlay-position'),
  openYtm: () => ipcRenderer.send('settings:open-ytm'),
  openExtensionFolder: () => ipcRenderer.send('settings:open-extension-folder'),
  openGuide: () => ipcRenderer.send('settings:open-guide'),
  onInfo: (cb) => ipcRenderer.on('settings:info', (_e, info) => cb(info))
});
