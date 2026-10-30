const path = require('path');
const fs = require('fs');
const {
  app,
  BrowserWindow,
  Menu,
  Tray,
  globalShortcut,
  ipcMain,
  nativeImage,
  net,
  screen,
  shell
} = require('electron');
const { Config, DEFAULT_HOTKEYS } = require('./config');
const { Connector } = require('./connector');
const i18n = require('../shared/i18n');

const YTM_URL = 'https://music.youtube.com/';
const ROOT = path.join(__dirname, '..', '..');
const ICON = path.join(ROOT, 'assets', 'icon.png');
const TRAY_ICON = path.join(ROOT, 'assets', 'tray.png');
const GUIDE_URL = {
  en: 'https://github.com/godes2020/maol/blob/HEAD/docs/INSTALL.md#3-connect-your-browser',
  ru: 'https://github.com/godes2020/maol/blob/HEAD/docs/INSTALL.ru.md#3-подключи-браузер'
};

// YouTube's cookie dialog can end up with dark text on a dark background
// inside the app window; force light, readable text there.
const CONSENT_PAGE_CSS = `
  html, body { background: #121212 !important; }
  body, body * { color: #ececec !important; }
  a, a * { color: #8ab4f8 !important; }
  button, [role="button"], input[type="submit"] {
    background-color: #2d2d2d !important;
    border: 1px solid #5f6368 !important;
  }
`;
const CONSENT_DIALOG_CSS = `
  ytd-consent-bump-v2-lightbox, ytmusic-consent-bump, tp-yt-paper-dialog:has([class*="consent"]) {
    --yt-spec-text-primary: #ececec !important;
    --yt-spec-text-secondary: #bdbdbd !important;
  }
  ytd-consent-bump-v2-lightbox *, ytmusic-consent-bump *, tp-yt-paper-dialog:has([class*="consent"]) * {
    color: #ececec !important;
  }
  ytd-consent-bump-v2-lightbox tp-yt-paper-dialog, ytmusic-consent-bump tp-yt-paper-dialog,
  tp-yt-paper-dialog:has([class*="consent"]) {
    background: #212121 !important;
  }
  ytd-consent-bump-v2-lightbox button, ytmusic-consent-bump button,
  tp-yt-paper-dialog:has([class*="consent"]) button {
    background: #3a3a3a !important;
    border: 1px solid #5f6368 !important;
  }
  ytd-consent-bump-v2-lightbox a, ytmusic-consent-bump a { color: #8ab4f8 !important; }
`;

// Overlay card size in CSS pixels (before scaling) plus room for the glow.
const OVERLAY_W = 300;
const OVERLAY_H = 140;
const OVERLAY_PAD = 18;

if (!app.requestSingleInstanceLock()) {
  app.quit();
  process.exit(0);
}

app.setAppUserModelId('io.github.godes2020.moa');

let config;
let t = (k) => k;
let mainWin = null;
let overlayWin = null;
let settingsWin = null;
let tray = null;
let quitting = false;
let editMode = false;
let hotkeysSuspended = false;
let failedHotkeys = [];
let lastState = null;
let lastArtUrl = null;
const artCache = new Map();
const connector = new Connector();

// ---------------------------------------------------------------------------
// Windows

function isBrowserMode() {
  return config.get().general.source !== 'app';
}

