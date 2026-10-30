const { contextBridge, ipcRenderer } = require('electron');

const listen = (channel) => (cb) => {
  const handler = (_e, data) => cb(data);
  ipcRenderer.on(channel, handler);
  return () => ipcRenderer.removeListener(channel, handler);
};

contextBridge.exposeInMainWorld('moa', {
  onState: listen('overlay:state'),
  onArt: listen('overlay:art'),
  onAudio: listen('overlay:audio'),
  onConfig: listen('overlay:config'),
  onEditMode: listen('overlay:edit-mode'),
  onToast: listen('overlay:toast'),
  command: (name, arg) => ipcRenderer.send('moa:command', name, arg),
  setInteractive: (on) => ipcRenderer.send('overlay:interactive', Boolean(on)),
  ready: () => ipcRenderer.send('overlay:ready')
});
