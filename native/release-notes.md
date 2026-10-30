## Moa 0.5.0 — native preview · нативная тестовая версия

A test build of the new Moa, rewritten in Rust + Slint. It is a single small `.exe` that uses far less memory than 0.3.x. Spotify sign-in and the built-in player (Yandex Music, SoundCloud, VK, YouTube Music) are new and need testing.

Тестовая сборка новой Moa, переписанной на Rust + Slint. Это один небольшой `.exe`, памяти он берёт намного меньше, чем 0.3.x. Вход в Spotify и встроенный плеер (Яндекс Музыка, SoundCloud, VK, YouTube Music) — новые, их особенно нужно проверить.

## ⬇ Download / Скачать

| File / Файл | |
|---|---|
| **`Moa-Setup-0.5.0-preview-x64.exe`** | **Installer (recommended).** Removes Moa 0.3 and older builds first, adds Start menu and desktop shortcuts · **Установщик (рекомендуется).** Сначала удаляет Moa 0.3 и старые сборки, создаёт ярлыки в «Пуске» и на рабочем столе |
| `Moa-0.5.0-preview-x64.exe` | Portable, no installation: download and run · Без установки: скачай и запусти |

> Windows may say *“Windows protected your PC”*. Click **More info → Run anyway** (the app isn't code-signed yet).<br>
> Windows может показать *«Система Windows защитила ваш компьютер»*. Нажми **Подробнее → Выполнить в любом случае** (у приложения пока нет подписи).

Settings carry over from Moa 0.3.1 (`%APPDATA%\Moa`). The installer closes Moa, runs the old version's uninstaller and clears its caches; settings and player sign-ins stay. With the portable exe, uninstall 0.3.1 yourself or close it first.

Настройки переносятся из Moa 0.3.1 (`%APPDATA%\Moa`). Установщик закрывает Moa, удаляет старую версию её же деинсталлятором и чистит её кэши; настройки и входы в плеере остаются. С портативным exe удали 0.3.1 сам или хотя бы закрой её.

## Fixed in this build / Исправлено в этой сборке

- The overlay no longer turns into a black box after a while: the window keeps its transparency when Windows resets its style, and is fully repainted when Windows throws its pixels away · Оверлей больше не превращается в чёрный прямоугольник: окно не теряет прозрачность, когда Windows сбрасывает его стиль, и полностью перерисовывается, когда Windows стирает его содержимое
- Moa's volume in the built-in player changes YouTube Music's own volume, so it no longer jumps back after a few seconds; other services keep the volume across tracks · Громкость из Moa во встроенном плеере меняет собственную громкость YouTube Music и больше не возвращается через несколько секунд; в других сервисах она сохраняется между треками
- Sliders in settings and the player can be dragged, not only clicked · Ползунки в настройках и плеере теперь перетаскиваются, а не только нажимаются
- An installer that removes the old Moa first · Установщик, который сначала удаляет старую Moa
- The overlay background is transparent on both renderers (the GPU setting from 0.3 no longer carries over) · Фон оверлея прозрачный при любом способе отрисовки (настройка GPU из 0.3 больше не переносится)
- The audio animation adapts to the track's loudness, so loud tracks keep moving instead of sitting at maximum · Анимация под звук подстраивается под громкость трека: на громких треках она больше не упирается в максимум
- The overlay no longer has a black background and stays a floating layer, not a separate window · У оверлея больше нет чёрного фона, и он остаётся плавающим слоем, а не отдельным окном
- The beat animation works with the built-in player too · Анимация под бит работает и со встроенным плеером
- The home logo and its animation are drawn on whole pixels; icon buttons are centred · Логотип на главной и его анимация рисуются по целым пикселям; иконки в кнопках по центру
- Hotkeys taken by the old Moa 0.3 are retried automatically, and Moa says what holds them · Бинды, занятые старой Moa 0.3, назначаются повторно сами, и Moa пишет, кто их держит
- Less CPU and memory: no redraws while music is paused; the player uses one renderer process and frees memory when hidden · Меньше нагрузка на процессор и память: без перерисовки на паузе; плеер работает в одном процессе отрисовки и освобождает память, когда скрыт

## What to check / Что проверить

- Music from any player shows on Home and in the overlay · Музыка из любого плеера видна на главной и в оверлее
- Play/pause, next, previous, seek, volume from Moa and from the hotkeys · Пауза, следующий, предыдущий, перемотка и громкость из Moa и по биндам
- Moving and resizing the overlay (Alt+Shift+M), Card and Cover styles · Перемещение и размер оверлея (Alt+Shift+M), виды «Карточка» и «Обложка»
- Tray icon, close to tray, start with Windows · Иконка в трее, сворачивание в трей, автозапуск
- Memory use in Task Manager · Потребление памяти в диспетчере задач
- Built-in player: Settings → Music source → Player in Moa, pick a service, Open player; sign in once, then controls and the overlay work · Встроенный плеер: Настройки → Откуда музыка → Плеер в Moa, выбери сервис, «Открыть плеер»; войди один раз, дальше работают управление и оверлей
- Spotify: Settings → Spotify account, paste your Client ID (Guide → Spotify) and sign in in the browser · Spotify: Настройки → Аккаунт Spotify, вставь свой Client ID (Гайд → Spotify) и войди в браузере
