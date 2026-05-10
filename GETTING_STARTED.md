# wispr-fox — Getting Started (Claude Code handover)

> Fresh Claude Code session: read this first, then start work.
> If you're a human: this is a WisperFlow-style dictation tool — hold F8, speak, release, your words appear in the focused text field.

## Current state (2026-05-10)

App is **working end-to-end** on Windows:
- F8 (Light) / F9 (Advanced) push-to-talk hotkeys → Groq Whisper → Clippy LLM cleanup → SendInput paste
- API key stored via Windows Credential Manager with file fallback
- Always-hot mic stream means F8 is effectively instant (~1ms gate-flip, audio callback fires 13ms after app startup)
- Onboarding flow works, settings/history routes scaffolded
- Not yet packaged — runs via `npm run tauri dev`

Repo at `D:\Claude Code Projects\wispr-fox`, branch `main`. Initial commit `47394c2`, latest `7ab4725`.

## What this is

A **push-to-talk dictation app** for Windows (Mac/Android later). Hold F8 anywhere → record mic → release → transcribe via Groq Whisper → optionally clean up with an LLM ("Clippy") → paste into the focused text field via Win32 SendInput (or clipboard fallback).

Two modes:
- **Light (F8)** — punctuation/capitalization fix only. Wraps raw text in `<transcript>...</transcript>` tags for prompt-injection defense. Length-delta tripwire if cleaned output diverges >40% from raw.
- **Advanced (F9)** — treats dictation as instructions ("make this an email", "shorter", etc.).

**Working name only.** "Wispr-Fox" avoids WisprFlow's trademark. Pick a real name before public release.

## Stack

- **Shell:** Tauri 2 (Rust) + SvelteKit (adapter-static) + Svelte 5 runes
- **CSS:** Tailwind v4 via `@tailwindcss/vite`
- **Audio:** `cpal` → 16 kHz mono i16 WAV via `hound`. **Always-hot stream** at app launch (see Audio architecture below).
- **Text injection:** `windows-rs` `SendInput` (Unicode events, chunked) → clipboard+Ctrl+V fallback via `arboard`
- **STT:** Groq `whisper-large-v3-turbo`. 30s timeout, 1 retry on 5xx.
- **LLM cleanup:** Groq `llama-3.3-70b-versatile` (both Light + Advanced). 8s timeout → fall back to raw on failure.
- **Secrets:** `keyring-rs` v3 (Windows Credential Manager) + JSON file fallback at `%APPDATA%/com.wispr-fox.app/.keys.json`. Keyring is best-effort; file fallback always works.
- **History:** `rusqlite` (bundled SQLite). Hourly GC by `retention_days` (default 7d) + 500 MB cap.
- **Hotkeys:** `tauri-plugin-global-shortcut`. Defaults F8/F9 (NOT Ctrl+Win+Space — those collide with Win IME picker and various background apps).

## Audio architecture (READ THIS before touching audio)

**The mic stream stays live continuously from app launch.** When not recording, samples are discarded in the cpal callback (no disk, no buffer, no copy — just `return`).

Why: WASAPI shared-mode `play()` returns instantly, but the first audio buffers can take 100ms-5s to arrive depending on driver (especially Realtek HD audio with effects). If we did cold-start every F8, users lose the first few seconds of speech, and Whisper hallucinates "thank you" / "gracias" on the near-silent buffer.

The always-hot approach:
1. App startup → `cpal::default_host().default_input_device().default_input_config()` (~15ms on Realtek)
2. `build_input_stream(...)` with **explicit 10ms buffer** (`sample_rate / 100`) — NOT the driver default which can be 100ms+
3. `stream.play()` → first callback fires within ~13ms
4. Recording = flipping a `Mutex<Option<WavWriter>>` (~1ms)

**Don't try to pause/resume the cpal stream on Realtek** — `pause()` actually stops the WASAPI engine, and `play()` re-warmup takes 3-5s. Already tested, doesn't work. Stay always-hot.

**Privacy/Teams concern:** Always-hot does NOT mean always recording. cpal's callback fires with samples → we check the writer gate → if `None`, return immediately. Samples never leave the callback's stack. Other apps can simultaneously read the mic in WASAPI shared mode. Only cost is the "mic in use" Windows indicator stays lit.

## Rust module map

```
src-tauri/src/
├─ lib.rs               Tauri builder, plugin wiring, tray, single-instance
├─ commands.rs           Thin #[tauri::command] wrappers — no logic
├─ flow.rs               State machine: Idle→Recording→Transcribing→Cleaning→Injecting
├─ audio/mod.rs          cpal recorder + WAV streaming + silence trim
│   audio/devices.rs     Input device enumeration for settings UI
├─ hotkey.rs             global-shortcut wrapper; emits {mode, edge: Down|Up} events
├─ inject/mod.rs         Dispatcher: try SendInput, fall back to clipboard
│   inject/sendinput.rs  windows-rs SendInput w/ KEYEVENTF_UNICODE
│   inject/clipboard.rs  Save prior clipboard → write text → Ctrl+V → restore
├─ stt/mod.rs            trait SttProvider { transcribe(wav, hint_lang) }
│   stt/groq.rs          Multipart POST to whisper-large-v3-turbo
├─ llm/mod.rs            trait LlmProvider { complete(system, user) }
│   llm/groq.rs          Chat-completions; model picked per Light/Advanced
│   llm/prompts.rs       LIGHT_SYSTEM and ADVANCED_SYSTEM constants (security boundary!)
├─ clippy.rs             Orchestrates Light vs Advanced; timeout → fall back to raw
├─ history/mod.rs        SQLite schema + CRUD + retention query
├─ secrets.rs            keyring + JSON file fallback wrapper
├─ gc.rs                 tokio interval task: hourly purge by retention_days
├─ settings.rs           Typed wrapper over tauri-plugin-store
└─ tray.rs               Tray icon, menu, recording-pulse animation
```

