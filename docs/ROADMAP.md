# wispr-fox roadmap

Public plan for what's coming. Nothing here is a hard commitment — single-
maintainer project, scope can shift.

## 🟢 Next up

These are queued, scoped, and likely to land in the next 1–2 releases.

- **Sign in first, set up second** (requested 2026-09-02). A fresh install
  currently drops you straight into onboarding with no way to say "I already
  have wispr-fox". The first screen should ask instead: **New here** →
  the existing onboarding, or **Already using wispr-fox** → sign in (Google or
  email) → straight into the app with devices, history, usage and **API keys**
  already synced. The sync engine and the account system for this already
  exist — the missing piece is the fork at first launch, so a second machine
  never has to be configured by hand. This is the difference between "install
  and go" and "install and spend ten minutes pasting keys".
- **Tell people about the traps we already work around** (requested
  2026-09-02). Several known failure modes are handled silently in code, so a
  user hitting one has no idea what happened or that it is expected: macOS
  Accessibility dropping after an update (text goes to the clipboard instead of
  pasting), sync not connecting, the mic taking seconds to come up on Bluetooth
  and clipping the start of a sentence, a noisy room hurting the transcript,
  and how to change or hide the avatar. Wanted as **contextual hints shown near
  where the problem shows up**, surfacing once and then getting out of the way —
  NOT a help page nobody opens, and not another wall of banner text. The
  Accessibility banner rewritten in v3.4.0-nightly.11 is the shape to copy: one
  short line, one button, everything else behind a "Why?".

- **v2.1.0 stable** — promote the current nightly line (wave/siri minimal
  avatars, Codex pixel pets, auto-titled history, reimagined onboarding) once
  it's user-tested and gets a ship signal.
- **Pet importer + one original pet** — a Settings → Appearance "Import pet"
  button (Codex `pet.json` format, or fetch-from-CDN into appdata) so pets
  install without bundling third-party art, plus ONE original built-in pixel
  pet ("Foxel", orange fox, same 8×9 grid).
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

## Done — milestones

Recent stable milestones. The full per-release chronology (every nightly, with
user-facing notes) lives on the [GitHub Releases page](https://github.com/kumaradarsh1993/wispr-fox/releases)
and in `docs/RELEASE_NOTES_v*.md`.

- ✅ **v2.1.0 (nightly, in progress)** — wave + siri minimal avatars, Codex
  pixel pets, auto-titled history, and the reimagined "Pick your engine"
  onboarding (Deepgram recommended vs Groq). Not yet promoted to stable.
- ✅ **v2.0.0 (stable, 2026-06-30)** — provider expansion (OpenAI / Deepgram /
  ElevenLabs STT, OpenAI LLM), keyring-first key-storage hardening,
  Settings/sidebar cleanup, per-model usage tracking, high-fidelity raster
  avatars, analytics dashboard, and the constrained two-box floater.
- ✅ **v1.2.0 (stable, 2026-06-04)** — macOS launch fix, platform-aware
  ⌥-based hotkeys, Escape-to-stop, Retina position fix, Gemini refresh,
  check-for-updates.
- ✅ **v1.0.0 (stable, 2026-05-27)** — first "looks like its name" release:
  visual identity, watercolor fox mascot, history page with Raw/Cleaned/
  Drafted tabs.
