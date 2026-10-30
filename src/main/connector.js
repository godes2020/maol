// Local WebSocket server the Moa Connector browser extension talks to.
// Only extension origins on this machine are accepted.
const { EventEmitter } = require('events');
const { WebSocketServer } = require('ws');

const PORT = 47823;

class Connector extends EventEmitter {
  constructor() {
    super();
    this.client = null;
    this.version = null;
    this.error = null;
    this.nextId = 1;
    this.pending = new Map();
  }

  get connected() {
    return Boolean(this.client);
  }

  start() {
    this.wss = new WebSocketServer({
      host: '127.0.0.1',
      port: PORT,
      path: '/moa',
      verifyClient: ({ origin }) => /^(chrome|moz)-extension:\/\//.test(origin || '')
    });
    this.wss.on('error', (err) => {
      this.error = err.code === 'EADDRINUSE' ? `port ${PORT} is busy` : err.message;
      this.emit('status');
    });
    this.wss.on('connection', (ws) => this.attach(ws));
  }

  attach(ws) {
    // The newest connection wins; a browser restart replaces the old socket.
    if (this.client && this.client !== ws) this.client.close();
    this.client = ws;
    this.emit('status');

    ws.on('message', (raw) => {
      let msg;
      try {
        msg = JSON.parse(raw);
      } catch {
        return;
      }
      if (msg.type === 'hello') {
        this.version = msg.version;
        this.emit('status');
      } else if (msg.type === 'state') {
        this.emit('state', msg.payload);
      } else if (msg.type === 'audio') {
        this.emit('audio', msg.payload);
      } else if (msg.type === 'result') {
        const done = this.pending.get(msg.id);
        if (done) done(msg.value);
      }
    });

    ws.on('close', () => {
      if (this.client !== ws) return;
      this.client = null;
      this.version = null;
      this.emit('status');
      this.emit('state', { hasTrack: false, playing: false });
    });
  }

  cmd(name, arg) {
    if (!this.client) return Promise.resolve(null);
    const id = this.nextId++;
    return new Promise((resolve) => {
      const done = (value) => {
        clearTimeout(timer);
        this.pending.delete(id);
        resolve(value ?? null);
      };
      const timer = setTimeout(() => done(null), 2500);
      this.pending.set(id, done);
      this.client.send(JSON.stringify({ type: 'cmd', id, name, arg: arg ?? null }));
    });
  }

  stop() {
    if (this.wss) this.wss.close();
  }
}

module.exports = { Connector, PORT };