function createMainWindow(showOnReady = false) {
  const w = config.get().window;
  mainWin = new BrowserWindow({
    width: w.width,
    height: w.height,
    x: w.x ?? undefined,
    y: w.y ?? undefined,
    minWidth: 360,
    minHeight: 480,
    title: 'Moa',
    icon: ICON,
    backgroundColor: '#030303',
    autoHideMenuBar: true,
    show: false,
    webPreferences: {
      preload: path.join(ROOT, 'src', 'preload', 'ytm.js'),
      contextIsolation: true,
      sandbox: true,
      backgroundThrottling: false,
      autoplayPolicy: 'no-user-gesture-required'
    }
  });

  const bridge = fs.readFileSync(path.join(ROOT, 'extension', 'ytm-bridge.js'), 'utf8');
  mainWin.webContents.on('dom-ready', () => {
    const wc = mainWin.webContents;
    const host = safeHost(wc.getURL());
    if (host === 'music.youtube.com') {
      wc.executeJavaScript(bridge).catch(() => {});
      wc.insertCSS(CONSENT_DIALOG_CSS).catch(() => {});
    } else if (/^consent\.(youtube|google)\.com$/.test(host)) {
      wc.insertCSS(CONSENT_PAGE_CSS).catch(() => {});
    }
  });

  mainWin.webContents.setWindowOpenHandler(({ url }) => {
    const host = safeHost(url);
    if (/(^|\.)(google\.com|youtube\.com)$/.test(host)) return { action: 'allow' };
    shell.openExternal(url);
    return { action: 'deny' };
  });

  mainWin.once('ready-to-show', () => {
    // Like Discord, Moa lives in the tray by default; the window is opt-in.
    const show = config.get().general.showWindow && !process.argv.includes('--hidden');
    if (show || showOnReady) mainWin.show();
  });

  const saveBounds = () => {
    if (!mainWin || mainWin.isMinimized() || mainWin.isMaximized()) return;
    const b = mainWin.getBounds();
    config.patch({ window: { width: b.width, height: b.height, x: b.x, y: b.y } });
  };
  mainWin.on('resized', saveBounds);
  mainWin.on('moved', saveBounds);

  mainWin.on('close', (e) => {
    if (quitting) return;
    if (config.get().general.closeToTray) {
      e.preventDefault();
      mainWin.hide();
    } else {
      quit();
    }
  });
  mainWin.on('closed', () => {
    mainWin = null;
  });

  mainWin.loadURL(YTM_URL);
}

function overlayBounds() {
  const o = config.get().overlay;
  const scale = clamp(Number(o.scale) || 1, 0.6, 2);
  const width = Math.round((OVERLAY_W + OVERLAY_PAD * 2) * scale);
  const height = Math.round((OVERLAY_H + OVERLAY_PAD * 2) * scale);
  const area = screen.getPrimaryDisplay().workArea;
  let x = o.x ?? area.x + 12;
  let y = o.y ?? area.y + 12;
  // Bring the overlay back if its saved position is off every display.
  const visible = screen.getAllDisplays().some((d) => {
    const b = d.bounds;
    return x + width > b.x + 20 && x < b.x + b.width - 20 && y + height > b.y + 20 && y < b.y + b.height - 20;
  });
  if (!visible) {
    x = area.x + 12;
    y = area.y + 12;
  }
  return { x, y, width, height, scale };
}

function createOverlayWindow() {
  const b = overlayBounds();
  overlayWin = new BrowserWindow({
    x: b.x,
    y: b.y,
    width: b.width,
    height: b.height,
    frame: false,
    transparent: true,
    resizable: false,
    movable: true,
    minimizable: false,
    maximizable: false,
    fullscreenable: false,
    skipTaskbar: true,
    focusable: false,
    hasShadow: false,
    alwaysOnTop: true,
    show: false,
    title: 'Moa Overlay',
    webPreferences: {
      preload: path.join(ROOT, 'src', 'preload', 'overlay.js'),
      contextIsolation: true,
      sandbox: true,
      backgroundThrottling: false
    }
  });
  overlayWin.setAlwaysOnTop(true, 'screen-saver');
  overlayWin.setVisibleOnAllWorkspaces(true, { visibleOnFullScreen: true });
  overlayWin.setIgnoreMouseEvents(true, { forward: true });
  overlayWin.webContents.setZoomFactor(b.scale);

  overlayWin.on('moved', () => {
    if (!overlayWin) return;
    const [x, y] = overlayWin.getPosition();
    config.patch({ overlay: { x, y } });
  });
  overlayWin.on('closed', () => {
    overlayWin = null;
  });

  overlayWin.loadFile(path.join(ROOT, 'src', 'overlay', 'overlay.html'), {
    query: { lang: currentLang() }
  });
}

