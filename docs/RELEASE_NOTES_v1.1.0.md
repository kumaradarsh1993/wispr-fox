# v1.1.0 — Hardened.

Ten nightly builds worth of reliability, security, and cross-platform
polish. The dictation pipeline was already solid; this release makes
the infrastructure around it solid too.

If you were on a nightly, this is the "those fixes are now stable" release.

---

## 🔒 Security pass

Three things that should have shipped in v1.0.0, now properly locked down:

1. **Content Security Policy.** The app now declares a strict CSP in
   `tauri.conf.json` — only `'self'`, `ipc:`, `http://ipc.localhost`,
   `asset:`, `http://asset.localhost`, and `data:` are permitted.
   Previously it was wide open (`null`).

2. **API keys are keyring-only.** Groq and Gemini keys now go
   exclusively through Windows Credential Manager (or macOS Keychain /
   Linux Secret Service). The old plaintext fallback path is removed.
   Existing plaintext entries are migrated on first launch and deleted.

3. **No unscoped filesystem access.** The `fs` plugin was removed
   entirely — the app doesn't need arbitrary filesystem access.

---

## 🍎 macOS catches up

- **Native hotkey defaults.** Mac function keys send media events, not
  F8/F9. macOS now defaults to `Ctrl+Alt+D` (dictate), `Ctrl+Alt+F`
  (draft), `Ctrl+Alt+C` (force-clean), `+Shift` for sticky variants.
  Existing Mac installs with stale F8/F9 bindings are auto-migrated.

- **Transparent floater works.** The `macOSPrivateApi` flag is now set,
  so the floating character renders with a proper transparent background
  instead of an opaque white block.

- **Accessibility nudge.** A dismissible banner appears if macOS
  Accessibility permission isn't granted yet (needed for auto-paste).

- **DMG builds ship with every release.** CI now produces
  `wispr-fox_*_aarch64.dmg` alongside Windows NSIS/MSI and Linux
  AppImage/deb/rpm. No more platform drift.

---

## ⚡ Pipeline reliability

- **Floater auto-recovery after sleep.** The floating character no
  longer vanishes when the machine wakes from sleep. A JS-side
  heartbeat detects gaps and triggers a hide/show/repaint cycle.

- **Transient STT retries.** Groq 503/529 errors during transcription
  now retry once after a 1-second pause instead of failing immediately.

- **Pipeline latency cut.** Removed a 500 ms artificial delay between
  STT and LLM stages. The status line now shows which stage is active
  ("Transcribing · Groq" / "Polishing · Groq").

- **Audio tail preserved.** The WAV writer now flushes a final
  partial buffer on stop, so the last fraction of a second isn't
  silently clipped.

---

## 🎨 Visual polish

- **Foxy thought bubble.** The fox floater now shows a speech/thought
  bubble during transcription and processing, with state-aware text.

- **Smaller fox footprint.** The fox in the floater is slightly smaller,
  leaving more room for the bubble without crowding the window.

- **Beige skin retired.** The cream-coloured paperclip variant was
  removed — feedback was it read as bland next to fox + stylized +
  real-clippy. Saved "beige" preferences auto-migrate to "stylized".

- **Sidebar overflow fix.** The sidebar no longer overflows on short
  windows — hotkey hints and usage stats scroll properly.

---

## 📦 Upgrading

Download from the [Releases page](https://github.com/kumaradarsh1993/wispr-fox/releases/latest).
Settings, history, and audio files carry over automatically.

macOS first launch: right-click the app → Open (or
`xattr -dr com.apple.quarantine /Applications/wispr-fox.app`).
