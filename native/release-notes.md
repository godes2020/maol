## Moa 0.5.0 — native preview · нативная тестовая версия

A test build of the new Moa, rewritten in Rust + Slint. It is a single small `.exe` that uses far less memory than 0.3.x. Not everything from 0.3.1 is in yet: Spotify sign-in and the built-in player come in later builds.

Тестовая сборка новой Moa, переписанной на Rust + Slint. Это один небольшой `.exe`, памяти он берёт намного меньше, чем 0.3.x. Пока есть не всё из 0.3.1: вход в Spotify и встроенный плеер появятся в следующих сборках.

## ⬇ Download / Скачать

| File / Файл | |
|---|---|
| **`Moa-0.5.0-preview-x64.exe`** | Portable, no installation: download and run · Без установки: скачай и запусти |

> Windows may say *“Windows protected your PC”*. Click **More info → Run anyway** (the app isn't code-signed yet).<br>
> Windows может показать *«Система Windows защитила ваш компьютер»*. Нажми **Подробнее → Выполнить в любом случае** (у приложения пока нет подписи).

Settings carry over from Moa 0.3.1 (`%APPDATA%\Moa`). 0.3.1 can stay installed: close it before starting the preview.

Настройки переносятся из Moa 0.3.1 (`%APPDATA%\Moa`). 0.3.1 можно не удалять: просто закрой её перед запуском тестовой версии.

## What to check / Что проверить

- Music from any player shows on Home and in the overlay · Музыка из любого плеера видна на главной и в оверлее
- Play/pause, next, previous, seek, volume from Moa and from the hotkeys · Пауза, следующий, предыдущий, перемотка и громкость из Moa и по биндам
- Moving and resizing the overlay (Alt+Shift+M), Card and Cover styles · Перемещение и размер оверлея (Alt+Shift+M), виды «Карточка» и «Обложка»
- Tray icon, close to tray, start with Windows · Иконка в трее, сворачивание в трей, автозапуск
- Memory use in Task Manager · Потребление памяти в диспетчере задач
