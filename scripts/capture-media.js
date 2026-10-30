// Renders the logo/icons and the README media (screenshots + GIF) with the
// pre-installed Playwright Chromium. Usage: npm run media
// Requires: playwright, ffmpeg (for the GIF).
const fs = require('fs');
const path = require('path');
const http = require('http');
const { execFileSync } = require('child_process');
const { chromium } = require('playwright');

const ROOT = path.join(__dirname, '..');
const MEDIA = path.join(ROOT, 'docs', 'media');
fs.mkdirSync(MEDIA, { recursive: true });

// ---------------------------------------------------------------------------
// Logo — the 5×5 pixel mark, extruded like the 3D render.

const PIXEL_PATH = 'M2 0h3v1H2zM1 1h1v1H1zM4 1h1v1H4zM0 2h1v3H0zM2 2h1v1H2zM4 2h1v1H4zM3 3h1v1H3zM1 4h2v1H1z';

function mark({ unit, x, y, depth = 0.34, glow = true, id = 'm' }) {
  const steps = [];
  for (let d = depth; d > 0; d -= 0.025) {
    steps.push(`<path d="${PIXEL_PATH}" transform="translate(0 ${d.toFixed(3)})" fill="url(#${id}side)" stroke="url(#${id}side)" stroke-width=".1" stroke-linejoin="round"/>`);
  }
  return `
  <defs>
    <linearGradient id="${id}face" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="5" y2="5">
      <stop offset="0" stop-color="#7dd3fc"/>
      <stop offset=".38" stop-color="#a78bfa"/>
      <stop offset=".72" stop-color="#f472b6"/>
      <stop offset="1" stop-color="#fdba74"/>
    </linearGradient>
    <linearGradient id="${id}side" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="5" y2="5">
      <stop offset="0" stop-color="#1e3a8a"/>
      <stop offset=".45" stop-color="#4c1d95"/>
      <stop offset="1" stop-color="#831843"/>
    </linearGradient>
    <linearGradient id="${id}shine" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#fff" stop-opacity=".45"/>
      <stop offset=".5" stop-color="#fff" stop-opacity="0"/>
    </linearGradient>
    <filter id="${id}glow" x="-50%" y="-50%" width="200%" height="200%">
      <feGaussianBlur stdDeviation=".35"/>
    </filter>
  </defs>
  <g transform="translate(${x} ${y}) scale(${unit})">
    ${glow ? `<path d="${PIXEL_PATH}" fill="#ec4899" opacity=".55" filter="url(#${id}glow)" transform="translate(0 .25)"/>` : ''}
    ${steps.join('\n    ')}
    <path d="${PIXEL_PATH}" fill="url(#${id}face)" stroke="url(#${id}face)" stroke-width=".1" stroke-linejoin="round"/>
    <path d="${PIXEL_PATH}" fill="url(#${id}shine)" opacity=".6"/>
  </g>`;
}

function iconSvg(size = 512) {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 512 512">
  <defs>
    <linearGradient id="tile" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#1b1233"/>
      <stop offset="1" stop-color="#0a0712"/>
    </linearGradient>
    <radialGradient id="tglow" cx=".7" cy=".25" r=".8">
      <stop offset="0" stop-color="#a855f7" stop-opacity=".45"/>
      <stop offset="1" stop-color="#a855f7" stop-opacity="0"/>
    </radialGradient>
  </defs>
  <rect x="16" y="16" width="480" height="480" rx="112" fill="url(#tile)"/>
  <rect x="16" y="16" width="480" height="480" rx="112" fill="url(#tglow)"/>
  <rect x="17.5" y="17.5" width="477" height="477" rx="110.5" fill="none" stroke="#fff" stroke-opacity=".08" stroke-width="3"/>
  ${mark({ unit: 60, x: 106, y: 96 })}
</svg>`;
}

function traySvg() {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 64 64">
  ${mark({ unit: 11, x: 4.5, y: 2.5, depth: 0.3, glow: false, id: 't' })}
</svg>`;
}

