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
const { SystemMedia } = require('./media');
const i18n = require('../shared/i18n');

const YTM_URL = 'https://music.youtube.com/';
const ROOT = path.join(__dirname, '..', '..');
const ICON = path.join(ROOT, 'assets', 'icon.png');
const TRAY_ICON = path.join(ROOT, 'assets', 'tray.png');
const GUIDE_URL = {
  en: 'https://github.com/godes2020/maol/blob/HEAD/docs/INSTALL.md#optional-browser-extension',
  ru: 'https://github.com/godes2020/maol/blob/HEAD/docs/INSTALL.ru.md#необязательно-расширение-для-браузера'
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

// Overlay content box in CSS pixels before scaling: the expanded card plus
// the edit toolbar. The window adds room for the glow on every side.
const OVERLAY_BOX = { card: { w: 300, h: 168 }, cover: { w: 96, h: 128 } };
const OVERLAY_PAD = 18;
const SCALE_MIN = 0.6;
const SCALE_MAX = 2.5;

if (!app.requestSingleInstanceLock()) {
  app.quit();
  process.exit(0);
}

app.setAppUserModelId('io.github.godes2020.moa');
// Moa never plays media itself in its own windows; keep Chromium from
// registering media keys or a media session of its own.
app.commandLine.appendSwitch('disable-features', 'HardwareMediaKeyHandling,MediaSessionService');

let config;
let t = (k) => k;
let mainWin = null;
let overlayWin = null;
let appWin = null;
let tray = null;
let quitting = false;
let editMode = false;
let hotkeysSuspended = false;
let failedHotkeys = [];
let lastState = null;
let lastArtUrl = null;
const artCache = new Map();
const connector = new Connector();
let media = null;
let resizing = null;
let anchors = { right: false, bottom: false };

// ---------------------------------------------------------------------------
// Windows

function source() {
  const s = config.get().general.source;
  return s === 'browser' || s === 'app' ? s : 'system';
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

  // The player keeps playing in the background; Moa quits from the tray.
  mainWin.on('close', (e) => {
    if (quitting) return;
    e.preventDefault();
    mainWin.hide();
  });
  mainWin.on('closed', () => {
    mainWin = null;
  });

  mainWin.loadURL(YTM_URL);
}

function overlayScale() {
  return clamp(Number(config.get().overlay.scale) || 1, SCALE_MIN, SCALE_MAX);
}

function overlaySize(scale = overlayScale()) {
  const box = OVERLAY_BOX[config.get().overlay.layout === 'cover' ? 'cover' : 'card'];
  return {
    width: Math.round((box.w + OVERLAY_PAD * 2) * scale),
    height: Math.round((box.h + OVERLAY_PAD * 2) * scale)
  };
}

// Keeps the visible box inside the work area of the display it is on; the
// transparent glow padding may hang over the edge.
function fitToScreen(b, scale) {
  const pad = Math.round(OVERLAY_PAD * scale);
  const area = screen.getDisplayMatching(b).workArea;
  return {
    ...b,
    x: clamp(b.x, area.x - pad, area.x + area.width - b.width + pad),
    y: clamp(b.y, area.y - pad, area.y + area.height - b.height + pad)
  };
}

function overlayBounds() {
  const o = config.get().overlay;
  const scale = overlayScale();
  const { width, height } = overlaySize(scale);
  const area = screen.getPrimaryDisplay().workArea;
  let x = o.x ?? area.x + 12;
  let y = o.y ?? area.y + 12;
  // Bring the overlay back if its saved position is off every display.
  const visible = screen.getAllDisplays().some((d) => {
    const r = d.bounds;
    return x + width > r.x + 20 && x < r.x + r.width - 20 && y + height > r.y + 20 && y < r.y + r.height - 20;
  });
  if (!visible) {
    x = area.x + 12;
    y = area.y + 12;
  }
  return { ...fitToScreen({ x, y, width, height }, scale), scale };
}

// Which screen half the overlay is in decides where it grows: on the right
// it grows to the left, at the bottom it unfolds upwards.
function computeAnchors(b) {
  const area = screen.getDisplayMatching(b).workArea;
  return {
    right: b.x + b.width / 2 > area.x + area.width / 2,
    bottom: b.y + b.height / 2 > area.y + area.height / 2
  };
}

// Resizes the overlay window while keeping the corner nearest to the screen
// edge in place.
function resizeOverlay(scale, from) {
  if (!overlayWin) return;
  const prev = from || overlayWin.getBounds();
  const { width, height } = overlaySize(scale);
  const a = computeAnchors(prev);
  const next = fitToScreen({
    x: a.right ? prev.x + prev.width - width : prev.x,
    y: a.bottom ? prev.y + prev.height - height : prev.y,
    width,
    height
  }, scale);
  overlayWin.setBounds(next);
  overlayWin.webContents.setZoomFactor(scale);
  return next;
}

function sendOverlayLayout() {
  if (!overlayWin) return;
  anchors = computeAnchors(overlayWin.getBounds());
  const side = config.get().overlay.side;
  sendOverlay('overlay:layout', {
    coverRight: side === 'auto' ? anchors.right : side === 'right',
    right: anchors.right,
    bottom: anchors.bottom
  });
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
    if (!overlayWin || resizing) return;
    const [x, y] = overlayWin.getPosition();
    config.patch({ overlay: { x, y } });
    sendOverlayLayout();
  });
  overlayWin.on('closed', () => {
    overlayWin = null;
  });

  overlayWin.loadFile(path.join(ROOT, 'src', 'overlay', 'overlay.html'), {
    query: { lang: currentLang() }
  });
}

