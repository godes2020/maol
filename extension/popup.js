const ru = /^(ru|uk|be|kk)/i.test(navigator.language);
const T = ru
  ? { moaOk: 'Moa подключена', moaBad: 'Moa не запущена', ytmOk: 'Вкладок YouTube Music: ', ytmBad: 'YouTube Music не открыта', hint: 'Запусти Moa и открой <a href="https://music.youtube.com" target="_blank">music.youtube.com</a>. Если вкладка уже была открыта, обнови её (F5).' }
  : { moaOk: 'Connected to Moa', moaBad: 'Moa is not running', ytmOk: 'YouTube Music tabs: ', ytmBad: 'YouTube Music is not open', hint: 'Start Moa and open <a href="https://music.youtube.com" target="_blank">music.youtube.com</a>. If the tab was already open, reload it (F5).' };

function set(id, ok, text) {
  const el = document.getElementById(id);
  el.className = 'row ' + (ok ? 'ok' : 'bad');
  el.lastElementChild.textContent = text;
}

function refresh() {
  chrome.runtime.sendMessage({ type: 'status' }, (s) => {
    if (!s) return;
    set('moa', s.moa, s.moa ? T.moaOk : T.moaBad);
    set('ytm', s.tabs > 0, s.tabs > 0 ? T.ytmOk + s.tabs : T.ytmBad);
    document.getElementById('hint').innerHTML = s.moa && s.tabs ? '' : T.hint;
  });
}
refresh();
setInterval(refresh, 1000);
