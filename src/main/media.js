// System media source: runs MoaMedia.exe (helper/MoaMedia), which reads the
// Windows media controls, so any player works without extensions: YouTube
// Music, Spotify, Yandex Music, SoundCloud... in any browser or app.
const { spawn } = require('child_process');
const EventEmitter = require('events');
const fs = require('fs');
const readline = require('readline');

class SystemMedia extends EventEmitter {
  constructor(exe) {
    super();
    this.exe = exe;
    this.proc = null;
    this.running = false;
    this.ready = false;
    this.error = null;
    this.art = new Map();
    this.pending = new Map();
    this.nextId = 1;
    this.restarts = 0;
  }

  get available() {
    return (process.platform === 'win32' || process.env.MOA_MEDIA_HELPER) && fs.existsSync(this.exe);
  }

  start() {
    if (this.running) return;
    this.running = true;
    if (!this.available) {
      this.error = process.platform === 'win32' ? 'MoaMedia.exe not found' : 'Windows only';
      this.emit('status');
      return;
    }
    this.spawn();
  }

  spawn() {
    this.error = null;
    // MOA_MEDIA_HELPER lets tests swap in a stand-in helper script.
    const cmd = process.env.MOA_MEDIA_HELPER ? process.execPath : this.exe;
    const args = process.env.MOA_MEDIA_HELPER ? [this.exe] : [];
    const proc = spawn(cmd, args, {
      env: { ...process.env, ELECTRON_RUN_AS_NODE: '1' },
      windowsHide: true,
      stdio: ['pipe', 'pipe', 'ignore']
    });
    this.proc = proc;
    readline.createInterface({ input: proc.stdout }).on('line', (line) => this.onLine(line));
    proc.on('error', (err) => {
      this.error = err.message;
      this.emit('status');
    });
    proc.on('exit', () => {
      if (this.proc === proc) this.proc = null;
      this.ready = false;
      for (const resolve of this.pending.values()) resolve(null);
      this.pending.clear();
      this.emit('state', { hasTrack: false, playing: false });
      this.emit('status');
      // Restart a crashed helper, backing off if it keeps failing.
      if (this.running && this.restarts < 5) {
        this.restarts++;
        setTimeout(() => this.running && !this.proc && this.spawn(), 2000 * this.restarts);
      }
    });
  }

  onLine(line) {
    let msg;
    try {
      msg = JSON.parse(line);
    } catch {
      return;
    }
    if (msg.type === 'hello') {
      this.ready = true;
      this.restarts = 0;
      this.audioOn = false;
      this.emit('status');
    } else if (msg.type === 'state') {
      this.emit('state', msg);
    } else if (msg.type === 'audio') {
      this.emit('audio', msg);
    } else if (msg.type === 'art') {
      this.art.set(msg.key, msg.data);
      // Only the current and the previous cover are ever needed.
      while (this.art.size > 2) this.art.delete(this.art.keys().next().value);
    } else if (msg.type === 'result') {
      const resolve = this.pending.get(msg.id);
      if (resolve) {
        this.pending.delete(msg.id);
        resolve(msg.value);
      }
    } else if (msg.type === 'error') {
      this.error = msg.message;
      this.emit('status');
    }
  }

  // Beat analysis runs in the helper only while it is needed.
  setAudio(on) {
    if (!this.ready || this.audioOn === on) return;
    this.audioOn = on;
    this.cmd('audio', on ? 1 : 0);
  }

  getArt(key) {
    return this.art.get(key) || null;
  }

  cmd(name, arg) {
    if (!this.proc) return Promise.resolve(null);
    const id = String(this.nextId++);
    return new Promise((resolve) => {
      this.pending.set(id, resolve);
      this.proc.stdin.write(`${id}\t${name}\t${arg ?? ''}\n`);
      setTimeout(() => {
        if (this.pending.delete(id)) resolve(null);
      }, 2500);
    });
  }

  stop() {
    this.running = false;
    if (this.proc) {
      this.proc.stdin.end();
      const proc = this.proc;
      setTimeout(() => proc.exitCode == null && proc.kill(), 1000);
      this.proc = null;
    }
    this.ready = false;
    this.art.clear();
  }
}

module.exports = { SystemMedia };