function markOnlySvg() {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="420" height="420" viewBox="0 0 420 420">
  ${mark({ unit: 70, x: 35, y: 25 })}
</svg>`;
}

// ---------------------------------------------------------------------------
// Static server so the overlay page can read its cover art into a canvas.

// Default config straight from the app, without starting Electron.
function loadDefaults() {
  const Module = require('module');
  const orig = Module._load;
  Module._load = (req, ...rest) => (req === 'electron' ? { app: { getPath: () => '' } } : orig(req, ...rest));
  try {
    return require('../src/main/config');
  } finally {
    Module._load = orig;
  }
}

function serve() {
  const types = { '.html': 'text/html', '.css': 'text/css', '.js': 'text/javascript', '.png': 'image/png', '.svg': 'image/svg+xml' };
  const server = http.createServer((req, res) => {
    const p = path.join(ROOT, decodeURIComponent(new URL(req.url, 'http://x').pathname));
    if (!p.startsWith(ROOT) || !fs.existsSync(p) || fs.statSync(p).isDirectory()) {
      res.writeHead(404);
      return res.end();
    }
    res.writeHead(200, { 'content-type': types[path.extname(p)] || 'application/octet-stream' });
    fs.createReadStream(p).pipe(res);
  });
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve(server)));
}

const SCENE_CSS = `
  html {
    background:
      radial-gradient(60% 70% at 70% 105%, rgba(251,146,60,.55), transparent 60%),
      radial-gradient(45% 55% at 72% 78%, rgba(244,114,182,.55), transparent 70%),
      linear-gradient(180deg, #0b0620 0%, #1d0f3d 55%, #3b1350 75%, #120822 76%, #07040f 100%) !important;
  }
  html::before {
    content: ""; position: fixed; left: -50%; right: -50%; bottom: 0; height: 24%;
    background:
      repeating-linear-gradient(90deg, rgba(167,139,250,.35) 0 2px, transparent 2px 80px),
      repeating-linear-gradient(0deg, rgba(167,139,250,.35) 0 2px, transparent 2px 36px);
    transform: perspective(300px) rotateX(60deg); transform-origin: bottom;
  }
  html::after {
    content: ""; position: fixed; width: 260px; height: 260px; right: 18%; bottom: 23%;
    border-radius: 50%;
    background: repeating-linear-gradient(180deg, #fdba74 0 18px, #f472b6 18px 22px);
    -webkit-mask: linear-gradient(#000 55%, transparent 100%);
    opacity: .9; filter: drop-shadow(0 0 40px rgba(244,114,182,.6));
  }
  body { overflow: visible; }
`;

async function main() {
  const browser = await chromium.launch();

  // ---- Icons ----
  const page = await browser.newPage({ deviceScaleFactor: 1 });
  async function renderSvg(svg, out, size) {
    await page.setViewportSize({ width: size, height: size });
    await page.setContent(`<html><body style="margin:0;background:transparent">${svg.replace(/width="\d+" height="\d+"/, `width="${size}" height="${size}"`)}</body></html>`);
    await page.screenshot({ path: out, omitBackground: true, clip: { x: 0, y: 0, width: size, height: size } });
  }
  fs.mkdirSync(path.join(ROOT, 'build'), { recursive: true });
  fs.mkdirSync(path.join(ROOT, 'assets'), { recursive: true });
  fs.writeFileSync(path.join(MEDIA, 'logo.svg'), iconSvg());
  fs.writeFileSync(path.join(MEDIA, 'mark.svg'), markOnlySvg());
  await renderSvg(iconSvg(), path.join(ROOT, 'build', 'icon.png'), 1024);
  await renderSvg(iconSvg(), path.join(ROOT, 'assets', 'icon.png'), 256);
  await renderSvg(traySvg(), path.join(ROOT, 'assets', 'tray.png'), 32);
  for (const size of [16, 48, 128]) {
    await renderSvg(iconSvg(), path.join(ROOT, 'extension', 'icons', `${size}.png`), size);
  }
  await renderSvg(iconSvg(), path.join(MEDIA, 'logo.png'), 256);

  // ---- Demo cover (original artwork, so no copyrighted covers in the repo) ----
  await page.setViewportSize({ width: 544, height: 544 });
  await page.setContent(`<html><body style="margin:0"><div style="width:544px;height:544px;position:relative;overflow:hidden;
    background:linear-gradient(180deg,#120a2e 0%,#3b0d5c 50%,#ff5e7e 78%,#ffb36b 100%)">
    <div style="position:absolute;left:122px;top:150px;width:300px;height:300px;border-radius:50%;
      background:repeating-linear-gradient(180deg,#ffd27a 0 26px,#ff6aa2 26px 32px);
      -webkit-mask:linear-gradient(#000 60%,transparent 100%)"></div>
    <div style="position:absolute;left:-40%;right:-40%;bottom:0;height:150px;
      background:repeating-linear-gradient(90deg,rgba(255,255,255,.5) 0 2px,transparent 2px 60px),
                 repeating-linear-gradient(0deg,rgba(255,255,255,.5) 0 2px,transparent 2px 28px),#1a0b2e;
      transform:perspective(200px) rotateX(55deg);transform-origin:bottom"></div>
    </div></body></html>`);
  await page.screenshot({ path: path.join(MEDIA, 'demo-cover.png') });

  // ---- Banner ----
  await page.setViewportSize({ width: 1280, height: 400 });
  await page.setContent(`<html><head><style>
      body{margin:0;width:1280px;height:400px;display:flex;align-items:center;gap:56px;padding:0 120px;box-sizing:border-box;
        font-family:"Segoe UI",system-ui,sans-serif;color:#fff;overflow:hidden;
        background:radial-gradient(60% 120% at 85% 0%,rgba(168,85,247,.45),transparent 60%),
                   radial-gradient(50% 120% at 100% 100%,rgba(244,114,182,.35),transparent 60%),
                   radial-gradient(40% 90% at 0% 100%,rgba(56,189,248,.25),transparent 60%),#0b0716}
      h1{margin:0;font-size:120px;line-height:1;letter-spacing:4px;font-weight:800;
        background:linear-gradient(90deg,#7dd3fc,#a78bfa 40%,#f472b6 75%,#fdba74);-webkit-background-clip:text;color:transparent}
      p{margin:14px 0 0;font-size:30px;color:#cfc7ea;letter-spacing:6px;text-transform:uppercase}
    </style></head><body>${markOnlySvg().replace('width="420" height="420"', 'width="250" height="250"')}
    <div><h1>Moa</h1><p>Music Overlay App</p></div></body></html>`);
  await page.screenshot({ path: path.join(MEDIA, 'banner.png') });
  await page.close();

  // ---- Overlay screenshots ----
  const server = await serve();
  const base = `http://127.0.0.1:${server.address().port}/src/overlay/overlay.html`;
  const shots = [
    { name: 'overlay-compact', q: 'lang=en' },
    { name: 'overlay-expanded', q: 'lang=en&expanded' },
    { name: 'overlay-muted', q: 'lang=en&muted' },
    { name: 'overlay-edit', q: 'lang=en&edit' },
    { name: 'overlay-cover', q: 'lang=en&layout=cover' },
    { name: 'overlay-ru', q: 'lang=ru&expanded&title=Полуночная аркада&artist=Неоновые пиксели&repeat=ONE' }
  ];
  const ov = await browser.newPage({ viewport: { width: 336, height: 176 }, deviceScaleFactor: 2 });
  for (const s of shots) {
    await ov.goto(`${base}?${s.q}`);
    await ov.waitForTimeout(900);
    await ov.screenshot({ path: path.join(MEDIA, `${s.name}.png`), omitBackground: true });
  }
  await ov.close();

  // ---- Hero: overlay above a game-like scene ----
  const hero = await browser.newPage({ viewport: { width: 1200, height: 600 }, deviceScaleFactor: 1 });
  await hero.goto(`${base}?lang=en&expanded`);
  await hero.addStyleTag({ content: SCENE_CSS + '.card{top:36px;left:36px;transform:scale(1.5);transform-origin:top left}' });
  await hero.waitForTimeout(900);
  await hero.screenshot({ path: path.join(MEDIA, 'hero.png') });
  await hero.close();

  // ---- Settings window ----
  const { DEFAULTS } = loadDefaults();
  for (const lang of ['en', 'ru']) {
    for (const tab of ['connect', 'overlay']) {
      const settings = await browser.newPage({ viewport: { width: 560, height: tab === 'overlay' ? 1180 : 720 }, deviceScaleFactor: 1 });
      await settings.addInitScript(([cfg, l]) => {
        const info = {
          connector: { connected: true, version: '0.2.0', error: null },
          extensionDir: 'C:\\Users\\you\\AppData\\Local\\Programs\\Moa\\resources\\extension',
          config: cfg, defaults: cfg.hotkeys, failed: [], version: '0.2.0', lang: l
        };
        window.moa = new Proxy({ getInfo: async () => info, patch: async () => info, resetHotkeys: async () => info, onInfo() {} }, {
          get: (t, k) => t[k] || (() => {})
        });
      }, [DEFAULTS, lang]);
      await settings.goto(`http://127.0.0.1:${server.address().port}/src/settings/settings.html?tab=${tab}`);
      await settings.waitForTimeout(400);
      await settings.screenshot({ path: path.join(MEDIA, `settings-${tab}-${lang}.png`) });
      await settings.close();
    }
  }

  // ---- GIFs. A fake clock makes every frame exactly 40 ms apart, so the
  // animation in the GIF runs at its real, smooth speed. ----
  async function recordGif(name, query, timeline, totalFrames) {
    const frames = path.join(MEDIA, '.frames');
    fs.rmSync(frames, { recursive: true, force: true });
    fs.mkdirSync(frames);
    const gp = await browser.newPage({ viewport: { width: 336, height: 176 }, deviceScaleFactor: 2 });
    await gp.clock.install();
    await gp.goto(`${base}?${query}`);
    await gp.addStyleTag({ content: 'html{background:#16121f}' });
    await gp.clock.runFor(1500);
    for (let f = 0; f < totalFrames; f++) {
      const action = timeline[f];
      if (action) await gp.evaluate(action);
      await gp.clock.runFor(40);
      await gp.screenshot({ path: path.join(frames, `f${String(f).padStart(4, '0')}.png`) });
    }
    await gp.close();
    // No dithering: dithered gradients shimmer from frame to frame.
    execFileSync('ffmpeg', ['-y', '-loglevel', 'error', '-framerate', '25', '-i', path.join(frames, 'f%04d.png'),
      '-vf', 'scale=420:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=160:stats_mode=full[p];[b][p]paletteuse=dither=none',
      path.join(MEDIA, `${name}.gif`)]);
    fs.rmSync(frames, { recursive: true, force: true });
  }

  await recordGif('overlay-demo', 'lang=en&time=128', {
    40: () => window.demo.setHover(true),
    85: () => window.demo.showToast('Volume 75%'),
    120: () => { window.demo.state.muted = true; window.demo.applyState(window.demo.state); },
    150: () => { window.demo.state.muted = false; window.demo.applyState(window.demo.state); },
    170: () => window.demo.setHover(false)
  }, 215);

  await recordGif('overlay-cover-demo', 'lang=en&time=60&layout=cover', {
    60: () => window.demo.setHover(true),
    120: () => window.demo.setHover(false)
  }, 170);

  await browser.close();
  server.close();
  console.log('media written to', MEDIA);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