## Frontend route map

```
src/routes/
├─ +layout.svelte        Wraps everything, imports app.css
├─ +layout.ts            SSR off, prerender on (adapter-static)
├─ +page.svelte          Main window: status pill, last transcript, nav
├─ history/+page.svelte  Table: time | duration | mode | transcript | actions
├─ settings/+page.svelte Tabs: API Keys, Hotkeys, Audio, Clippy, Retention, Startup
└─ onboarding/+page.svelte  3-step: paste Groq key → hotkey preview → mic test
```

Stores follow a rune-based singleton pattern: `settings-store.svelte.ts`, `history-store.svelte.ts`.

## Open feature requests from user testing (PRIORITIZED)

User dictated these directly via the working app. Pick from this list when they say "let's continue" — these are NOT speculation.

### Priority 1 (UX blockers)
- **History page auto-scrolls to bottom — should stick to top.** User explicitly: "we are doing a last-and-first sorting mechanism." Reverse the order or scroll to top on mount.
- **Apple-like UI polish** — currently functional but utilitarian. User wants lighter, more refined. Reference Apple's design language. Already started: light color scheme forced, #fafafa background, antialiased fonts, system font stack.
- **Play button on each history row** — audio files exist at `%APPDATA%/com.wispr-fox.app/audio/{date}/{uuid}.wav`. Add a button that opens or plays inline. Lets user debug their own dictation issues without leaving the app.

### Priority 2 (new features)
- **F10 third hotkey: "Drafting mode"** — user gives context + brief + main text, LLM drafts a polished output. Different prompt from Advanced. Example: user dictates a rambling idea → app drafts a polished email.
- **Sticky/toggle mode via `Ctrl+F8`** — alternative to push-to-talk. Press once to start, press again to stop. For longer dictations where holding F8 is annoying.
- **Floating recording overlay with waveform** — small always-on-top pill showing live mic level + status. Muted/subtle aesthetic. Lets user see recording is active without staring at the main window.
- **Show system prompts in Settings** — display the Light/Advanced prompts to the user so they can see what's happening. Possibly let them tweak.
- **Debug mode** — some way to inspect what's happening internally. Could be a debug tab showing recent flow events, Whisper responses, timing, etc. Audio playback in history rows partially covers this.

### Priority 3 (later)
- **Better model selection** — user thinks Llama 3.3 70B might still be inadequate for Light. Worth A/B testing against Gemma 2 9B, Mixtral, or even a hosted Claude/GPT-4o-mini call. Settings UI for model selection is scaffolded but needs polish.

## Known UX papercuts (small fixes)

- `start_recording failed: recording already in progress` logged as ERROR on key-repeat — should be DEBUG. (Windows sends repeat key-down events while F8 held.)
- A few unused warnings: `DEFAULT_MODEL` in `stt/groq.rs`, `GroqStt::new`, unused `MissingKey` enum variants.
- `[404] GET /favicon.ico` — needs a favicon in `static/`.

## Architectural decisions (locked — do NOT change without explicit user approval)

1. Two separate global hotkeys (Light vs Advanced) — not one hotkey with a modifier. F10 for the new Drafting mode follows the same pattern.
2. SendInput first, clipboard+Ctrl+V fallback (with prior-clipboard restore).
3. Clippy ships in v1 with both modes.
4. Light prompt wraps raw text in `<transcript>...</transcript>` tags with prompt-injection defenses + 40% length-delta tripwire. **This is a security boundary.** See `llm/prompts.rs`.
5. Secrets: keyring primary, JSON file fallback. Both write paths active so reads never fail even with broken Credential Manager.
6. History in SQLite (not JSON store). Purge by retention_days hourly.
7. Audio at `appData/audio/{YYYY-MM-DD}/{clip_id}.wav`. Clips < 300ms discarded.
8. Language hint = auto (do NOT pin to `en` — user mixes Hindi and Indian-accented English).
9. **Always-hot mic stream** — see Audio architecture above. Do not change without strong evidence the user's hardware allows fast cold-start.

## How to run

```bash
# Prerequisites: Node.js 20+, Rust toolchain, MSVC C++ Build Tools
npm install
npm run tauri dev
```

If you hit a `target/` cache error referencing `C:\Claude Code Projects\...` after a project move:
```bash
cd src-tauri && cargo clean
```

## Approved plan file

The original implementation plan (200+ lines, includes smoke-test matrix) lives at:
`C:\Users\kadar\.claude\plans\so-i-am-looking-wise-stearns.md`

Note: many specifics in that plan have evolved during implementation (LLM model, audio approach, secrets storage). Treat as historical context, not current spec. This file is the current spec.

## Patterns from sibling project

This project mirrors `D:\Claude Code Projects\md-reader` conventions:
- Rune-based `$state` + class singleton stores
- `lib.rs` → `commands.rs` → domain modules pattern
- adapter-static + SSR-off routing