function overlayConfig() {
  return config.get().overlay;
}

// With the system source the helper listens to the sound output for the
// beat glow, only while music plays and the glow is on.
function updateAudioCapture() {
  if (!media) return;
  const on = source() === 'system' && config.get().overlay.reactive && Boolean(lastState && lastState.playing);
  media.setAudio(on);
}

function applyOverlayConfig() {
  if (!overlayWin) return;
  const scale = overlayScale();
  const { width, height } = overlaySize(scale);
  const [w, h] = overlayWin.getSize();
  if (w !== width || h !== height) {
    const b = resizeOverlay(scale);
    config.patch({ overlay: { x: b.x, y: b.y } });
  }
  overlayWin.webContents.setZoomFactor(scale);
  sendOverlay('overlay:config', overlayConfig());
  sendOverlayLayout();
  updateOverlayVisibility();
}

// Dragging the resize grip in edit mode: follow the cursor from the main
// process, where screen coordinates do not change with the zoom factor.
function startResize() {
  if (!overlayWin || resizing) return;
  const b = overlayWin.getBounds();
  const a = computeAnchors(b);
  const box = OVERLAY_BOX[config.get().overlay.layout === 'cover' ? 'cover' : 'card'];
  const start = screen.getCursorScreenPoint();
  const scale0 = overlayScale();
  resizing = setInterval(() => {
    const p = screen.getCursorScreenPoint();
    const dx = (p.x - start.x) * (a.right ? -1 : 1);
    const dy = (p.y - start.y) * (a.bottom ? -1 : 1);
    const delta = box.w === box.h - 32 ? (dx + dy) / 2 : dx;
    const scale = Math.round(clamp(scale0 * (1 + delta / (box.w * scale0)), SCALE_MIN, SCALE_MAX) * 100) / 100;
    if (scale === overlayScale()) return;
    config.get().overlay.scale = scale;
    resizeOverlay(scale, b);
  }, 16);
}

