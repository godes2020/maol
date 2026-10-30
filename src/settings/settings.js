(async () => {
  const api = window.moa;
  let info = await api.getInfo();
  const t = MoaI18n.make(info.lang);
  const $ = (id) => document.getElementById(id);
  const body = document.body;

  // ---------------- Static text & icons ----------------
  document.querySelectorAll('[data-i18n]').forEach((el) => { el.textContent = t(el.dataset.i18n); });
  document.querySelectorAll('[data-tip]').forEach((el) => { el.title = t(el.dataset.tip); });
  document.querySelectorAll('[data-icon-left]').forEach((el) => el.insertAdjacentHTML('afterbegin', MoaIcons.icon(el.dataset.iconLeft)));
  MoaIcons.hydrate();
  $('version').textContent = `v${info.version}`;

  $('winMin').addEventListener('click', () => api.window('minimize'));
  $('winClose').addEventListener('click', () => api.window('close'));
  document.querySelectorAll('[data-url]').forEach((el) => el.addEventListener('click', () => api.openUrl(el.dataset.url)));

  // ---------------- Tabs ----------------
  function openTab(name) {
    const btn = document.querySelector(`#tabs button[data-tab="${name}"]`);
    if (!btn) return;
    document.querySelectorAll('#tabs button').forEach((b) => b.classList.toggle('active', b === btn));
    document.querySelectorAll('.tab').forEach((s) => s.classList.toggle('active', s.id === `tab-${name}`));
    $('scroller').scrollTop = 0;
  }
  document.querySelectorAll('#tabs button').forEach((btn) => btn.addEventListener('click', () => openTab(btn.dataset.tab)));
  openTab(new URLSearchParams(location.search).get('tab') || 'home');
  api.onTab(openTab);

  // ---------------- Platforms (home, nothing playing) ----------------
  const PLATFORMS = [
    { name: 'YouTube Music', icon: 'youtubemusic', color: '#FF0000', url: 'https://music.youtube.com/' },
    { name: 'Spotify', icon: 'spotify', color: '#1DB954', url: 'https://open.spotify.com/' },
    { name: t('home.yandex'), icon: 'music', color: '#FFCC00', fg: '#111', url: 'https://music.yandex.ru/' },
    { name: 'SoundCloud', icon: 'soundcloud', color: '#FF5500', url: 'https://soundcloud.com/' },
    { name: 'Apple Music', icon: 'applemusic', color: '#FA243C', url: 'https://music.apple.com/' },
    { name: 'Deezer', icon: 'deezer', color: '#A238FF', url: 'https://www.deezer.com/' },
    { name: t('home.vk'), icon: 'vk', color: '#0077FF', url: 'https://vk.com/music' },
    { name: 'TIDAL', icon: 'tidal', color: '#000000', url: 'https://listen.tidal.com/' }
  ];
  const platforms = $('platforms');
  for (const p of PLATFORMS) {
    const b = document.createElement('button');
    b.className = 'platform';
    b.title = p.url.replace('https://', '');
    b.innerHTML = `<span class="logo-tile" style="background:${p.color};color:${p.fg || '#fff'};box-shadow:inset 0 0 0 1px rgba(255,255,255,.12)">${MoaIcons.icon(p.icon)}</span><small></small>`;
    b.querySelector('small').textContent = p.name;
    b.addEventListener('click', () => api.openUrl(p.url));
    platforms.append(b);
  }

  // Which app or browser is playing, from the Windows media session id.
  const APPS = [
    [/spotify/i, 'spotify', 'Spotify'],
    [/yandex.*music|music.*yandex/i, 'music', t('home.yandex')],
    [/msedge/i, 'globe', 'Microsoft Edge'],
    [/chrome/i, 'googlechrome', 'Chrome'],
    [/zen/i, 'zenbrowser', 'Zen'],
    [/firefox|308046B0AF4A39CB/i, 'firefoxbrowser', 'Firefox'],
    [/brave/i, 'brave', 'Brave'],
    [/opera/i, 'opera', 'Opera'],
    [/vivaldi/i, 'vivaldi', 'Vivaldi'],
    [/yandex|browser/i, 'globe', t('home.yandexBrowser')],
    [/soundcloud/i, 'soundcloud', 'SoundCloud'],
    [/deezer/i, 'deezer', 'Deezer'],
    [/tidal/i, 'tidal', 'TIDAL'],
    [/apple|itunes/i, 'applemusic', 'Apple Music']
  ];
  function sourceBadge(s) {
    if (info.source !== 'system' || !(s && s.app)) return { icon: 'youtubemusic', name: 'YouTube Music' };
    const id = (s && s.app) || '';
    const hit = APPS.find(([re]) => re.test(id));
    if (hit) return { icon: hit[1], name: hit[2] };
    const name = id.replace(/\.exe$/i, '').split(/[\\/!]/).pop();
    return name && name.length < 30 ? { icon: 'music', name } : null;
  }

  // ---------------- Now playing ----------------
  let state = null;
  let stateAt = 0;
  let seeking = false;
  const fmt = (sec) => {
    sec = Math.max(0, Math.floor(sec || 0));
    const h = Math.floor(sec / 3600);
    const m = Math.floor((sec % 3600) / 60);
    const ss = String(sec % 60).padStart(2, '0');
    return h ? `${h}:${String(m).padStart(2, '0')}:${ss}` : `${m}:${ss}`;
  };
  const setFill = (input) => {
    const min = Number(input.min) || 0;
    const max = Number(input.max) || 100;
    input.style.setProperty('--fill', `${((Number(input.value) - min) / (max - min)) * 100}%`);
  };

  function currentTime() {
    if (!state) return 0;
    let cur = state.currentTime || 0;
    if (state.playing) cur += (performance.now() - stateAt) / 1000;
    return Math.min(cur, state.duration || cur);
  }

  function renderProgress() {
    if (!state || seeking) return;
    const dur = state.duration || 0;
    const cur = currentTime();
    const seek = $('npSeek');
    seek.value = dur ? Math.round((cur / dur) * 1000) : 0;
    setFill(seek);
    $('npCur').textContent = fmt(cur);
    $('npDur').textContent = fmt(dur);
  }

  function applyState(s) {
    state = s;
    stateAt = performance.now();
    const idle = !s || !s.hasTrack;
    body.classList.toggle('idle', idle);
    body.classList.toggle('playing', Boolean(s && s.playing));
    body.classList.toggle('paused', Boolean(s && !s.playing));
    body.classList.toggle('muted', Boolean(s && s.muted));
    $('npTitle').textContent = idle ? t('home.nothing') : s.title;
    $('npArtist').textContent = idle ? '' : s.artist || '';
    const badge = idle ? null : sourceBadge(s);
    $('npSource').innerHTML = badge ? `${MoaIcons.icon(badge.icon)}<span></span>` : '';
    if (badge) $('npSource').querySelector('span').textContent = badge.name;

    const repeat = (s && s.repeat) || 'NONE';
    $('npRepeat').classList.toggle('on', repeat !== 'NONE');
    $('npRepeat').classList.toggle('one', repeat === 'ONE');
    $('npRepeat').classList.toggle('disabled', Boolean(s && s.repeat === null));
    $('npSeek').disabled = Boolean(s && s.canSeek === false);
    const hasVolume = Boolean(s && s.volume != null);
    $('npVolume').hidden = !hasVolume;
    if (hasVolume && document.activeElement !== $('npVol')) {
      $('npVol').value = s.volume;
      setFill($('npVol'));
    }
    renderProgress();
  }

  function extractColors(img) {
    try {
      const c = document.createElement('canvas');
      c.width = c.height = 24;
      const g = c.getContext('2d', { willReadFrequently: true });
      g.drawImage(img, 0, 0, 24, 24);
      const d = g.getImageData(0, 0, 24, 24).data;
      const buckets = new Map();
      for (let i = 0; i < d.length; i += 4) {
        const r = d[i], gr = d[i + 1], b = d[i + 2];
        const max = Math.max(r, gr, b), min = Math.min(r, gr, b);
        if (max < 40) continue;
        const sat = (max - min) / max;
        const key = `${r >> 5},${gr >> 5},${b >> 5}`;
        const e = buckets.get(key) || { r: 0, g: 0, b: 0, n: 0, w: 0 };
        e.r += r; e.g += gr; e.b += b; e.n++; e.w += (0.15 + sat * 2) * (max / 255);
        buckets.set(key, e);
      }
      const sorted = [...buckets.values()].sort((a, b) => b.w - a.w);
      if (!sorted.length) return null;
      const rgb = (e) => {
        let r = e.r / e.n, gg = e.g / e.n, b = e.b / e.n;
        const max = Math.max(r, gg, b);
        if (max < 215) { const k = 215 / max; r *= k; gg *= k; b *= k; }
        return [r, gg, b].map((v) => Math.round(Math.min(255, v))).join(', ');
      };
      const second = sorted.find((e, i) => i > 0 && Math.abs(e.r / e.n - sorted[0].r / sorted[0].n) + Math.abs(e.b / e.n - sorted[0].b / sorted[0].n) > 60) || sorted[1] || sorted[0];
      return [rgb(sorted[0]), rgb(second)];
    } catch {
      return null;
    }
  }

  function applyArt(src) {
    const img = $('npArt');
    const root = document.documentElement.style;
    if (!src) {
      img.classList.remove('loaded');
      img.removeAttribute('src');
      $('ambientArt').style.backgroundImage = '';
      root.removeProperty('--a1');
      root.removeProperty('--a2');
      return;
    }
    const probe = new Image();
    probe.onload = () => {
      img.src = src;
      img.classList.add('loaded');
      $('ambientArt').style.backgroundImage = `url("${src}")`;
      const colors = extractColors(probe);
      if (colors) {
        root.setProperty('--a1', colors[0]);
        root.setProperty('--a2', colors[1]);
      }
    };
    probe.src = src;
  }

  document.querySelectorAll('[data-cmd]').forEach((el) => el.addEventListener('click', () => api.command(el.dataset.cmd)));

  const seek = $('npSeek');
  seek.addEventListener('input', () => {
    seeking = true;
    setFill(seek);
    if (state && state.duration) $('npCur').textContent = fmt((seek.value / 1000) * state.duration);
  });
  seek.addEventListener('change', () => {
    seeking = false;
    if (!state || !state.duration) return;
    const to = (seek.value / 1000) * state.duration;
    api.command('seekTo', to);
    state.currentTime = to;
    stateAt = performance.now();
    renderProgress();
  });

  const vol = $('npVol');
  let volTimer = 0;
  vol.addEventListener('input', () => {
    setFill(vol);
    clearTimeout(volTimer);
    volTimer = setTimeout(() => api.command('volumeTo', Number(vol.value)), 60);
  });

  const first = await api.getState();
  applyState(first.state);
  applyArt(first.art);
  api.onState(applyState);
  api.onArt(applyArt);
  setInterval(renderProgress, 500);

  // ---------------- Quick toggles & status ----------------
  function renderQuick() {
    const o = info.config.overlay;
    $('qOverlay').classList.toggle('on', o.visible);
    $('qLock').classList.toggle('on', o.interaction === 'locked');
    $('qLock').querySelector('.ic').outerHTML = MoaIcons.icon(o.interaction === 'locked' ? 'lock' : 'lock-open');
    $('qLayout').querySelector('.ic').outerHTML = MoaIcons.icon(o.layout === 'cover' ? 'square' : 'rectangle-horizontal');
    $('qLayoutText').textContent = t(o.layout === 'cover' ? 'ov.cover' : 'ov.card');
  }
  $('qOverlay').addEventListener('click', () => api.action('toggleOverlay'));
  $('qMove').addEventListener('click', () => api.action('editOverlay'));
  $('qLock').addEventListener('click', () => api.action('toggleLock'));
  $('qLayout').addEventListener('click', () => api.action('toggleLayout'));

  function renderStatus() {
    const status = $('status');
    let ok = false;
    let text = '';
    if (info.source === 'system') {
      const m = info.media || {};
      ok = Boolean(m.ready);
      text = ok ? t('status.system') : m.error ? `${t('status.systemError')}: ${m.error}` : t('status.starting');
      if (info.connector && info.connector.connected) text += ` · ${t('status.extension')}`;
    } else if (info.source === 'browser') {
      const c = info.connector || {};
      ok = Boolean(c.connected);
      text = c.error ? t('status.connectorError', { error: c.error }) : ok ? t('status.extension') : t('status.noExtension');
    } else {
      ok = true;
      text = t('status.app');
    }
    status.classList.toggle('ok', ok);
    $('statusText').textContent = text;
    $('browserSteps').hidden = info.source !== 'browser' && !(info.connector && info.connector.connected);
    $('showWindowRow').hidden = info.source !== 'app';
    $('extDir').textContent = info.extensionDir || '';
  }

  $('openExtFolder').addEventListener('click', () => api.openExtensionFolder());
  $('openGuide').addEventListener('click', () => api.openGuide());

  // ---------------- Hotkeys ----------------
  const list = $('hotkeyList');
  const msg = $('hotkeyMsg');
  let capturing = null;
  const KEY_LABEL = { Up: '↑', Down: '↓', Left: '←', Right: '→', Super: 'Win', Space: '␣' };

  function showMsg(text) {
    msg.hidden = !text;
    msg.textContent = text || '';
  }

  function renderHotkeys() {
    list.textContent = '';
    for (const action of Object.keys(info.defaults)) {
      const accel = info.config.hotkeys[action] || '';
      const row = document.createElement('div');
      row.className = 'hk' + (accel && info.failed.includes(accel) ? ' failed' : '');
      const label = document.createElement('span');
      label.className = 'hk-label';
      label.textContent = t(`action.${action}`);
      const btn = document.createElement('button');
      btn.className = 'hk-keys' + (accel ? '' : ' unset');
      if (accel) {
        for (const k of accel.split('+')) {
          const kbd = document.createElement('kbd');
          kbd.textContent = KEY_LABEL[k] || k;
          btn.append(kbd);
        }
      } else {
        btn.textContent = t('hk.unset');
      }
      btn.addEventListener('click', () => startCapture(action, btn));
      row.append(label, btn);
      list.append(row);
    }
    if (info.failed.length) showMsg(t('hk.failed', { keys: info.failed.join(', ') }));
  }

  function startCapture(action, btn) {
    stopCapture();
    capturing = { action, btn };
    btn.className = 'hk-keys capturing';
    btn.textContent = t('hk.press');
    api.suspendHotkeys(true);
  }

  function stopCapture() {
    if (!capturing) return;
    capturing = null;
    api.suspendHotkeys(false);
    renderHotkeys();
  }

  const CODE_MAP = {
    ArrowUp: 'Up', ArrowDown: 'Down', ArrowLeft: 'Left', ArrowRight: 'Right',
    Space: 'Space', Enter: 'Enter', Tab: 'Tab', Home: 'Home', End: 'End',
    PageUp: 'PageUp', PageDown: 'PageDown', Insert: 'Insert', Delete: 'Delete',
    Minus: '-', Equal: '=', BracketLeft: '[', BracketRight: ']', Semicolon: ';',
    Quote: "'", Backquote: '`', Backslash: '\\', Comma: ',', Period: '.', Slash: '/',
    NumpadAdd: 'numadd', NumpadSubtract: 'numsub', NumpadMultiply: 'nummult',
    NumpadDivide: 'numdiv', NumpadDecimal: 'numdec'
  };

  function keyFromCode(code) {
    if (/^Key[A-Z]$/.test(code)) return code.slice(3);
    if (/^Digit\d$/.test(code)) return code.slice(5);
    if (/^Numpad\d$/.test(code)) return 'num' + code.slice(6);
    if (/^F\d{1,2}$/.test(code)) return code;
    return CODE_MAP[code] || null;
  }

  window.addEventListener('keydown', async (e) => {
    if (!capturing) return;
    e.preventDefault();
    e.stopPropagation();
    const mods = [];
    if (e.ctrlKey) mods.push('Ctrl');
    if (e.altKey) mods.push('Alt');
    if (e.shiftKey) mods.push('Shift');
    if (e.metaKey) mods.push('Super');

    if (!mods.length && e.code === 'Escape') return stopCapture();
    if (!mods.length && (e.code === 'Backspace' || e.code === 'Delete')) {
      const action = capturing.action;
      info = await api.patch({ hotkeys: { [action]: '' } });
      showMsg('');
      return stopCapture();
    }

    const key = keyFromCode(e.code);
    if (!key) return; // waiting for a non-modifier key
    if (!mods.length && !/^F\d+$/.test(key)) {
      showMsg(t('hk.needModifier'));
      return;
    }
    const accel = [...mods, key].join('+');
    const owner = Object.entries(info.config.hotkeys).find(([a, k]) => a !== capturing.action && k && k.toLowerCase() === accel.toLowerCase());
    if (owner) {
      showMsg(t('hk.duplicate', { action: t(`action.${owner[0]}`) }));
      return;
    }
    const action = capturing.action;
    capturing = null;
    api.suspendHotkeys(false);
    info = await api.patch({ hotkeys: { [action]: accel } });
    showMsg('');
    renderHotkeys();
  }, true);

  window.addEventListener('blur', stopCapture);

  $('resetHotkeys').addEventListener('click', async () => {
    info = await api.resetHotkeys();
    showMsg('');
    renderHotkeys();
  });

  // ---------------- Generic controls ----------------
  const getPath = (obj, p) => p.split('.').reduce((o, k) => (o ? o[k] : undefined), obj);
  const toPatch = (p, value) => {
    const [section, key] = p.split('.');
    return { [section]: { [key]: value } };
  };

  function formatOutput(input) {
    const out = input.parentElement.querySelector('output');
    if (!out) return;
    const v = Number(input.value);
    const f = input.dataset.format;
    out.textContent = f === 'percent' ? `${Math.round(v * 100)}%` : f === 'sec' ? `${v} ${t('hk.sec')}` : f === 'pct' ? `${v}%` : (Number.isInteger(v) ? v : v.toFixed(1));
  }

  async function save(pathName, value) {
    const prevLang = info.config.general.language;
    const prevGpu = info.config.general.gpu;
    info = await api.patch(toPatch(pathName, value));
    if (pathName === 'general.language' && value !== prevLang) $('restartNote').hidden = false;
    if (pathName === 'general.gpu' && value !== prevGpu) $('restartNote').hidden = false;
    renderAll();
  }

  const inputs = [...document.querySelectorAll('input[data-path]')];
  inputs.forEach((input) => {
    const evt = input.type === 'range' ? 'input' : 'change';
    input.addEventListener(evt, () => {
      let v = input.type === 'checkbox' ? input.checked : Number(input.value);
      if (input.type === 'range') {
        setFill(input);
        formatOutput(input);
      }
      save(input.dataset.path, v);
    });
  });

  const choices = [...document.querySelectorAll('.seg[data-path], .tiles[data-path]')];
  choices.forEach((group) => {
    group.querySelectorAll('button[data-value]').forEach((btn) => {
      btn.addEventListener('click', () => save(group.dataset.path, btn.dataset.value));
    });
  });

  function renderInputs() {
    for (const input of inputs) {
      if (document.activeElement === input && input.type === 'range') continue;
      const value = getPath(info.config, input.dataset.path);
      if (input.type === 'checkbox') input.checked = Boolean(value);
      else {
        input.value = value;
        setFill(input);
        formatOutput(input);
      }
    }
    for (const group of choices) {
      const value = getPath(info.config, group.dataset.path);
      group.querySelectorAll('button[data-value]').forEach((b) => b.classList.toggle('active', b.dataset.value === value));
    }
  }

  $('moveOverlay').addEventListener('click', () => api.action('editOverlay'));
  $('resetOverlay').addEventListener('click', () => api.resetOverlayPosition());

  $('unlockNow').addEventListener('click', () => save('overlay.interaction', 'hover'));

  function renderAll() {
    const o = info.config.overlay;
    $('lockedWarn').hidden = !(o.alwaysExpanded && o.interaction === 'locked');
    renderInputs();
    renderQuick();
    renderStatus();
  }

  api.onInfo((next) => {
    info = next;
    renderAll();
  });

  renderHotkeys();
  renderAll();
})();
