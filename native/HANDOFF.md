# Moa native: handoff notes

Native rewrite of Moa in Rust + Slint 1.18 (`native/`). The Electron 0.3.1 app in `src/` and `helper/` is still the released version.

## Status
- Builds and runs on Linux (`cargo build --release`). The default `skia` feature uses the skia-software renderer and takes about 25–31 MB private memory with both windows open.
- On Linux the media source is a demo (`src/media/mock.rs`). The env vars `MOA_DEMO_IDLE=1`, `MOA_DEMO_COVER=1`, `MOA_TAB=home|overlay|keys|guide|settings` and `MOA_CONFIG=<path>` are for testing.
- The UI is `ui/*.slint`, the strings are `ui/strings.json` and the in-app guide is `ui/guide.json`.

## Done
1. Layout: checked every tab under xvfb and fixed it.
2. Windows compile: the build is clean on `x86_64-pc-windows-msvc`.
3. CI: the `native` job in ci.yml builds the exe on `windows-latest` and runs `scripts/smoke.ps1`. The release.yml inputs `publish: "true", native: "true"` publish the **v0.5.0 pre-release** with `Moa-0.5.0-preview-x64.exe` and `Moa-Setup-0.5.0-preview-x64.exe` (Inno Setup, `installer/moa.iss`: per-user, runs the uninstallers of older Moa builds, keeps `config.json` and `WebView2`).
4. Spotify OAuth PKCE (`src/spotify.rs`): loopback `http://127.0.0.1:47825/callback`, refresh token only, display name from `/v1/me`.
5. WebView2 player (`src/player.rs`, `src/player-bridge.js`, `ui/player.slint`): wry 0.57 builds a child webview in a Slint window on first open. The bridge reads Media Session and media elements and runs commands through the site's own media-key handlers.
6. README/INSTALL now have a "Native preview 0.5.0" section. Idle RAM on Windows (CI smoke test) is **10.6 MB private, 38.3 MB working set**.

7. Feedback fixes after the first 0.5.0 test:
   - Overlay on the software renderer is a layered window with color key RGB(1,2,3) (`platform::win::apply_style`). Glows outside the card are off when keyed. The GPU renderer is keyed too: it showed black on Windows as well.
   - The GPU switch is stored as `general.gpuNative`: Moa 0.3 had `gpu: true` by default, and carrying it over started the native app on the GPU renderer.
   - winit rewrites GWL_EXSTYLE on show/hide, so `keep_on_top` (every 3 s) and `set_click_through` re-apply TOOLWINDOW|NOACTIVATE.
   - Beat analysis (WASAPI loopback) runs for every source while music plays.
   - Animations use `Track.clock`, which only advances while playing, instead of `animation-tick()`.
   - WebView2: one renderer process, no background networking, `MemoryUsageLevel::Low` while hidden.
   - Hotkeys: failed ones are retried every 5 s; the message names the old Moa 0.3 when `Moa.exe` is running.
   - Pixel-art mark snapped to whole pixels; icon-only buttons centre the icon exactly.

## Next steps
- Wait for user feedback on 0.5.0. The built-in player and Spotify were not tested by hand on Windows: CI only checks that the app starts.
- Spotify is sign-in only for now. Next: playlists and playback control through the Web API (`user-modify-playback-state` is already in the scopes).
- Measure RAM with the player open and with music playing on a real machine, and add the numbers to the README memory section.

## Rules
- Work on branch `claude/tender-noether-b7vxj4`. Do not open a PR unless asked.
- Use official auth only: no cookie copying, user-agent spoofing or unofficial APIs.