function endResize() {
  if (!resizing) return;
  clearInterval(resizing);
  resizing = null;
  const [x, y] = overlayWin.getPosition();
  config.patch({ overlay: { x, y, scale: overlayScale() } });
  sendOverlayLayout();
  pushSettingsInfo();
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

// The Moa window: now playing, quick toggles and every setting. It is
// destroyed when closed so it costs no memory while Moa sits in the tray.
function createAppWindow(tab) {
  if (appWin) {
    if (appWin.isMinimized()) appWin.restore();
    appWin.show();
    appWin.focus();
    if (typeof tab === 'string' && tab) appWin.webContents.send('app:tab', tab);
    return;
  }
  appWin = new BrowserWindow({
    width: 440,
    height: 700,
    minWidth: 400,
    minHeight: 560,
    title: 'Moa',
    icon: ICON,
    frame: false,
    backgroundColor: '#0c0b10',
    show: false,
    webPreferences: {
      preload: path.join(ROOT, 'src', 'preload', 'settings.js'),
      contextIsolation: true,
      sandbox: true,
      spellcheck: false
    }
  });
  appWin.setMenu(null);
  appWin.once('ready-to-show', () => appWin && appWin.show());
  appWin.webContents.setWindowOpenHandler(({ url }) => {
    if (url.startsWith('https://')) shell.openExternal(url);
    return { action: 'deny' };
  });
  appWin.on('closed', () => {
    appWin = null;
    hotkeysSuspended = false;
    registerHotkeys();
    if (!config.get().general.closeToTray && !quitting) quit();
  });
  appWin.loadFile(path.join(ROOT, 'src', 'settings', 'settings.html'), {
    query: { lang: currentLang(), tab: typeof tab === 'string' ? tab : '' }
  });
}

function toggleAppWindow() {
  if (appWin && appWin.isVisible() && appWin.isFocused()) appWin.close();
  else createAppWindow();
}

// "The player": the built-in YouTube Music window, or the browser tab.
function showPlayer() {
  const src = source();
  if (src === 'browser' && connector.connected) return connector.cmd('focusPlayer');
  if (src !== 'app') return shell.openExternal(YTM_URL);
  if (!mainWin) return createMainWindow(true);
  if (mainWin.isVisible() && !mainWin.isMinimized() && mainWin.isFocused()) return mainWin.hide();
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
      { label: t('tray.open'), click: () => createAppWindow() },
      ...(source() === 'app' ? [{ label: t('tray.show'), click: showPlayer }] : []),
      { type: 'separator' },
      {
        label: t('tray.overlay'),
        type: 'checkbox',
        checked: o.visible,
        click: () => runAction('toggleOverlay')
      },
      { label: t('tray.editOverlay'), type: 'checkbox', checked: editMode, click: () => runAction('editOverlay') },
      { type: 'separator' },
      { label: t('tray.quit'), click: quit }
    ])
  );
}

function createTray() {
  const img = nativeImage.createFromPath(TRAY_ICON);
  tray = new Tray(img.isEmpty() ? nativeImage.createFromPath(ICON).resize({ width: 16, height: 16 }) : img);
  tray.setToolTip('Moa — Music Overlay App');
  tray.on('click', () => createAppWindow());
  rebuildTrayMenu();
}