function applyOverlayConfig() {
  if (!overlayWin) return;
  const b = overlayBounds();
  const [w, h] = overlayWin.getSize();
  if (w !== b.width || h !== b.height) overlayWin.setBounds({ x: b.x, y: b.y, width: b.width, height: b.height });
  overlayWin.webContents.setZoomFactor(b.scale);
  sendOverlay('overlay:config', config.get().overlay);
  updateOverlayVisibility();
}

function updateOverlayVisibility() {
  if (!overlayWin) return;
  const o = config.get().overlay;
  const idle = !lastState || !lastState.hasTrack;
  const show = editMode || (o.visible && !(o.hideWhenIdle && idle));
  if (show && !overlayWin.isVisible()) {
    overlayWin.showInactive();
    overlayWin.setAlwaysOnTop(true, 'screen-saver');
  } else if (!show && overlayWin.isVisible()) {
    overlayWin.hide();
  }
}

function setEditMode(on) {
  editMode = on;
  if (!overlayWin) return;
  overlayWin.setIgnoreMouseEvents(!on, { forward: true });
  overlayWin.setFocusable(on);
  sendOverlay('overlay:edit-mode', on);
  updateOverlayVisibility();
  if (on) overlayWin.focus();
  rebuildTrayMenu();
}

function createSettingsWindow(tab) {
  if (settingsWin) {
    settingsWin.show();
    settingsWin.focus();
    return;
  }
  settingsWin = new BrowserWindow({
    width: 560,
    height: 720,
    minWidth: 460,
    minHeight: 480,
    title: t('settings.title'),
    icon: ICON,
    backgroundColor: '#0d0b14',
    autoHideMenuBar: true,
    webPreferences: {
      preload: path.join(ROOT, 'src', 'preload', 'settings.js'),
      contextIsolation: true,
      sandbox: true
    }
  });
  settingsWin.setMenu(null);
  settingsWin.webContents.setWindowOpenHandler(({ url }) => {
    if (url.startsWith('https://')) shell.openExternal(url);
    return { action: 'deny' };
  });
  settingsWin.on('closed', () => {
    settingsWin = null;
    hotkeysSuspended = false;
    registerHotkeys();
  });
  settingsWin.loadFile(path.join(ROOT, 'src', 'settings', 'settings.html'), {
    query: { lang: currentLang(), tab: typeof tab === 'string' ? tab : '' }
  });
}

function toggleMainWindow() {
  if (isBrowserMode()) return showMainWindow();
  if (!mainWin) return;
  if (mainWin.isVisible() && !mainWin.isMinimized() && mainWin.isFocused()) {
    mainWin.hide();
  } else {
    showMainWindow();
  }
}

// In browser mode "the Moa window" is the YouTube Music tab in the browser.
function showMainWindow() {
  if (isBrowserMode()) {
    if (connector.connected) connector.cmd('focusPlayer');
    else shell.openExternal(YTM_URL);
    return;
  }
  if (!mainWin) return createMainWindow(true);
  if (mainWin.isMinimized()) mainWin.restore();
  mainWin.show();
  mainWin.focus();
}

// ---------------------------------------------------------------------------
// Tray & menu

function rebuildTrayMenu() {
  if (!tray) return;
  const o = config.get().overlay;
  tray.setContextMenu(
    Menu.buildFromTemplate([
      { label: isBrowserMode() ? t('tray.openPlayer') : t('tray.show'), click: showMainWindow },
      { type: 'separator' },
      {
        label: t('tray.overlay'),
        type: 'checkbox',
        checked: o.visible,
        click: () => runAction('toggleOverlay')
      },
      { label: t('tray.editOverlay'), type: 'checkbox', checked: editMode, click: () => runAction('editOverlay') },
      { label: t('tray.settings'), click: createSettingsWindow },
      { type: 'separator' },
      { label: t('tray.quit'), click: quit }
    ])
  );
}

