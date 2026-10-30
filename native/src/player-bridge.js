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
  // Sites also tend to reset the volume for every new track, so the volume
  // asked in Moa is applied again to whatever starts playing.
  let last = null;
  let want = null;
  let applying = false;
  const apply = (m) => {
    if (want === null || Math.abs(m.volume - want) < 0.005) return;
    applying = true;
    m.volume = want;
    applying = false;
  };
  const play = HTMLMediaElement.prototype.play;
  HTMLMediaElement.prototype.play = function (...args) {
    last = this;
    apply(this);
    return play.apply(this, args);
  };
  // The site's own volume control wins from then on.
  document.addEventListener('volumechange', (e) => {
    if (!applying && want !== null && Math.abs(e.target.volume - want) > 0.01 && e.isTrusted && document.hasFocus()) want = null;
  }, true);

  // YouTube Music keeps its own volume and loudness normalisation on top of
  // the <video> element and puts it back after a few seconds, so there the
  // volume goes through the player's API and the page's own slider.
  const yt = () => {
    const p = document.getElementById('movie_player');
    return p && typeof p.setVolume === 'function' && typeof p.getVolume === 'function' ? p : null;
  };
  const ytSet = (p, v) => {
    p.setVolume(v);
    if (v > 0 && p.isMuted && p.isMuted()) p.unMute();
    const slider = document.querySelector('ytmusic-player-bar #volume-slider');
    if (slider && 'value' in slider) {
      slider.value = v;
      slider.dispatchEvent(new CustomEvent('change', { bubbles: true }));
    }
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
      ...volumeState(m)
    };
  }

  function volumeState(m) {
    const p = yt();
    if (p) return { volume: Math.round(p.getVolume()), muted: !!(p.isMuted && p.isMuted()) };
    return { volume: m ? Math.round(m.volume * 100) : null, muted: m ? m.muted : false };
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
          const p = yt();
          if (p) {
            const v = Math.round(Math.max(0, Math.min(100, name === 'setVolume' ? arg : p.getVolume() + arg)));
            ytSet(p, v);
            return v;
          }
          if (!m) return null;
          const v = name === 'setVolume' ? arg : m.volume * 100 + arg;
          want = Math.max(0, Math.min(100, v)) / 100;
          apply(m);
          if (m.muted && v > 0) m.muted = false;
          return Math.round(m.volume * 100);
        }
        case 'toggleMute': {
          const p = yt();
          if (p && p.isMuted) {
            const muted = p.isMuted();
            muted ? p.unMute() : p.mute();
            return !muted;
          }
          if (!m) return null;
          m.muted = !m.muted;
          return m.muted;
        }
        default:
          return null;
      }
    }
  };
})();
