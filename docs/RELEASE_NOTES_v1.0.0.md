# v1.0.0 — Hello, fox.

Eleven months of bug-fixing and feature-batching brought wispr-fox to
the point where it actually deserves a 1.0. The dictation pipeline is
solid, settings finally persist, recordings of any length transcribe
reliably, and — most visibly — the app now looks like its name.

This is the first build I'd put in front of a non-technical person
without flinching.

---

## 🦊 What changed visually

Every illustration is now a watercolor fox.

| Surface | Now |
|---|---|
| **Sidebar brand** | The bold flat fox face is the brand mark + sidebar-toggle, both at once |
| **Sidebar bottom** | A pastoral watercolor fox sitting in tall grass, eyes closed |
| **Empty history** | A friendly watercolor fox in plants with "No transcripts yet" |
| **Loading** | A playful walking fox with a gentle bob animation |
| **End of history list** | An autumn pastoral horizon fades in as a soft full-stop |
| **Floater (new "Fox" skin, now default)** | Five watercolor poses cross-fade based on state: sitting (idle), alert-eared (listening), curious head-tilt (thinking, also when you hover), eyes-closed smile (done). Each pose has its own subtle motion — breathing, perking, tilting, bouncing. |

The Paperclip, Cream, and real Microsoft Clippy skins are all still
there — switch in the sidebar Floater section. New installs default
to Fox.

---

## 🐛 What was fixed in this milestone

Three real bugs collapsed into the 1.0 cut:

1. **"Recording already ongoing" ghost toast.** A race condition in
   the hotkey dispatcher could surface this red toast mid-recording
   even though the audio was still recording fine — the Clippy ear
   would vanish, the icon would reset, but the pipeline would
   complete normally when you released the key. Fixed with a
   synchronous 150 ms debounce gate (catches auto-repeat + the
   spawned-task race) plus filtering the benign "already in
   progress" error out of the UI error channel entirely.

2. **"Took too long — try again" toast at the 1m 30s mark.** The
   frontend watchdog was arming on the `listening` state too, so
   any recording longer than 90 seconds would trip the alarm even
   though recording duration is user-controlled (a long monologue
   is normal, not a stuck pipeline). The watchdog now only arms for
   transient pipeline states (transcribing / cleaning / pasting).

3. **Settings didn't actually persist across launches.** They lived
   in an in-memory Rust mutex; tauri-plugin-store was loaded but
   never used. Every restart reset theme, model picks, hotkey
   rebinds, prompt customisations to defaults. Now wired through
   `tauri-plugin-store` to a `user-prefs.json` in the app data dir —
   survives restarts AND reinstalls.

(All three originally shipped as v0.3.x and v0.4.x hotfixes; they
land here as part of the 1.0 cut.)

---

## 🎯 What 1.0 means

Calling something "1.0" should mean something specific. For us:

- **Core flow is reliable.** F8 transcribe, F9 draft, Shift+F8
  one-shot cleanup all work. Long recordings auto-chunk. Network
  hiccups don't strand recordings — errors are surfaced clearly and
  the Retry button works.

- **Settings persist.** Whatever you configure stays configured
  across restarts and reinstalls.

- **Visual identity matches the brand.** wispr-FOX looks like
  wispr-fox.

- **Cross-app behaviour is sane.** Outlook, Slack, Teams, browsers
  — focus restore puts the cursor where you started dictating, or
  silent-delivers to the clipboard if you've navigated away.

- **Three text versions per recording.** Raw / Cleaned / Drafted,
  generate any on demand from the history page.

- **F10 is retired** by default. Windows reserves it as the menu-
  activation key — it broke Outlook in v0.x. F9 is now drafting.

---

## 🚧 What's deliberately still on the roadmap

- **Hindi / Hinglish quality.** Whisper's Indic accuracy is mediocre.
  Sarvam Saaras v3 (purpose-built for Indian languages) is on deck
  as a second STT provider, behind a toggle.
- **Live2D animated mascot.** The watercolor PNGs cross-fade nicely
  but they're still snapshots. The plan: layer a Live2D-rigged fox
  on top for genuinely smooth motion + audio-reactive mouth + eye
  tracking. Asset commissioned separately.
- **macOS DMG.** Windows-first; macOS path needs code-signing setup.

---

## ⬇ Install

Quit any running wispr-fox, run `wispr-fox_1.0.0_x64-setup.exe` below.
Settings + history + API keys all carry over from any earlier version.

If this is your first time, the onboarding takes about 2 minutes:
paste a [Groq API key](https://console.groq.com/keys) (free tier
covers daily-driving the app), test your mic, done. Press F8 anywhere
on your computer to dictate.

Issues or feedback → open a [GitHub issue](https://github.com/kumaradarsh1993/wispr-fox/issues).
