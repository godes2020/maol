(async () => {
  const api = window.moa;
  let info = await api.getInfo();
  const t = MoaI18n.make(info.lang);

  document.title = t('settings.title');
  document.querySelectorAll('[data-i18n]').forEach((el) => { el.textContent = t(el.dataset.i18n); });
  document.getElementById('version').textContent = info.version;

  // ---------------- Tabs ----------------
  document.querySelectorAll('#tabs button').forEach((btn) => {
    btn.addEventListener('click', () => {
      document.querySelectorAll('#tabs button').forEach((b) => b.classList.toggle('active', b === btn));
      document.querySelectorAll('.tab').forEach((s) => s.classList.toggle('active', s.id === `tab-${btn.dataset.tab}`));
    });
  });

  // ---------------- Hotkeys ----------------
  const list = document.getElementById('hotkeyList');
  const msg = document.getElementById('hotkeyMsg');
  let capturing = null;

  const pretty = (accel) => accel.replace(/\+/g, ' + ').replace('Super', 'Win').replace(/\bUp\b/, '↑').replace(/\bDown\b/, '↓').replace(/\bLeft\b/, '←').replace(/\bRight\b/, '→');

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
      btn.textContent = accel ? pretty(accel) : t('settings.unset');
      btn.addEventListener('click', () => startCapture(action, btn));
      row.append(label, btn);
      list.append(row);
    }
    if (info.failed.length) showMsg(t('settings.failed', { keys: info.failed.map(pretty).join(', ') }));
  }

  function startCapture(action, btn) {
    stopCapture();
    capturing = { action, btn };
    btn.classList.add('capturing');
    btn.textContent = t('settings.press');
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
      showMsg(t('settings.needModifier'));
      return;
    }
    const accel = [...mods, key].join('+');
    const owner = Object.entries(info.config.hotkeys).find(([a, k]) => a !== capturing.action && k && k.toLowerCase() === accel.toLowerCase());
    if (owner) {
      showMsg(t('settings.duplicate', { action: t(`action.${owner[0]}`) }));
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

  document.getElementById('resetHotkeys').addEventListener('click', async () => {
    info = await api.resetHotkeys();
    showMsg('');
    renderHotkeys();
  });

  // ---------------- Generic inputs ----------------
  const getPath = (obj, p) => p.split('.').reduce((o, k) => (o ? o[k] : undefined), obj);
  const toPatch = (p, value) => {
    const [section, key] = p.split('.');
    return { [section]: { [key]: value } };
  };

  function formatOutput(input) {
    const out = input.parentElement.querySelector('output');
    if (!out) return;
    const v = Number(input.value);
    out.textContent = input.dataset.format === 'percent' ? `${Math.round(v * 100)}%` : (Number.isInteger(v) ? v : v.toFixed(1));
  }

  document.querySelectorAll('[data-path]').forEach((input) => {
    const value = getPath(info.config, input.dataset.path);
    if (input.type === 'checkbox') input.checked = Boolean(value);
    else input.value = value;
    formatOutput(input);

    const evt = input.type === 'range' ? 'input' : 'change';
    input.addEventListener(evt, async () => {
      let v = input.type === 'checkbox' ? input.checked : input.value;
      if (input.type === 'range') v = Number(v);
      formatOutput(input);
      info = await api.patch(toPatch(input.dataset.path, v));
    });
  });

  document.getElementById('moveOverlay').addEventListener('click', () => api.editOverlay());
  document.getElementById('resetOverlay').addEventListener('click', () => api.resetOverlayPosition());

  renderHotkeys();
})();
