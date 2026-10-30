// Shared UI strings. Loaded with require() in the main process and with a
// plain <script> tag in renderer pages (exposes window.MoaI18n).
(function (root) {
  const strings = {
    en: {
      'app.name': 'Moa',
      'tray.show': 'Show Moa',
      'tray.overlay': 'Show overlay',
      'tray.editOverlay': 'Move overlay',
      'tray.settings': 'Settings…',
      'tray.quit': 'Quit',
      'menu.app': 'Moa',
      'menu.reload': 'Reload',
      'menu.back': 'Back',
      'menu.forward': 'Forward',
      'menu.devtools': 'Developer tools',

      'overlay.idleTitle': 'Nothing playing',
      'overlay.idleArtist': 'Open Moa and pick a song',
      'overlay.editHint': 'Drag me anywhere',
      'overlay.editDone': 'to finish',
      'overlay.volume': 'Volume',
      'overlay.muted': 'Muted',
      'overlay.repeatOff': 'Repeat off',
      'overlay.repeatAll': 'Repeat playlist',
      'overlay.repeatOne': 'Repeat track',
      'overlay.mute': 'Mute / unmute',
      'overlay.prev': 'Previous',
      'overlay.next': 'Next',
      'overlay.repeat': 'Repeat',
      'overlay.openApp': 'Open Moa',
      'overlay.hide': 'Hide overlay',
      'overlay.playPause': 'Play / pause',

      'settings.title': 'Moa — Settings',
      'settings.hotkeys': 'Hotkeys',
      'settings.hotkeysHint': 'Click a shortcut and press a new combination. Esc — cancel, Backspace — unbind. Hotkeys work everywhere, even inside games.',
      'settings.overlay': 'Overlay',
      'settings.playback': 'Playback',
      'settings.general': 'General',
      'settings.press': 'Press keys…',
      'settings.unset': 'Not set',
      'settings.resetHotkeys': 'Reset hotkeys to defaults',
      'settings.duplicate': 'This combination is already used by “{action}”.',
      'settings.failed': 'Could not register: {keys}. Another app is probably using this combination.',
      'settings.needModifier': 'Use at least one modifier (Ctrl, Alt, Shift, Win) or an F-key.',
      'settings.overlayVisible': 'Show overlay',
      'settings.overlayAlwaysExpanded': 'Always show full player (not only on hover)',
      'settings.overlayHideWhenIdle': 'Hide overlay when nothing is playing',
      'settings.overlayOpacity': 'Opacity',
      'settings.overlayScale': 'Size',
      'settings.overlayReactive': 'Music-reactive background',
      'settings.overlayIntensity': 'Animation intensity',
      'settings.overlayMove': 'Move overlay',
      'settings.overlayReset': 'Reset position',
      'settings.seekStep': 'Seek step, seconds',
      'settings.volumeStep': 'Volume step, %',
      'settings.language': 'Language',
      'settings.languageAuto': 'System',
      'settings.autostart': 'Launch Moa when Windows starts',
      'settings.closeToTray': 'Closing the window keeps Moa running in the tray',
      'settings.startHidden': 'Start minimized to tray',
      'settings.restartNote': 'Language changes apply after restarting Moa.',
      'settings.version': 'Version',

      'action.playPause': 'Play / pause',
      'action.next': 'Next track',
      'action.previous': 'Previous track',
      'action.seekForward': 'Seek forward',
      'action.seekBackward': 'Seek backward',
      'action.volumeUp': 'Volume up',
      'action.volumeDown': 'Volume down',
      'action.toggleMute': 'Mute / unmute',
      'action.cycleRepeat': 'Repeat: off → playlist → track',
      'action.toggleOverlay': 'Show / hide overlay',
      'action.editOverlay': 'Move overlay (edit mode)',
      'action.showApp': 'Show / hide Moa window',
      'action.openSettings': 'Open settings'
    },
    ru: {
      'app.name': 'Moa',
      'tray.show': 'Открыть Moa',
      'tray.overlay': 'Показывать оверлей',
      'tray.editOverlay': 'Переместить оверлей',
      'tray.settings': 'Настройки…',
      'tray.quit': 'Выход',
      'menu.app': 'Moa',
      'menu.reload': 'Перезагрузить',
      'menu.back': 'Назад',
      'menu.forward': 'Вперёд',
      'menu.devtools': 'Инструменты разработчика',

      'overlay.idleTitle': 'Ничего не играет',
      'overlay.idleArtist': 'Открой Moa и выбери песню',
      'overlay.editHint': 'Перетащи куда угодно',
      'overlay.editDone': '— готово',
      'overlay.volume': 'Громкость',
      'overlay.muted': 'Без звука',
      'overlay.repeatOff': 'Повтор выключен',
      'overlay.repeatAll': 'Повтор плейлиста',
      'overlay.repeatOne': 'Повтор трека',
      'overlay.mute': 'Звук вкл / выкл',
      'overlay.prev': 'Предыдущий',
      'overlay.next': 'Следующий',
      'overlay.repeat': 'Повтор',
      'overlay.openApp': 'Открыть Moa',
      'overlay.hide': 'Скрыть оверлей',
      'overlay.playPause': 'Пауза / play',

      'settings.title': 'Moa — Настройки',
      'settings.hotkeys': 'Горячие клавиши',
      'settings.hotkeysHint': 'Нажми на сочетание и введи новое. Esc — отмена, Backspace — убрать бинд. Бинды работают везде, даже в играх.',
      'settings.overlay': 'Оверлей',
      'settings.playback': 'Воспроизведение',
      'settings.general': 'Общие',
      'settings.press': 'Нажми клавиши…',
      'settings.unset': 'Не задано',
      'settings.resetHotkeys': 'Сбросить бинды по умолчанию',
      'settings.duplicate': 'Это сочетание уже занято: «{action}».',
      'settings.failed': 'Не удалось назначить: {keys}. Скорее всего, сочетание занято другой программой.',
      'settings.needModifier': 'Нужен хотя бы один модификатор (Ctrl, Alt, Shift, Win) или F-клавиша.',
      'settings.overlayVisible': 'Показывать оверлей',
      'settings.overlayAlwaysExpanded': 'Всегда показывать полный плеер (не только при наведении)',
      'settings.overlayHideWhenIdle': 'Скрывать оверлей, когда ничего не играет',
      'settings.overlayOpacity': 'Прозрачность',
      'settings.overlayScale': 'Размер',
      'settings.overlayReactive': 'Фон реагирует на музыку',
      'settings.overlayIntensity': 'Сила анимации',
      'settings.overlayMove': 'Переместить оверлей',
      'settings.overlayReset': 'Сбросить позицию',
      'settings.seekStep': 'Шаг перемотки, секунд',
      'settings.volumeStep': 'Шаг громкости, %',
      'settings.language': 'Язык',
      'settings.languageAuto': 'Системный',
      'settings.autostart': 'Запускать Moa вместе с Windows',
      'settings.closeToTray': 'При закрытии окна Moa остаётся в трее',
      'settings.startHidden': 'Запускать свёрнутым в трей',
      'settings.restartNote': 'Язык сменится после перезапуска Moa.',
      'settings.version': 'Версия',

      'action.playPause': 'Пауза / play',
      'action.next': 'Следующий трек',
      'action.previous': 'Предыдущий трек',
      'action.seekForward': 'Перемотка вперёд',
      'action.seekBackward': 'Перемотка назад',
      'action.volumeUp': 'Громче',
      'action.volumeDown': 'Тише',
      'action.toggleMute': 'Звук вкл / выкл',
      'action.cycleRepeat': 'Повтор: выкл → плейлист → трек',
      'action.toggleOverlay': 'Показать / скрыть оверлей',
      'action.editOverlay': 'Переместить оверлей (режим настройки)',
      'action.showApp': 'Показать / скрыть окно Moa',
      'action.openSettings': 'Открыть настройки'
    }
  };

  function resolveLang(pref, systemLocale) {
    if (pref === 'en' || pref === 'ru') return pref;
    const loc = String(systemLocale || '').toLowerCase();
    return /^(ru|uk|be|kk)/.test(loc) ? 'ru' : 'en';
  }

  function make(lang) {
    const dict = strings[lang] || strings.en;
    return function t(key, vars) {
      let s = dict[key] != null ? dict[key] : (strings.en[key] != null ? strings.en[key] : key);
      if (vars) for (const k of Object.keys(vars)) s = s.replace('{' + k + '}', vars[k]);
      return s;
    };
  }

  const api = { strings, resolveLang, make };
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
  else root.MoaI18n = api;
})(typeof window !== 'undefined' ? window : globalThis);
