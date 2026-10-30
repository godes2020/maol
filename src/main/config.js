const fs = require('fs');
const path = require('path');
const { app } = require('electron');

// Order here is the order hotkeys are listed in the settings window.
const DEFAULT_HOTKEYS = {
  playPause: 'Alt+Shift+K',
  next: 'Alt+Shift+Right',
  previous: 'Alt+Shift+Left',
  seekForward: 'Alt+Shift+L',
  seekBackward: 'Alt+Shift+J',
  volumeUp: 'Alt+Shift+Up',
  volumeDown: 'Alt+Shift+Down',
  toggleMute: 'Alt+Shift+0',
  cycleRepeat: 'Alt+Shift+R',
  toggleOverlay: 'Alt+Shift+O',
  editOverlay: 'Alt+Shift+M',
  toggleLock: 'Alt+Shift+U',
  showApp: 'Alt+Shift+Y',
  openPlayer: ''
};

const DEFAULTS = {
  hotkeys: DEFAULT_HOTKEYS,
  overlay: {
    visible: true,
    layout: 'card',
    // Which side the cover sits on in the card: 'auto' follows the screen
    // half the overlay is on, so the cover is always at the screen edge.
    side: 'auto',
    // 'locked': fully click-through, controls only in edit mode (for games).
    // 'hover': hovering unfolds the player and its buttons are clickable.
    interaction: 'locked',
    alwaysExpanded: false,
    hideWhenIdle: false,
    opacity: 1,
    scale: 1,
    reactive: true,
    intensity: 1,
    x: null,
    y: null
  },
  playback: {
    seekStep: 5,
    volumeStep: 5
  },
  general: {
    // 'system': Windows media controls, works with any player (default).
    // 'browser': the Moa Connector extension. 'app': the built-in window.
    source: 'system',
    migrated: 0,
    onboarded: false,
    language: 'auto',
    autostart: false,
    closeToTray: true,
    showWindow: false,
    gpu: true
  },
  window: {
    width: 480,
    height: 760,
    x: null,
    y: null
  }
};

function isPlainObject(v) {
  return v && typeof v === 'object' && !Array.isArray(v);
}

// Deep-merges `src` into a copy of `base`, keeping only keys known in `base`.
function merge(base, src) {
  const out = {};
  for (const key of Object.keys(base)) {
    const b = base[key];
    const s = src ? src[key] : undefined;
    if (isPlainObject(b)) out[key] = merge(b, isPlainObject(s) ? s : {});
    else out[key] = s === undefined ? b : s;
  }
  return out;
}

class Config {
  constructor() {
    this.file = path.join(app.getPath('userData'), 'config.json');
    this.data = merge(DEFAULTS, this.read());
    this.migrate();
  }

  migrate() {
    const g = this.data.general;
    if (g.migrated < 3) {
      // 0.3 reads any player through Windows; the extension became optional.
      if (g.source === 'browser') g.source = 'system';
      g.migrated = 3;
      this.save();
    }
  }

  read() {
    try {
      return JSON.parse(fs.readFileSync(this.file, 'utf8'));
    } catch {
      return {};
    }
  }

  save() {
    clearTimeout(this.timer);
    this.timer = setTimeout(() => this.flush(), 300);
  }

  flush() {
    clearTimeout(this.timer);
    try {
      fs.mkdirSync(path.dirname(this.file), { recursive: true });
      fs.writeFileSync(this.file, JSON.stringify(this.data, null, 2));
    } catch (err) {
      console.error('[moa] failed to save config', err);
    }
  }

  get() {
    return this.data;
  }

  // Applies a partial update such as { overlay: { opacity: 0.8 } }.
  patch(partial) {
    this.data = merge(DEFAULTS, merge(this.data, partial));
    this.save();
    return this.data;
  }
}

module.exports = { Config, DEFAULTS, DEFAULT_HOTKEYS };
