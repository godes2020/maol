// Runs in the main world of music.youtube.com — injected by the Moa window,
// or loaded by the Moa Connector browser extension. Reads player state,
// analyses audio for the overlay visuals and executes playback commands.
// Talks to the isolated preload / content script through window.postMessage.
(() => {
  if (window.__moa) return;

  const $ = (sel, root = document) => root.querySelector(sel);
  const post = (type, payload) => window.postMessage({ __moa: 1, type, payload }, '*');

  const playerBar = () => $('ytmusic-player-bar');
  const moviePlayer = () => $('#movie_player');
  const video = () => $('video');

  // Player buttons live in the bottom bar or, in newer layouts, in
  // <ytmusic-player-controls>; prefer the bar, then search the whole page.
  function clickIn(bar, selectors) {
    for (const root of [bar, document]) {
      if (!root) continue;
      for (const sel of selectors) {
        const el = $(sel, root);
        if (el) {
          el.click();
          return true;
        }
      }
    }
    return false;
  }

  // ---- State -------------------------------------------------------------

  function readRepeat() {
    try {
      const app = $('ytmusic-app');
      const mode = app && app.queue && app.queue.store.getState().queue.repeatMode;
      if (mode) return String(mode).toUpperCase();
    } catch {}
    const bar = playerBar();
    const fromAttr = bar && (bar.getAttribute('repeat-mode') || bar.repeatMode_);
    if (fromAttr) return String(fromAttr).toUpperCase();
    const btn = $('.repeat');
    const label = ((btn && (btn.getAttribute('title') || btn.getAttribute('aria-label'))) || '').toLowerCase();
    if (/one|одн|трек/.test(label)) return 'ONE';
    if (/all|все|плейлист/.test(label)) return 'ALL';
    return 'NONE';
  }

  function readMeta() {
    const md = navigator.mediaSession && navigator.mediaSession.metadata;
    const bar = playerBar();
    let title = (md && md.title) || '';
    let artist = (md && md.artist) || '';
    let album = (md && md.album) || '';
    let artwork = '';
    if (md && md.artwork && md.artwork.length) {
      const best = [...md.artwork].sort((a, b) => parseInt(b.sizes || 0, 10) - parseInt(a.sizes || 0, 10))[0];
      artwork = best.src;
    }
    if (!title && bar) {
      const t = $('.title', bar);
      title = t ? t.textContent.trim() : '';
      const by = $('.byline', bar);
      artist = by ? by.textContent.split('•')[0].trim() : '';
      const img = $('.image', bar);
      artwork = img ? img.src : '';
    }
    // Ask for a bigger thumbnail than the one in the player bar.
    artwork = artwork.replace(/=w\d+-h\d+/, '=w544-h544');
    return { title, artist, album, artwork };
  }

  function readState() {
    const v = video();
    const p = moviePlayer();
    const meta = readMeta();
    let volume = v ? Math.round(v.volume * 100) : 100;
    let muted = v ? v.muted : false;
    try {
      if (p && p.getVolume) volume = p.getVolume();
      if (p && p.isMuted) muted = p.isMuted();
    } catch {}
    return {
      ...meta,
      hasTrack: Boolean(meta.title) && Boolean(v && v.src),
      playing: Boolean(v && !v.paused && !v.ended),
      currentTime: v ? v.currentTime || 0 : 0,
      duration: v && isFinite(v.duration) ? v.duration : 0,
      volume,
      muted,
      repeat: readRepeat()
    };
  }

  setInterval(() => post('state', readState()), 250);

  // ---- Audio analysis ----------------------------------------------------
  // The <video> element plays MSE blob: URLs, so it is same-origin and can be
  // routed through an AnalyserNode without silencing it.

  let ctx = null;
  let analyser = null;
  let freq = null;
  const connected = new WeakSet();
  let bassAvg = 0;
  let lastBeat = 0;
  const beatGaps = [];

  function ensureAudio() {
    const v = video();
    if (!v) return false;
    if (!ctx) {
      ctx = new AudioContext();
      analyser = ctx.createAnalyser();
      analyser.fftSize = 1024;
      analyser.smoothingTimeConstant = 0.6;
      analyser.connect(ctx.destination);
      freq = new Uint8Array(analyser.frequencyBinCount);
    }
    if (ctx.state === 'suspended') ctx.resume().catch(() => {});
    // Routing the element through a context that the browser keeps suspended
    // (autoplay policy) would silence the music, so wait until it runs.
    if (ctx.state !== 'running') return false;
    if (!connected.has(v)) {
      try {
        ctx.createMediaElementSource(v).connect(analyser);
      } catch {
        // Already routed elsewhere; analysis just stays silent.
      }
      connected.add(v);
    }
    return true;
  }

  function avg(from, to) {
    let s = 0;
    for (let i = from; i < to; i++) s += freq[i];
    return s / ((to - from) * 255);
  }

  function median(arr) {
    const s = [...arr].sort((a, b) => a - b);
    return s[Math.floor(s.length / 2)];
  }

  setInterval(() => {
    const v = video();
    if (!v || v.paused) {
      post('audio', { energy: 0, bass: 0, beat: false, bands: [0, 0, 0, 0, 0], bpm: 0 });
      return;
    }
    if (!ensureAudio()) return;
    analyser.getByteFrequencyData(freq);
    const binHz = ctx.sampleRate / analyser.fftSize;
    const bin = (hz) => Math.max(1, Math.round(hz / binHz));
    const bass = avg(bin(40), bin(160));
    const energy = avg(bin(40), bin(8000));
    const edges = [40, 160, 500, 1500, 4000, 10000];
    const bands = [];
    for (let i = 0; i < edges.length - 1; i++) bands.push(avg(bin(edges[i]), bin(edges[i + 1])));

    const now = performance.now();
    let beat = false;
    if (bass > bassAvg * 1.25 && bass > 0.3 && now - lastBeat > 280) {
      beat = true;
      if (lastBeat && now - lastBeat < 1500) {
        beatGaps.push(now - lastBeat);
        if (beatGaps.length > 12) beatGaps.shift();
      }
      lastBeat = now;
    }
    bassAvg = bassAvg * 0.92 + bass * 0.08;
    const bpm = beatGaps.length >= 4 ? Math.round(60000 / median(beatGaps)) : 0;
    post('audio', { energy, bass, beat, bands, bpm });
  }, 33);

  // ---- Commands ----------------------------------------------------------

  function setVolume(value) {
    const vol = Math.max(0, Math.min(100, Math.round(value)));
    const p = moviePlayer();
    if (p && p.setVolume) {
      p.setVolume(vol);
      if (vol > 0 && p.isMuted && p.isMuted()) p.unMute();
    } else if (video()) {
      video().volume = vol / 100;
    }
    // Keep YouTube Music's own slider in sync so it doesn't snap back.
    const slider = $('ytmusic-player-bar #volume-slider');
    if (slider) {
      slider.value = vol;
      slider.dispatchEvent(new CustomEvent('immediate-value-change'));
      slider.dispatchEvent(new Event('change'));
    }
    return vol;
  }

  function seekTo(seconds) {
    const v = video();
    if (!v) return;
    const dur = isFinite(v.duration) ? v.duration : 0;
    const t = Math.max(0, dur ? Math.min(dur - 0.5, seconds) : seconds);
    const p = moviePlayer();
    if (p && p.seekTo) p.seekTo(t, true);
    else v.currentTime = t;
  }

  const commands = {
    playPause() {
      if (clickIn(playerBar(), ['#play-pause-button'])) return;
      const v = video();
      if (v) v.paused ? v.play() : v.pause();
    },
    next() {
      if (clickIn(playerBar(), ['.next-button'])) return;
      const p = moviePlayer();
      if (p && p.nextVideo) p.nextVideo();
    },
    previous() {
      if (clickIn(playerBar(), ['.previous-button'])) return;
      const p = moviePlayer();
      if (p && p.previousVideo) p.previousVideo();
    },
    seekBy(delta) {
      const v = video();
      if (v) seekTo(v.currentTime + Number(delta || 0));
    },
    seekTo(seconds) {
      seekTo(Number(seconds || 0));
    },
    volumeBy(delta) {
      return setVolume(readState().volume + Number(delta || 0));
    },
    setVolume(value) {
      return setVolume(Number(value));
    },
    toggleMute() {
      const p = moviePlayer();
      if (p && p.isMuted) {
        p.isMuted() ? p.unMute() : p.mute();
      } else if (video()) {
        video().muted = !video().muted;
      }
    },
    cycleRepeat() {
      clickIn(playerBar(), ['yt-icon-button.repeat', '.repeat', '[aria-label*="epeat"]']);
    }
  };

  function run(name, arg) {
    const fn = commands[name];
    if (!fn) return null;
    try {
      ensureAudio();
      const res = fn(arg);
      setTimeout(() => post('state', readState()), 60);
      return res === undefined ? null : res;
    } catch (err) {
      console.warn('[moa] command failed', name, err);
      return null;
    }
  }

  window.__moa = { cmd: run };

  // Commands relayed by the browser extension's content script.
  window.addEventListener('message', (event) => {
    const d = event.data;
    if (event.source !== window || !d || d.__moa !== 2 || d.type !== 'cmd') return;
    post('result', { id: d.id, value: run(d.name, d.arg) });
  });
})();
