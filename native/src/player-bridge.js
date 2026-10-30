// Runs in every page of the built-in player (Yandex Music, SoundCloud, VK,
// YouTube Music). Reads the track from the standard Media Session API and the
// page's own <audio>/<video>, and runs Moa's playback commands through the
// handlers the site itself registered for the media keys.
(() => {
  if (window.__moa || window.top !== window) return;

  const handlers = {};
  const ms = navigator.mediaSession;
  if (ms && ms.setActionHandler) {
    const orig = ms.setActionHandler.bind(ms);
    ms.setActionHandler = (action, handler) => {
      handlers[action] = handler;
      return orig(action, handler);
    };
  }

  // Sites often play through `new Audio()` that never joins the DOM.
  let last = null;
  const play = HTMLMediaElement.prototype.play;
  HTMLMediaElement.prototype.play = function (...args) {
    last = this;
    return play.apply(this, args);
  };
  const media = () => {
    const all = [...document.querySelectorAll('audio, video')];
    if (last) all.unshift(last);
    return all.find((m) => !m.paused) || all.find((m) => m.currentSrc) || null;
  };

  function read() {
    const m = media();
    const md = ms && ms.metadata;
    let artwork = null;
    if (md && md.artwork && md.artwork.length) {
      const best = [...md.artwork].sort((a, b) => parseInt(b.sizes || 0, 10) - parseInt(a.sizes || 0, 10))[0];
      artwork = best.src || null;
    }
    const duration = m && isFinite(m.duration) ? m.duration : 0;
    return {
      hasTrack: !!((md && md.title) || (m && m.currentSrc)),
      playing: m ? !m.paused : !!ms && ms.playbackState === 'playing',
      title: (md && md.title) || '',
      artist: (md && md.artist) || '',
      album: (md && md.album) || '',
      artwork,
      duration,
      currentTime: m ? m.currentTime || 0 : 0,
      canSeek: duration > 0,
      repeat: null,
      volume: m ? Math.round(m.volume * 100) : null,
      muted: m ? m.muted : false
    };
  }

  let sent = '';
  let sentAt = 0;
  function tick() {
    const s = read();
    const key = JSON.stringify({ ...s, currentTime: 0 });
    // Position only matters every second or so; the overlay interpolates.
    if (key === sent && Date.now() - sentAt < 1000) return;
    sent = key;
    sentAt = Date.now();
    window.ipc.postMessage(JSON.stringify({ type: 'state', payload: s }));
  }
  setInterval(tick, 250);

  const call = (action, details) => {
    const h = handlers[action];
    if (!h) return false;
    h({ action, ...details });
    return true;
  };

  window.__moa = {
    cmd(name, arg) {
      const m = media();
      switch (name) {
        case 'playPause': {
          const playing = m ? !m.paused : ms && ms.playbackState === 'playing';
          if (!call(playing ? 'pause' : 'play') && m) playing ? m.pause() : m.play();
          return null;
        }
        case 'next':
          call('nexttrack');
          return null;
        case 'previous':
          call('previoustrack');
          return null;
        case 'seekTo':
        case 'seekBy': {
          if (!m) return null;
          let to = name === 'seekTo' ? arg : m.currentTime + arg;
          to = Math.max(0, isFinite(m.duration) ? Math.min(to, m.duration) : to);
          if (!call('seekto', { seekTime: to })) m.currentTime = to;
          return to;
        }
        case 'setVolume':
        case 'volumeBy': {
          if (!m) return null;
          const v = name === 'setVolume' ? arg : m.volume * 100 + arg;
          m.volume = Math.max(0, Math.min(100, v)) / 100;
          if (m.muted && v > 0) m.muted = false;
          return Math.round(m.volume * 100);
        }
        case 'toggleMute':
          if (!m) return null;
          m.muted = !m.muted;
          return m.muted;
        default:
          return null;
      }
    }
  };
})();
