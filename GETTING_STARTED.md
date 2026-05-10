# wispr-fox — Getting Started (Claude Code handover)

> This file exists so a **fresh Claude Code session** can orient itself quickly.
> If you're a human reading this: the app is a WisperFlow-style dictation tool — hold a hotkey, speak, release, your words appear in the focused text field.

## What this is

A **push-to-talk dictation app** for Windows (Mac/Android later). Hold a global hotkey anywhere → record mic → release → transcribe via Groq Whisper → optionally clean up with an LLM ("Clippy") → paste into the focused text field via Win32 SendInput (or clipboard fallback).

Two modes: **Light** (punctuation/capitalisation fix only) and **Advanced** (treats dictation as instructions — "make this an email", "shorter", etc.).

**Working name only.** "Wispr-Fox" avoids WisprFlow's trademark. Pick a real name before any public release.

## Stack

- **Shell:** Tauri 2 (Rust) + SvelteKit (adapter-static) + Svelte 5 runes
- **CSS:** Tailwind v4 via `@tailwindcss/vite`
- **Audio:** `cpal` → 16 kHz mono i16 WAV via `hound`. Resample with `rubato` only if device refuses 16 kHz.
- **Text injection:** `windows-rs` `SendInput` (Unicode events, chunked for long text) → clipboard+Ctrl+V fallback via `arboard` (restores prior clipboard text+image)
- **STT:** Groq `whisper-large-v3-turbo` (free tier ≈ 2 hrs/day). 30s timeout, 1 retry on 5xx.
- **LLM cleanup:** Groq `Llama 3.1 8B` (Light, temp 0.2) / `Llama 3.3 70B` or Cerebras `Qwen3-235B` (Advanced, temp 0.4). 8s timeout → fall back to raw transcript on failure.
- **Secrets:** `keyring-rs` v3 → Windows Credential Manager. Two entries: `groq_stt_key`, `groq_llm_key` (split so different free-tier accounts can be used).
- **History:** `rusqlite` (bundled SQLite). Hourly GC by `retention_days` (default 7d) + 500 MB cap.
- **Hotkeys:** `tauri-plugin-global-shortcut`. Defaults: Light = `Ctrl+Win+Space`, Advanced = `Ctrl+Shift+Win+Space`.

## Rust module map

```
src-tauri/src/
├─ lib.rs               Tauri builder, plugin wiring, tray, single-instance
├─ commands.rs           Thin #[tauri::command] wrappers — no logic
├─ flow.rs               State machine: Idle→Recording→Transcribing→Cleaning→Injecting
├─ audio/mod.rs          cpal recorder + WAV streaming via hound
├─ audio/devices.rs      Input device enumeration for settings UI
├─ hotkey.rs             global-shortcut wrapper; emits {mode, edge} events
├─ inject/mod.rs         Dispatcher: try SendInput, fall back to clipboard
├─ inject/sendinput.rs   windows-rs SendInput w/ KEYEVENTF_UNICODE
├─ inject/clipboard.rs   Save prior clipboard → write text → Ctrl+V → restore
├─ stt/mod.rs            trait SttProvider { transcribe(wav, hint_lang) }
├─ stt/groq.rs           Multipart POST to whisper-large-v3-turbo
├─ llm/mod.rs            trait LlmProvider { complete(system, user) }
├─ llm/groq.rs           Chat-completions; model picked per Light/Advanced
├─ llm/prompts.rs        LIGHT_SYSTEM and ADVANCED_SYSTEM prompt constants (security boundary!)
├─ clippy.rs             Orchestrates Light vs Advanced; timeout → fall back to raw
├─ history/mod.rs        SQLite schema + CRUD + retention query
├─ secrets.rs            keyring wrapper; never logs values
├─ gc.rs                 tokio interval task: hourly purge by retention_days
├─ settings.rs           Typed wrapper over tauri-plugin-store
└─ tray.rs               Tray icon, menu, recording-pulse animation
```

## Frontend route map

```
src/routes/
├─ +layout.ts            SSR off, prerender on (adapter-static)
├─ +page.svelte          Main window: status pill, last transcript, nav
├─ history/+page.svelte  Table: time | duration | mode | transcript | actions
├─ settings/+page.svelte Tabs: API Keys, Hotkeys, Audio, Clippy, Retention, Startup
└─ onboarding/+page.svelte  3-step: paste Groq key → hotkey preview → mic test
```

Stores follow a rune-based singleton pattern: `settings-store.svelte.ts`, `history-store.svelte.ts`.

## Architectural decisions (locked — do NOT change without explicit user approval)

1. Two separate global hotkeys (Light vs Advanced) — not one hotkey with a modifier.
2. SendInput first, clipboard+Ctrl+V fallback (with prior-clipboard restore).
3. Clippy ships in v1 with both modes.
4. Light prompt wraps raw text in `<transcript>...</transcript>` tags with explicit prompt-injection defenses + 40% length-delta tripwire. **This is a security boundary.** See `llm/prompts.rs`.
5. All API keys in Windows Credential Manager via keyring-rs — never plaintext on disk.
6. History in SQLite (not JSON store). Purge by retention_days hourly.
7. Audio at `appData/audio/{YYYY-MM-DD}/{clip_id}.wav`. Clips < 300ms discarded.
8. Language hint = auto (do NOT pin to `en` — user mixes Hindi and Indian-accented English).

## Open questions (not blocking, but flag if they come up)

1. **Code signing** — ship unsigned for personal use. OV cert (~$200/yr) only if distributing.
2. **PTT vs toggle** — default is push-to-talk. If `tauri-plugin-global-shortcut` doesn't surface key-up reliably, fall back to toggle.
3. **Brand name** — "wispr-fox" is placeholder. Pick before any public release.
4. **Clipboard non-text** — fallback restores text + image but drops file/HTML formats. Confirm acceptable.
5. **History DB location** — currently `%AppData%\Roaming\com.wispr-fox.app\history.sqlite`. User may prefer a visible folder.

## How to run

```bash
# Prerequisites: Node.js 20+, Rust toolchain, MSVC C++ Build Tools
npm install
npm run tauri dev
```

## Approved plan file

The full implementation plan (200+ lines, includes smoke-test matrix) lives at:
`C:\Users\kadar\.claude\plans\so-i-am-looking-wise-stearns.md`

## Patterns from sibling project

This project mirrors `D:\Claude Code Projects\md-reader` conventions:
- Rune-based `$state` + class singleton stores
- `lib.rs` → `commands.rs` → domain modules pattern
- adapter-static + SSR-off routing
