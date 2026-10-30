# Moa native: handoff notes

Native rewrite of Moa in Rust + Slint 1.18 (`native/`). The Electron 0.3.1 app in `src/` and `helper/` is still the released version.

## Status
- Builds and runs on Linux (`cargo build --release`). The default `skia` feature uses the skia-software renderer and takes about 25–31 MB private memory with both windows open.
- On Linux the media source is a demo (`src/media/mock.rs`). The env vars `MOA_DEMO_IDLE=1`, `MOA_DEMO_COVER=1`, `MOA_TAB=home|overlay|keys|guide|settings` and `MOA_CONFIG=<path>` are for testing.
- The UI is `ui/*.slint`, the strings are `ui/strings.json` and the in-app guide is `ui/guide.json`.

## Next steps (in order)
1. **Layout.** Buttons were cramped to the left: the bottom tab bar, the platform tiles on the idle home screen, chips, style tiles and segmented buttons. I added `horizontal-stretch: 1` at the use sites and `viewport-width: self.width` in Page (`ui/controls.slint`), but have not checked the result. Take screenshots of every tab under xvfb and fix what is still wrong.
2. **Windows compile.** Run `cargo check --target x86_64-pc-windows-msvc --no-default-features` and fix `src/media/system_win.rs`, `src/platform/win.rs`, `src/tray.rs` and `src/keys.rs`.
3. **CI.** Add a workflow job that builds the native exe on `windows-latest`, then extend release.yml (inputs `publish: "true"`) and publish a 0.5.0 pre-release.
4. Spotify OAuth PKCE (loopback `http://127.0.0.1:47825/callback`) in `src/spotify.rs`.
5. A WebView2 player window, loaded on demand, in `src/player.rs`.
6. README/INSTALL updates, plus a RAM measurement on Windows.

## Rules
- Work on branch `claude/tender-noether-b7vxj4`. Do not open a PR unless asked.
- Use official auth only: no cookie copying, user-agent spoofing or unofficial APIs.