function createTray() {
  const img = nativeImage.createFromPath(TRAY_ICON);
  tray = new Tray(img.isEmpty() ? nativeImage.createFromPath(ICON).resize({ width: 16, height: 16 }) : img);
  tray.setToolTip('Moa — Music Overlay App');
  tray.on('click', showMainWindow);
  tray.on('double-click', showMainWindow);
  rebuildTrayMenu();
}

function createAppMenu() {
  const menu = Menu.buildFromTemplate([
    {
      label: t('menu.app'),
      submenu: [
        { label: t('tray.settings'), accelerator: 'CmdOrCtrl+,', click: createSettingsWindow },
        { label: t('tray.editOverlay'), click: () => runAction('editOverlay') },
        { type: 'separator' },
        { label: t('menu.back'), accelerator: 'Alt+Left', click: () => mainWin && mainWin.webContents.navigationHistory.goBack() },
        { label: t('menu.forward'), accelerator: 'Alt+Right', click: () => mainWin && mainWin.webContents.navigationHistory.goForward() },
        { label: t('menu.reload'), accelerator: 'CmdOrCtrl+R', click: () => mainWin && mainWin.webContents.reload() },
        { label: t('menu.devtools'), accelerator: 'CmdOrCtrl+Shift+I', click: () => mainWin && mainWin.webContents.toggleDevTools() },
        { type: 'separator' },
        { label: t('tray.quit'), accelerator: 'CmdOrCtrl+Q', click: quit }
      ]
    }
  ]);
  Menu.setApplicationMenu(menu);
}

// ---------------------------------------------------------------------------
// Player commands & hotkeys

function ytm(name, arg) {
  if (isBrowserMode()) return connector.cmd(name, arg);
  if (!mainWin) return Promise.resolve(null);
  const code = `window.__moa ? window.__moa.cmd(${JSON.stringify(name)}, ${JSON.stringify(arg ?? null)}) : null`;
  return mainWin.webContents.executeJavaScript(code).catch(() => null);
}

function toast(text) {
  sendOverlay('overlay:toast', text);
}

const REPEAT_NEXT = { NONE: 'ALL', ALL: 'ONE', ONE: 'NONE' };
const REPEAT_LABEL = { NONE: 'overlay.repeatOff', ALL: 'overlay.repeatAll', ONE: 'overlay.repeatOne' };

const actions = {
  playPause: () => ytm('playPause'),
  next: () => ytm('next'),
  previous: () => ytm('previous'),
  seekForward: () => ytm('seekBy', config.get().playback.seekStep),
  seekBackward: () => ytm('seekBy', -config.get().playback.seekStep),
  volumeUp: async () => {
    const v = await ytm('volumeBy', config.get().playback.volumeStep);
    if (v != null) toast(`${t('overlay.volume')} ${v}%`);
  },
  volumeDown: async () => {
    const v = await ytm('volumeBy', -config.get().playback.volumeStep);
    if (v != null) toast(`${t('overlay.volume')} ${v}%`);
  },
  toggleMute: async () => {
    await ytm('toggleMute');
    const muted = lastState ? !lastState.muted : true;
    toast(muted ? t('overlay.muted') : `${t('overlay.volume')} ${lastState ? lastState.volume : ''}%`);
  },
  cycleRepeat: async () => {
    await ytm('cycleRepeat');
    const next = REPEAT_NEXT[(lastState && lastState.repeat) || 'NONE'] || 'NONE';
    toast(t(REPEAT_LABEL[next]));
  },
  toggleOverlay: () => {
    const visible = !config.get().overlay.visible;
    config.patch({ overlay: { visible } });
    if (!visible && editMode) setEditMode(false);
    applyOverlayConfig();
    rebuildTrayMenu();
  },
  editOverlay: () => setEditMode(!editMode),
  toggleLock: () => {
    const interaction = config.get().overlay.interaction === 'locked' ? 'hover' : 'locked';
    config.patch({ overlay: { interaction } });
    applyOverlayConfig();
    pushSettingsInfo();
    toast(t(interaction === 'locked' ? 'overlay.locked' : 'overlay.unlocked'));
  },
  showApp: toggleMainWindow,
  openSettings: createSettingsWindow
};

