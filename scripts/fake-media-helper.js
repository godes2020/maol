// Stand-in for MoaMedia.exe so the system source can be tried on any OS:
//   MOA_MEDIA_HELPER=scripts/fake-media-helper.js npm start
const fs = require('fs');
const path = require('path');
const readline = require('readline');

const out = (o) => process.stdout.write(JSON.stringify(o) + '\n');
const cover = fs.readFileSync(path.join(__dirname, '..', 'docs', 'media', 'demo-cover.png')).toString('base64');
const s = {
  hasTrack: true, playing: true, title: 'Midnight Arcade', artist: 'Neon Pixels', album: '',
  app: 'Chrome', artwork: 'art1', duration: 170, canSeek: true, canNext: true, canPrevious: true,
  repeat: 'NONE', volume: 70, muted: false, currentTime: 42
};
let at = Date.now();
const now = () => (s.playing ? s.currentTime + (Date.now() - at) / 1000 : s.currentTime);

out({ type: 'hello', version: 1 });
out({ type: 'art', key: 'art1', data: `data:image/png;base64,${cover}` });
if (process.env.FAKE_IDLE) s.hasTrack = s.playing = false;
const send = () => out({ type: 'state', ...s, currentTime: now() });
setInterval(send, 1000);
send();

// Synthetic 124 bpm beat for the glow.
let audioTimer = 0;
function setAudio(on) {
  clearInterval(audioTimer);
  if (!on) return out({ type: 'audio', energy: 0, bass: 0, beat: false, bands: [0, 0, 0, 0, 0], bpm: 0 });
  const beatMs = 60000 / 124;
  let last = 0;
  audioTimer = setInterval(() => {
    const t = Date.now();
    const kick = Math.exp(-((t % beatMs) / beatMs) * 6);
    const beat = t - last > beatMs * 0.9 && t % beatMs < 40;
    if (beat) last = t;
    out({ type: 'audio', energy: 0.35 + 0.35 * kick, bass: 0.3 + 0.65 * kick, beat, bpm: 124, bands: [0.25 + 0.6 * kick, 0.5, 0.45, 0.35, 0.3] });
  }, 33);
}

readline.createInterface({ input: process.stdin }).on('line', (line) => {
  const [id, name, arg] = line.split('\t');
  let value = true;
  s.currentTime = now();
  at = Date.now();
  if (name === 'playPause') s.playing = !s.playing;
  else if (name === 'seekTo') s.currentTime = Number(arg);
  else if (name === 'seekBy') s.currentTime = Math.max(0, s.currentTime + Number(arg));
  else if (name === 'volumeBy') value = s.volume = Math.max(0, Math.min(100, s.volume + Number(arg)));
  else if (name === 'setVolume') value = s.volume = Number(arg);
  else if (name === 'toggleMute') value = s.muted = !s.muted;
  else if (name === 'audio') setAudio(arg === '1');
  else if (name === 'cycleRepeat') value = s.repeat = { NONE: 'ALL', ALL: 'ONE', ONE: 'NONE' }[s.repeat];
  out({ type: 'result', id, value });
  send();
}).on('close', () => process.exit(0));