function createAppMenu() {
  const menu = Menu.buildFromTemplate([
    {
      label: t('menu.app'),
      submenu: [
        { label: t('tray.open'), accelerator: 'CmdOrCtrl+,', click: () => createAppWindow() },
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
  if (source() === 'system') return media ? media.cmd(name, arg) : Promise.resolve(null);
  if (source() === 'browser') return connector.cmd(name, arg);
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
    const res = await ytm('toggleMute');
    const muted = typeof res === 'boolean' ? res : lastState ? !lastState.muted : true;
    toast(muted ? t('overlay.muted') : `${t('overlay.volume')} ${lastState && lastState.volume != null ? lastState.volume : ''}%`);
  },
  cycleRepeat: async () => {
    const res = await ytm('cycleRepeat');
    if (res === 'unsupported') return toast(t('overlay.repeatUnsupported'));
    const next = typeof res === 'string' ? res : REPEAT_NEXT[(lastState && lastState.repeat) || 'NONE'] || 'NONE';
    toast(t(REPEAT_LABEL[next] || 'overlay.repeatOff'));
  },
  toggleOverlay: () => {
    const visible = !config.get().overlay.visible;
    config.patch({ overlay: { visible } });
    if (!visible && editMode) setEditMode(false);
    applyOverlayConfig();
    rebuildTrayMenu();
    pushSettingsInfo();
  },
  editOverlay: () => setEditMode(!editMode),
  toggleLock: () => {
    const interaction = config.get().overlay.interaction === 'locked' ? 'hover' : 'locked';
    config.patch({ overlay: { interaction } });
    applyOverlayConfig();
    pushSettingsInfo();
    toast(t(interaction === 'locked' ? 'overlay.locked' : 'overlay.unlocked'));
  },
  showApp: toggleAppWindow,
  openPlayer: showPlayer,
  flipSide: () => {
    const side = (config.get().overlay.side === 'auto' ? anchors.right : config.get().overlay.side === 'right') ? 'left' : 'right';
    config.patch({ overlay: { side } });
    applyOverlayConfig();
    pushSettingsInfo();
  },
  toggleLayout: () => {
    config.patch({ overlay: { layout: config.get().overlay.layout === 'cover' ? 'card' : 'cover' } });
    applyOverlayConfig();
    pushSettingsInfo();
  }
};

function runAction(name, arg) {
  const fn = actions[name];
  if (fn) return fn(arg);
  // Direct player commands from the overlay and the Moa window.
  if (['seekTo', 'setVolume'].includes(name)) return ytm(name, arg);
  if (name === 'volumeTo') return ytm('setVolume', arg);
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
  if (url.startsWith('art') && media) return media.getArt(url);
  if (url.startsWith('data:')) return url;
  if (artCache.has(url)) return artCache.get(url);
  try {
    const res = await net.fetch(url);
    if (!res.ok) return null;
    const buf = Buffer.from(await res.arrayBuffer());
    const type = res.headers.get('content-type') || 'image/jpeg';
    const dataUrl = `data:${type};base64,${buf.toString('base64')}`;
    artCache.set(url, dataUrl);
    if (artCache.size > 8) artCache.delete(artCache.keys().next().value);
    return dataUrl;
  } catch {
    return null;
  }
}

function extensionDir() {
  return app.isPackaged ? path.join(process.resourcesPath, 'extension') : path.join(ROOT, 'extension');
}

function helperExe() {
  if (process.env.MOA_MEDIA_HELPER) return path.resolve(process.env.MOA_MEDIA_HELPER);
  return app.isPackaged
    ? path.join(process.resourcesPath, 'helper', 'MoaMedia.exe')
    : path.join(ROOT, 'build', 'helper', 'MoaMedia.exe');
}

function settingsInfo() {
  return {
    source: source(),
    media: media ? { ready: media.ready, error: media.error, available: media.available } : null,
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
    if (source() === 'app' && mainWin && e.sender === mainWin.webContents) handleState(state);
  });
  ipcMain.on('moa:audio', (e, audio) => {
    if (source() === 'app' && mainWin && e.sender === mainWin.webContents) sendOverlay('overlay:audio', audio);
  });

  connector.on('state', (state) => {
    if (source() === 'browser') handleState(state);
  });
  connector.on('audio', (audio) => {
    if (source() === 'browser') sendOverlay('overlay:audio', audio);
  });
  connector.on('status', pushSettingsInfo);
  media.on('state', (state) => {
    if (source() === 'system') handleState(state);
  });
  media.on('status', () => {
    updateAudioCapture();
    pushSettingsInfo();
  });
  media.on('audio', (audio) => {
    if (source() === 'system') sendOverlay('overlay:audio', audio);
  });

  ipcMain.on('settings:open-ytm', () => shell.openExternal(YTM_URL));
  ipcMain.on('settings:open-extension-folder', () => shell.openPath(extensionDir()));
  ipcMain.on('settings:open-guide', () => shell.openExternal(GUIDE_URL[currentLang()] || GUIDE_URL.en));

  ipcMain.on('moa:command', (_e, name, arg) => runAction(name, arg));
  ipcMain.on('moa:action', (_e, name) => runAction(name));

  ipcMain.on('overlay:ready', () => {
    sendOverlay('overlay:config', overlayConfig());
    sendOverlayLayout();
    sendOverlay('overlay:edit-mode', editMode);
    if (lastState) sendOverlay('overlay:state', lastState);
    if (lastArtUrl) fetchArt(lastArtUrl).then((art) => sendOverlay('overlay:art', art));
    updateOverlayVisibility();
  });

  ipcMain.on('overlay:resize', (_e, on) => (on ? startResize() : endResize()));

  ipcMain.on('app:window', (_e, what) => {
    if (!appWin) return;
    if (what === 'minimize') appWin.minimize();
    else if (what === 'close') appWin.close();
  });
  ipcMain.handle('app:state', async () => ({
    state: lastState,
    art: lastArtUrl ? await fetchArt(lastArtUrl) : null
  }));
  ipcMain.on('settings:open-url', (_e, url) => {
    if (typeof url === 'string' && url.startsWith('https://')) shell.openExternal(url);
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
      updateAudioCapture();
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
    config.patch({ overlay: { x: null, y: null, scale: 1, side: 'auto' } });
    if (overlayWin) {
      const b = overlayBounds();
      overlayWin.setBounds({ x: b.x, y: b.y, width: b.width, height: b.height });
      applyOverlayConfig();
    }
    pushSettingsInfo();
  });
}

async function handleState(state) {
  const hadTrack = lastState && lastState.hasTrack;
  lastState = state;
  sendOverlay('overlay:state', state);
  sendApp('app:state', state);
  updateAudioCapture();
  if (Boolean(hadTrack) !== Boolean(state.hasTrack)) updateOverlayVisibility();
  if (state.artwork !== lastArtUrl) {
    lastArtUrl = state.artwork;
    const art = await fetchArt(state.artwork);
    if (lastArtUrl === state.artwork) {
      sendOverlay('overlay:art', art);
      sendApp('app:art', art);
    }
  }
}

function sendApp(channel, data) {
  if (appWin && !appWin.isDestroyed()) appWin.webContents.send(channel, data);
}

function pushSettingsInfo() {
  sendApp('settings:info', settingsInfo());
}

// Switches the music source at runtime. Only the active one runs: the
// built-in window alone takes hundreds of megabytes.
function applySource() {
  handleState({ hasTrack: false, playing: false });
  const src = source();
  if (src === 'system') media.start();
  else media.stop();
  if (src === 'browser') connector.start();
  else connector.stop();
  if (src !== 'app' && mainWin) mainWin.destroy();
  if (src === 'app' && !mainWin) createMainWindow(true);
  sendOverlay('overlay:config', overlayConfig());
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

app.on('second-instance', () => createAppWindow());

app.on('before-quit', () => {
  quitting = true;
  if (config) config.flush();
});

app.on('will-quit', () => {
  globalShortcut.unregisterAll();
  connector.stop();
  if (media) media.stop();
});

app.on('window-all-closed', () => {
  // Keep running in the tray; quitting is explicit.
});

// The config is read before the app is ready because turning off hardware
// acceleration (saves the GPU process, ~60-100 MB) must happen first.
config = new Config();
if (!config.get().general.gpu) app.disableHardwareAcceleration();

app.whenReady().then(() => {
  t = i18n.make(currentLang());
  media = new SystemMedia(helperExe());

  createAppMenu();
  setupIpc();
  if (source() === 'system') media.start();
  if (source() === 'browser') connector.start();
  if (source() === 'app') createMainWindow();
  createOverlayWindow();
  createTray();
  registerHotkeys();
  applyAutostart();

  if (!config.get().general.onboarded) {
    config.patch({ general: { onboarded: true } });
    createAppWindow('home');
  }
});
