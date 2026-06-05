# wispr-fox roadmap

Public plan for what's coming. Nothing here is a hard commitment — single-
maintainer project, scope can shift.

## 🟢 Next up

These are queued, scoped, and likely to land in the next 1–2 releases.

- **Time saved / words saved stats** — fun personal dashboard with date
  filters and a weekly digest. Captures total time spent, words dictated,
  estimated time saved vs typing. ~ETA next nightly.
- **Avatar plugin loader** — load user-authored avatars from
  `%APPDATA%\com.wispr-fox.app\avatars\` per the
  [Avatar SDK](AVATAR_SDK.md). Drop-folder install + Settings UI for
  install / uninstall / mute / unmute.

## 🟡 Likely

Wanted, partly-designed, no firm timeline.

- **Sarvam Saaras v3** as a second STT provider — SOTA on IndicVoices
  benchmark, much better than Whisper for Hindi / Hinglish. Add behind a
  provider toggle, default Groq for English, Sarvam for Hindi.
- **macOS text injection polishing** — current Mac build uses the
  clipboard fallback when the focused-app HWND-equivalent isn't available;
  the Windows path goes straight through SendInput. Wire up
  AXUIElement / NSWorkspace for a parity SendInput-style path.
- **Streaming partial transcripts** — once Groq exposes streaming Whisper.
  Would let the floater paint the running transcript as you talk.
- **Linux testing pass** — builds land on every release tag but nobody's
  daily-driving them. AppImage / deb / rpm all need someone to actually
  run them and report bugs.

## 🟣 Maybe

Ideas with real merit but big enough to need a focused effort.

- **Live2D fox skin** — proper animated mascot using the [free Live2D
  Sample models](https://www.live2d.com/en/learn/sample/). Risky on
  licensing if we ever charge.
- **Per-app tone v2** — extend the existing app-context classifier
  (Outlook → email register, WhatsApp → casual) with user-editable rules.
- **Custom OpenAI-compatible STT/LLM endpoints** — for self-hosted or
  Ollama users. Probably one "advanced" panel in Settings.

## 🔴 Not happening

Recurring asks where the answer is no. Documenting to save everyone
time.

- **Subscription** — wispr-fox is free, you pay your AI provider
  directly. There's no upsell coming.
- **Account signup** — there's no server, there's no us-with-a-server, so
  there's nothing to sign up to.
- **Telemetry** — including "anonymous usage data". No.
- **Cloud sync of recordings** — they stay on your machine. Backup is your
  responsibility (the audio + sqlite live in your AppData folder; copy it
  wherever you want).
- **iOS** — out of scope. The sibling
  [wispr-fox-android](https://github.com/kumaradarsh1993/wispr-fox-android)
  project is the mobile play and it's Android-only.
- **Corporate-proxy / alternate-STT routing** — investigated 2026-06-04
  after Zscaler blocked Groq on one user's company Mac. Took the
  IT-exception route instead. Single-track on Groq stays simpler.

## Done — recent

- ✅ **v1.3.0-nightly.1** — floater overhaul, part 1: per-skin window
  sizing re-derived from real art bounds (kills the right-side dead-zone
  and the clipped paperclip), one deliberate "box" with three size-states
  (active / idle / dormant) instead of per-frame resizing, a global
  floater **scale** control (sidebar S/M/L + a Settings slider, sticky),
  and a **dormant rest** state — the avatar shrinks and naps after a
  minute idle, waking the instant you use it or hover.
- ✅ **v1.2.0 (stable, 2026-06-04)** — macOS launch fix, platform-aware
  hotkeys, Escape-to-stop, Retina position fix, onboarding scroll/skip,
  Cat (lab) experimental skin, Gemini model refresh, check-for-updates.
- ✅ **v1.1.0-nightly.14** — per-skin floater window sizes that grow on
  bubble, polling clickthrough reverted.
- ✅ **v1.0.0 (2026-05-27)** — first "looks like its name" stable release.
  Visual identity, watercolor fox mascot, history page with Raw/Cleaned/
  Drafted tabs.

See [GitHub Releases](https://github.com/kumaradarsh1993/wispr-fox/releases)
for the full chronology.
