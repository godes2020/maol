(() => {
  const params = new URLSearchParams(location.search);
  const t = MoaI18n.make(params.get('lang') || 'en');
  const $ = (id) => document.getElementById(id);
  const root = document.documentElement.style;

  const card = $('card');
  const els = {
    art: $('art'), bgArt: $('bgArt'), flash: $('flash'),
    title: $('title'), artist: $('artist'), toast: $('toast'),
    cur: $('cur'), rem: $('rem'), bar: $('bar'), repeatBtn: $('repeatBtn'),
    eqs: [$('eqCompact'), $('eqFull')]
  };
  const bars = [...document.querySelectorAll('.bars')].map((b) => [...b.children]);

  let state = null;
  let cfg = { alwaysExpanded: false, reactive: true, intensity: 1, opacity: 1 };
  let hovering = false;
  let editing = false;
  let stateAt = 0;

  // ---------------- i18n ----------------
  document.querySelectorAll('[data-tip]').forEach((el) => { el.title = t(el.dataset.tip); });
  $('editHint').textContent = t('overlay.editHint');
  $('editDone').textContent = t('overlay.editDone');

  // ---------------- Helpers ----------------
  const fmt = (s) => {
    s = Math.max(0, Math.floor(s || 0));
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const sec = String(s % 60).padStart(2, '0');
    return h ? `${h}:${String(m).padStart(2, '0')}:${sec}` : `${m}:${sec}`;
  };

  function setExpanded() {
    const on = editing || hovering || cfg.alwaysExpanded;
    card.classList.toggle('expanded', on);
    card.classList.toggle('compact', !on);
  }

  function setTitle(text) {
    if (els.title.textContent === text) return;
    els.title.textContent = text;
    const box = els.title.parentElement;
    box.classList.remove('marquee');
    requestAnimationFrame(() => {
      const overflow = els.title.scrollWidth - box.clientWidth;
      if (overflow > 4) {
        box.style.setProperty('--marquee-x', `-${overflow + 8}px`);
        box.style.setProperty('--marquee-s', `${Math.max(6, overflow / 18)}s`);
        box.classList.add('marquee');
      }
    });
  }

  // ---------------- Colours from the cover ----------------
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
        const sat = max ? (max - min) / max : 0;
        if (max < 40) continue;
        const key = `${r >> 5},${gr >> 5},${b >> 5}`;
        const e = buckets.get(key) || { r: 0, g: 0, b: 0, n: 0, w: 0 };
        e.r += r; e.g += gr; e.b += b; e.n++; e.w += (0.15 + sat * 2) * (max / 255);
        buckets.set(key, e);
      }
      const sorted = [...buckets.values()].sort((a, b) => b.w - a.w);
      if (!sorted.length) return null;
      const toRgb = (e) => {
        let r = e.r / e.n, gg = e.g / e.n, b = e.b / e.n;
        // Lift dark colours so they read on a dark card.
        const max = Math.max(r, gg, b);
        if (max < 215) { const k = 215 / max; r *= k; gg *= k; b *= k; }
        return [r, gg, b].map((v) => Math.round(Math.min(255, v))).join(', ');
      };
      const c1 = toRgb(sorted[0]);
      const c2 = toRgb(sorted.find((e, i) => i > 0 && Math.abs(e.r / e.n - sorted[0].r / sorted[0].n) + Math.abs(e.b / e.n - sorted[0].b / sorted[0].n) > 60) || sorted[1] || sorted[0]);
      return { c1, c2 };
    } catch {
      return null;
    }
  }

  function setArt(src) {
    if (!src) {
      els.art.classList.remove('loaded');
      els.art.removeAttribute('src');
      els.bgArt.style.backgroundImage = '';
      root.setProperty('--c1', '139, 92, 246');
      root.setProperty('--c2', '236, 72, 153');
      return;
    }
    const img = new Image();
    img.onload = () => {
      els.art.src = src;
      els.art.classList.add('loaded');
      els.bgArt.style.backgroundImage = `url("${src}")`;
      const colors = extractColors(img);
      if (colors) {
        root.setProperty('--c1', colors.c1);
        root.setProperty('--c2', colors.c2);
      }
    };
    img.src = src;
  }

  // ---------------- State ----------------
  function applyState(s) {
    state = s;
    stateAt = performance.now();
    const idle = !s || !s.hasTrack;
    card.classList.toggle('idle', idle);
    card.classList.toggle('paused', !idle && !s.playing);
    setTitle(idle ? t('overlay.idleTitle') : s.title);
    els.artist.textContent = idle ? t('overlay.idleArtist') : s.artist;
    els.eqs.forEach((b) => b.classList.toggle('muted', Boolean(s && (s.muted || s.volume === 0))));
    const r = ((s && s.repeat) || 'NONE').toLowerCase();
    els.repeatBtn.className = `btn repeat r-${r}`;
    renderProgress();
  }

  function currentTime() {
    if (!state) return 0;
    let cur = state.currentTime;
    if (state.playing) cur += (performance.now() - stateAt) / 1000;
    return Math.min(cur, state.duration || cur);
  }

  function renderProgress() {
    const dur = (state && state.duration) || 0;
    const cur = currentTime();
    const p = dur ? Math.min(1, cur / dur) : 0;
    root.setProperty('--progress', p.toFixed(4));
    els.cur.textContent = fmt(cur);
    els.rem.textContent = `-${fmt(dur - cur)}`;
  }

  // ---------------- Audio reactivity ----------------
  let energy = 0;
  let bass = 0;
  let flashTimer = 0;
  function applyAudio(a) {
    if (!cfg.reactive) {
      root.setProperty('--e', 0);
      root.setProperty('--b', 0);
      return;
    }
    // Fast attack, slower release looks more musical.
    energy = a.energy > energy ? a.energy : energy * 0.85 + a.energy * 0.15;
    bass = a.bass > bass ? a.bass : bass * 0.8 + a.bass * 0.2;
    root.setProperty('--e', Math.min(1, energy * 1.6).toFixed(3));
    root.setProperty('--b', Math.min(1, Math.max(0, (bass - 0.25) * 1.6)).toFixed(3));
    const bands = a.bands || [];
    for (const group of bars) {
      group.forEach((el, i) => el.style.setProperty('--h', Math.min(1, (bands[i] || 0) * 1.5).toFixed(3)));
    }
    if (a.beat) {
      els.flash.classList.add('on');
      clearTimeout(flashTimer);
      flashTimer = setTimeout(() => els.flash.classList.remove('on'), 70);
    }
    if (a.bpm >= 60 && a.bpm <= 200) root.setProperty('--beat-ms', `${Math.round(60000 / a.bpm)}ms`);
  }

  // ---------------- Config / edit / toast ----------------
  function applyConfig(c) {
    cfg = { ...cfg, ...c };
    root.setProperty('--opacity', cfg.opacity);
    root.setProperty('--intensity', cfg.intensity);
    card.classList.toggle('static', !cfg.reactive);
    setExpanded();
  }

  function setEdit(on) {
    editing = on;
    document.body.classList.toggle('edit', on);
    setExpanded();
  }

  let toastTimer = 0;
  function showToast(text) {
    els.toast.textContent = text;
    els.artist.parentElement.classList.add('toasting');
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => els.artist.parentElement.classList.remove('toasting'), 1400);
  }

  // ---------------- Input ----------------
  function command(name, arg) {
    if (window.moa) window.moa.command(name, arg);
  }

  document.querySelectorAll('[data-cmd]').forEach((el) => {
    el.addEventListener('click', (e) => {
      e.stopPropagation();
      command(el.dataset.cmd);
    });
  });

  els.bar.addEventListener('click', (e) => {
    if (!state || !state.duration) return;
    const r = els.bar.getBoundingClientRect();
    const ratio = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
    command('seekTo', ratio * state.duration);
    state.currentTime = ratio * state.duration;
    stateAt = performance.now();
    renderProgress();
  });

  card.addEventListener('mouseenter', () => {
    hovering = true;
    setExpanded();
    if (window.moa) window.moa.setInteractive(true);
  });
  card.addEventListener('mouseleave', () => {
    hovering = false;
    setExpanded();
    if (window.moa) window.moa.setInteractive(false);
  });

  setInterval(renderProgress, 250);

  // ---------------- Wiring ----------------
  if (window.moa) {
    window.moa.onState(applyState);
    window.moa.onArt(setArt);
    window.moa.onAudio(applyAudio);
    window.moa.onConfig(applyConfig);
    window.moa.onEditMode(setEdit);
    window.moa.onToast(showToast);
    applyState(null);
    window.moa.ready();
  } else {
    runDemo();
  }

  // Demo mode: used for screenshots in the README (open overlay.html in a browser).
  function runDemo() {
    const demo = {
      title: params.get('title') || 'Midnight Arcade',
      artist: params.get('artist') || 'Neon Pixels',
      hasTrack: !params.has('idle'),
      playing: !params.has('paused'),
      currentTime: Number(params.get('time') || 130),
      duration: Number(params.get('duration') || 170),
      volume: 70,
      muted: params.has('muted'),
      repeat: params.get('repeat') || 'NONE'
    };
    applyConfig({ alwaysExpanded: params.has('expanded'), reactive: true, intensity: 1, opacity: 1 });
    if (params.has('edit')) setEdit(true);
    applyState(demo);
    if (!params.has('idle')) setArt(params.get('art') || '../../docs/media/demo-cover.png');
    if (params.has('toast')) showToast(params.get('toast'));
    window.demo = { showToast, setEdit, state: demo, applyState, setHover(on) { hovering = on; setExpanded(); } };
    const bpm = 124;
    const beatMs = 60000 / bpm;
    const start = performance.now();
    setInterval(() => {
      if (!demo.playing || params.has('idle')) return applyAudio({ energy: 0, bass: 0, bands: [0, 0, 0, 0, 0] });
      const tms = performance.now() - start;
      const phase = (tms % beatMs) / beatMs;
      const kick = Math.exp(-phase * 6);
      const wob = (k) => 0.5 + 0.5 * Math.sin(tms / (180 + k * 70) + k);
      applyAudio({
        energy: 0.35 + 0.35 * kick,
        bass: 0.3 + 0.65 * kick,
        beat: phase < 0.05,
        bpm,
        bands: [0.25 + 0.6 * kick, 0.3 + 0.4 * wob(1), 0.25 + 0.4 * wob(2), 0.2 + 0.35 * wob(3), 0.15 + 0.3 * wob(4)]
      });
    }, 33);
  }
})();
