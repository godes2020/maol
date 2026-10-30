<div align="center">

<img src="media/logo.png" width="96" alt="Moa">

# Installing Moa

**English** · [Русский](INSTALL.ru.md) · [← Back to README](../README.md)

</div>

## Requirements

- Windows 10 or Windows 11 (64-bit)
- About 300 MB of free disk space
- A Google account (only if you want your own playlists and library; you can listen without signing in)

## 1. Download

Go to the [**latest release**](https://github.com/godes2020/maol/releases/latest) and pick one file under **Assets**:

| File | What it is |
|---|---|
| `Moa-Setup-x.y.z.exe` | **Recommended.** A normal installer that adds Start menu and desktop shortcuts and an uninstaller. |
| `Moa-x.y.z-portable.exe` | A single file that runs without installation. Good for a USB stick or if you'd rather not install anything. |

## 2. Install

1. Double-click `Moa-Setup-x.y.z.exe`.
2. **If Windows shows a blue “Windows protected your PC” window**, click **More info**, then **Run anyway**.
   This happens because Moa is a free open-source app without a paid code-signing certificate. You can read every line of the source code in this repository.
3. Choose the installer language, then choose whether to install for yourself only (no admin rights needed) and which folder to use.
4. Click **Install**, then **Finish**. Moa starts.

## 3. First launch

1. Moa opens a small window with YouTube Music.
2. Click **Sign in** in the top-right corner and log in to your Google account the same way you would in a browser.
3. Open any playlist, album or song and press play.
4. The **overlay** appears in the top-left corner of your screen.
5. To move it, press <kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>M</kbd>, drag it, and press <kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>M</kbd> again.

<div align="center"><img src="media/overlay-edit.png" width="320" alt="Overlay edit mode"></div>

> **Tip:** closing the Moa window doesn't stop the music. Moa keeps running in the system tray (the area near the clock, which may be hidden behind the **^** arrow). Right-click the icon to open **Settings** or **Quit**.

## 4. Games

- Set the game to **Borderless** or **Windowed fullscreen** to see the overlay on top of it.
- In **exclusive fullscreen** the overlay is hidden, but all hotkeys still work.
- If a game uses one of Moa's hotkeys, change it in **Settings → Hotkeys**.

## Updating

Download the newer `Moa-Setup-x.y.z.exe` from [Releases](https://github.com/godes2020/maol/releases) and run it. It installs over the old version and keeps your settings and Google session.

> To get notified about new versions, click **Watch → Custom → Releases** at the top of the repository page.

## Uninstalling

**Settings → Apps → Installed apps → Moa → Uninstall** (or *Control Panel → Programs*).

Your settings and Google session stay in `%APPDATA%\Moa`. Delete that folder if you want to remove everything.

## Troubleshooting

| Problem | Fix |
|---|---|
| The overlay doesn't appear | Make sure a song is playing and the overlay is enabled (<kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>O</kbd>, or tray → *Show overlay*). If it went off-screen, use **Settings → Overlay → Reset position**. |
| A hotkey does nothing | It's taken by another app. It's highlighted orange in **Settings → Hotkeys**, so pick a different one. |
| Black or white window after sign-in | Press <kbd>Ctrl</kbd>+<kbd>R</kbd> in the Moa window to reload. |
| Google says the browser may not be secure | Wait a minute and try again, or use a different sign-in method (for example, a phone prompt). |
| Antivirus warns about the file | That's a false positive for unsigned apps. Download only from this repository's Releases page. |

Still stuck? [Open an issue](https://github.com/godes2020/maol/issues/new) and describe what happened, including your Windows version and Moa version.
