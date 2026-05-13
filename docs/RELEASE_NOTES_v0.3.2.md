# v0.3.2 — Critical hotfix: Clippy back, settings persist

Real fixes for three regressions in v0.3.0/v0.3.1.

---

## 🐛 Fixes

### Clippy vanished entirely

Selecting any skin (Paperclip / Cream / Clippy) showed an empty floater
window. Clippy was gone.

**Root cause:** the v0.3.1 hotfix removed the `clickWiggling` state
variable from the script but left one stale `class:wiggle={clickWiggling}`
binding in the SVG template. Svelte 5 throws a `ReferenceError` on
mount and the entire Clippy page fails to render. None of the skins
showed up because the component itself never mounted.

v0.3.2 removes the orphaned binding. Clippy renders again.

### Preferences not persisting across restarts (let alone reinstalls)

This was a real bug, not a regression — it has been broken since v0.1.x.

Your theme, model picks, hotkey rebinds, prompt customisations etc.
lived in an **in-memory Rust struct only**. The frontend kept a copy in
its own runes-state, mirrored to Rust on every change. Neither side
ever wrote to disk. tauri-plugin-store was loaded but unused.

Result: every app launch reset everything to defaults. The only things
that actually persisted were:
- API keys (Windows Credential Manager, separate mechanism — fine)
- History DB (SQLite, separate mechanism — fine)
- Skin choice (localStorage, fine)

**Now fixed.** v0.3.2 wires settings through `tauri-plugin-store` to
`user-prefs.json` in your app data folder. On launch we read from disk
first, fall through to Rust defaults if absent, and write back to disk
on every change. Survives reboots AND reinstalls (the installer doesn't
touch app data).

First launch on v0.3.2 will see your defaults (since there's no saved
file yet); every change after that persists.

### Missing capabilities

A few Tauri 2 permissions that were silently failing got added:
- `core:window:allow-unminimize` — was preventing the double-click-
  Clippy-to-open-main flow when the main window was minimised
- `store:allow-{load,get,set,save}` — needed for the new disk persistence
- `autostart:default + allow-{enable,disable,is-enabled}` — was
  silently no-oping the "Launch at login" toggle for some users

---

## 🐛 Known issue (deferred)

**Settings sub-menu buttons unclickable in true fullscreen mode.**
Reported but not reproduced cleanly yet — appears to be a Tauri-on-
Windows quirk with `decorations: true` + borderless-fullscreen, not
something the CSS can defensively fix. Workaround: use windowed /
maximised mode for now. Tracking for a future build.

---

## 🦊 + Hindi STT — research is in

If you dictate code-switched Hinglish ("भाई आज खाया की नहीं — poha
khaayoge ya aloo ka paratha?") Whisper does mediocre work. State-of-
the-art for Indic + code-switching as of May 2026 is **Sarvam Saaras
v3** (Indian startup, purpose-built, beats Whisper / Gemini / GPT-4o
on the public IndicVoices benchmark for Hindi). ₹1,100 free credits ≈
36 hours of audio on signup, no credit card.

Recommended path for v0.4.0: add Sarvam as a second STT provider
behind a toggle. Keep Groq Whisper for English-only (cheaper +
faster). ~4 hours of integration work. Not in this hotfix.

---

## ⬇ Get it

Quit any running wispr-fox, run `wispr-fox_0.3.2_x64-setup.exe` over
the v0.3.1 install. Settings + history persist (and starting now,
*actually* persist).