function runAction(name, arg) {
  const fn = actions[name];
  if (fn) return fn(arg);
  // Direct player commands from the overlay (e.g. seekTo from the progress bar).
  if (['seekTo', 'setVolume'].includes(name)) return ytm(name, arg);
  return null;
}

function registerHotkeys() {
  globalShortcut.unregisterAll();
  failedHotkeys = [];
  if (hotkeysSuspended) return;
  const keys = config.get().hotkeys;
  for (const [action, accel] of Object.entries(keys)) {
    if (!accel || !actions[action]) continue;
    try {
      if (!globalShortcut.register(accel, () => runAction(action))) failedHotkeys.push(accel);
    } catch {
      failedHotkeys.push(accel);
    }
  }
}

// ---------------------------------------------------------------------------
// State relay

function sendOverlay(channel, data) {
  if (overlayWin && !overlayWin.isDestroyed()) overlayWin.webContents.send(channel, data);
}

async function fetchArt(url) {
  if (!url) return null;
  if (artCache.has(url)) return artCache.get(url);
  try {
    const res = await net.fetch(url);
    if (!res.ok) return null;
    const buf = Buffer.from(await res.arrayBuffer());
    const type = res.headers.get('content-type') || 'image/jpeg';
    const dataUrl = `data:${type};base64,${buf.toString('base64')}`;
    artCache.set(url, dataUrl);
    if (artCache.size > 30) artCache.delete(artCache.keys().next().value);
    return dataUrl;
  } catch {
    return null;
  }
}

function extensionDir() {
  return app.isPackaged ? path.join(process.resourcesPath, 'extension') : path.join(ROOT, 'extension');
}

function settingsInfo() {
  return {
    connector: { connected: connector.connected, version: connector.version, error: connector.error },
    extensionDir: extensionDir(),
    config: config.get(),
    defaults: DEFAULT_HOTKEYS,
    failed: failedHotkeys,
    version: app.getVersion(),
    lang: currentLang()
  };
}

