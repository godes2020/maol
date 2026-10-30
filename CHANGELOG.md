# Changelog

## 0.3.0

- **Any player, no extension.** The new default source reads the Windows media controls through a tiny helper (`MoaMedia.exe`), so YouTube Music, Spotify, Yandex Music, SoundCloud, Apple Music, Deezer, VK, TIDAL and others work in any browser or app. You sign in on the service itself. Volume hotkeys change the player's slider in the Windows mixer.
- **One Moa window** with a new design: now playing with seek and volume, quick toggles, overlay settings, hotkeys and settings. It has its own title bar, a bottom tab bar, custom sliders, switches and scrollbars, icons, and one-click links to the music services.
- **Overlay:** resize it by dragging the corner in move mode. The card mirrors itself on the right half of the screen (or set the side manually), unfolds upwards near the bottom, and a move-mode toolbar can mirror it, switch the style or finish.
- **Cover style:** on hover, controls appear on top of the dimmed cover instead of unfolding a card.
- **Less memory:** beat analysis moved into the helper (WASAPI loopback), the Moa window is unloaded when closed, only the active source runs, blur filters were replaced with cheaper effects, the animation idles when still, and hardware acceleration can be turned off.
- The browser extension and the built-in window remain as optional sources.

## 0.2.1

- **Firefox and Zen support.** Moa Connector now also works in Firefox 128+ and Firefox-based browsers (Zen, LibreWolf, Floorp). The release includes `Moa-Connector-firefox.xpi`, and the install guide has the steps.

## 0.2.0

- **Sign in through your browser.** Google blocks sign-in inside app windows, so music now plays in your own browser. The new **Moa Connector** extension (Chrome, Edge, Yandex, Opera, Brave) links the YouTube Music tab to Moa. The built-in window remains as an option for listening without an account.
- **Locked overlay (default).** Clicks pass through the overlay into the game. Buttons work only in move mode (Alt+Shift+M). “Opens on hover” is available in the settings or with Alt+Shift+U.
- **“Cover only” style.** Just the cover with its progress ring and beat glow. It unfolds into the full player when needed.
- **Smooth animation.** No more flicker: values ease every frame, and a soft glow breathes with the beat instead of flashing.
- **No extra window.** Like Discord, Moa is just the overlay and a tray icon. The window is opt-in.
- **Every setting explained**, in English and Russian. New **Connection** tab with setup steps and a live status.
- Readable YouTube cookie dialog in the built-in window.
- Fixed repeat, play and next buttons in newer YouTube Music layouts.

## 0.1.0 — first release

- YouTube Music in a compact desktop window with Google sign-in
- Always-on-top click-through overlay: cover with progress ring, title, artist, live equalizer / mute button
- Expands on hover: seek bar, play/pause, previous/next, repeat, open app, hide overlay
- Music-reactive background: colours from the cover art, bass pulse, beat flashes, tempo-synced drift
- Global hotkeys for play/pause, next/previous, seek, volume, mute, repeat, overlay toggle and move mode (Alt+Shift+M)
- Settings window: rebind hotkeys, overlay opacity/size/behaviour, seek & volume steps, language, autostart, tray
- English and Russian interface
- Windows installer and portable build via GitHub Actions