function setupIpc() {
  ipcMain.on('moa:state', (e, state) => {
    if (!isBrowserMode() && mainWin && e.sender === mainWin.webContents) handleState(state);
  });
  ipcMain.on('moa:audio', (e, audio) => {
    if (!isBrowserMode() && mainWin && e.sender === mainWin.webContents) sendOverlay('overlay:audio', audio);
  });

  connector.on('state', (state) => {
    if (isBrowserMode()) handleState(state);
  });
  connector.on('audio', (audio) => {
    if (isBrowserMode()) sendOverlay('overlay:audio', audio);
  });
  connector.on('status', pushSettingsInfo);

  ipcMain.on('settings:open-ytm', () => shell.openExternal(YTM_URL));
  ipcMain.on('settings:open-extension-folder', () => shell.openPath(extensionDir()));
  ipcMain.on('settings:open-guide', () => shell.openExternal(GUIDE_URL[currentLang()] || GUIDE_URL.en));

  ipcMain.on('moa:command', (_e, name, arg) => runAction(name, arg));
  ipcMain.on('moa:action', (_e, name) => runAction(name));

  ipcMain.on('overlay:ready', () => {
    sendOverlay('overlay:config', config.get().overlay);
    sendOverlay('overlay:edit-mode', editMode);
    if (lastState) sendOverlay('overlay:state', lastState);
    if (lastArtUrl) fetchArt(lastArtUrl).then((art) => sendOverlay('overlay:art', art));
    updateOverlayVisibility();
  });

  ipcMain.on('overlay:interactive', (_e, on) => {
    if (!overlayWin || editMode) return;
    if (on && config.get().overlay.interaction === 'locked') return;
    overlayWin.setIgnoreMouseEvents(!on, { forward: true });
  });

  ipcMain.handle('settings:get', () => settingsInfo());

  ipcMain.handle('settings:patch', (_e, partial) => {
    const prevLang = config.get().general.language;
    config.patch(partial || {});
    const c = config.get();
    if (partial && partial.hotkeys) registerHotkeys();
    if (partial && partial.overlay) {
      applyOverlayConfig();
      rebuildTrayMenu();
    }
    if (partial && partial.general) {
      applyAutostart();
      if (c.general.language !== prevLang) config.flush();
      if ('source' in partial.general) applySource();
    }
    return settingsInfo();
  });

  ipcMain.handle('settings:reset-hotkeys', () => {
    config.patch({ hotkeys: DEFAULT_HOTKEYS });
    registerHotkeys();
    return settingsInfo();
  });

  ipcMain.on('settings:suspend-hotkeys', (_e, on) => {
    hotkeysSuspended = on;
    registerHotkeys();
  });

  ipcMain.on('settings:reset-overlay-position', () => {
    config.patch({ overlay: { x: null, y: null } });
    if (overlayWin) {
      const b = overlayBounds();
      overlayWin.setPosition(b.x, b.y);
    }
  });
}

async function handleState(state) {
  const hadTrack = lastState && lastState.hasTrack;
  lastState = state;
  sendOverlay('overlay:state', state);
  if (Boolean(hadTrack) !== Boolean(state.hasTrack)) updateOverlayVisibility();
  if (state.artwork !== lastArtUrl) {
    lastArtUrl = state.artwork;
    const art = await fetchArt(state.artwork);
    if (lastArtUrl === state.artwork) sendOverlay('overlay:art', art);
  }
}

function pushSettingsInfo() {
  if (settingsWin && !settingsWin.isDestroyed()) settingsWin.webContents.send('settings:info', settingsInfo());
}

// Switches between the browser extension and the built-in window at runtime.
function applySource() {
  handleState({ hasTrack: false, playing: false });
  if (isBrowserMode()) {
    if (mainWin) mainWin.destroy();
  } else if (!mainWin) {
    createMainWindow(true);
  }
  rebuildTrayMenu();
  pushSettingsInfo();
}

// ---------------------------------------------------------------------------
// Misc

function applyAutostart() {
  if (process.platform === 'linux') return;
  app.setLoginItemSettings({ openAtLogin: Boolean(config.get().general.autostart), args: ['--hidden'] });
}

function currentLang() {
  return i18n.resolveLang(config.get().general.language, app.getLocale());
}

function safeHost(url) {
  try {
    return new URL(url).hostname;
  } catch {
    return '';
  }
}

function clamp(v, min, max) {
  return Math.min(max, Math.max(min, v));
}

function quit() {
  quitting = true;
  app.quit();
}

app.on('second-instance', showMainWindow);

app.on('before-quit', () => {
  quitting = true;
  if (config) config.flush();
});

app.on('will-quit', () => {
  globalShortcut.unregisterAll();
  connector.stop();
});

app.on('window-all-closed', () => {
  // Keep running in the tray; quitting is explicit.
});

app.whenReady().then(() => {
  config = new Config();
  t = i18n.make(currentLang());

  createAppMenu();
  setupIpc();
  connector.start();
  if (!isBrowserMode()) createMainWindow();
  createOverlayWindow();
  createTray();
  registerHotkeys();
  applyAutostart();

  if (!config.get().general.onboarded) {
    config.patch({ general: { onboarded: true } });
    createSettingsWindow('connect');
  }
});
