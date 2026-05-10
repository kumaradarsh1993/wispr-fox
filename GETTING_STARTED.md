# wispr-fox — Getting Started (Claude Code handover)

> Fresh Claude Code session: **read this whole file first**, then start work.
> If you're a human: this is a WisperFlow-style dictation tool — hold F8, speak, release, your words appear in the focused text field.

## Current state (2026-05-11)

App is **working end-to-end** on Windows. Validated via live user testing:
- F8 (Light) / F9 (Advanced) push-to-talk hotkeys → Groq Whisper → Llama 3.3 70B cleanup → SendInput paste
- API keys saved via JSON file fallback (Windows Credential Manager bypassed due to user's setup)
- True cold-start audio capture: ~25-200ms `setup_ms`, captures from first syllable
- Single "dop" audio cue on press + release (rodio sine wave, custom files supported)
- Clippy paperclip icon throughout (taskbar, tray, installer)
- Window: 600×500 centered, light Apple-ish theme, no black-space-when-maximized issues
- Onboarding flow works, settings/history routes scaffolded but UI-thin

Repo at `D:\Claude Code Projects\wispr-fox`, branch `main`. ~10 commits, latest `e817214`.

**Not yet done:** the full UI rebuild. The user is feeding a brief into Google Stitch — when they hand back HTML/CSS/screenshots, port into the SvelteKit frontend. Brief details below.

## What this is

A **push-to-talk dictation app** for Windows (Mac/Android later). Hold F8 anywhere → record mic → release → transcribe via Groq Whisper → optionally clean up with an LLM ("Clippy") → paste into the focused text field via Win32 SendInput (or clipboard fallback).

Two modes (the user confirmed F9 and F10 are effectively merged for now — F9 already does instruction-handling):
- **Light (F8)** — punctuation/capitalization fix only. Wraps raw text in `<transcript>...</transcript>` tags for prompt-injection defense. Length-delta tripwire if cleaned output diverges >40% from raw.
- **Advanced (F9)** — treats your dictation as instructions ("make this an email", "draft a thank-you to my manager + invite to lunch").

**Working name only.** "Wispr-Fox" avoids WisprFlow's trademark. Pick a real name before public release.

## Stack

- **Shell:** Tauri 2 (Rust) + SvelteKit (adapter-static) + Svelte 5 runes
- **CSS:** Tailwind v4 via `@tailwindcss/vite`
- **Audio capture:** `cpal` → 16 kHz mono i16 WAV via `hound`. **True cold-start** — fresh stream per F8 press (see Audio architecture).
- **Audio cues:** `rodio` plays a generated 360 Hz sine "dop" at start + stop. User can override by dropping `start.{wav,mp3,ogg}` and `stop.{wav,mp3,ogg}` files at `%APPDATA%/com.wispr-fox.app/sounds/`.
- **Text injection:** `windows-rs` `SendInput` (Unicode events, chunked) → clipboard+Ctrl+V fallback via `arboard`. The "streaming" appearance the user sometimes sees is just the target app's input handling — we send all events back-to-back; some apps (web inputs, React) re-render per character, others batch. Not a bug.
- **STT:** Groq `whisper-large-v3-turbo`. 30s timeout, 1 retry on 5xx. **Free tier: 2,000 RPD**.
- **LLM cleanup:** Groq `llama-3.3-70b-versatile` (both Light + Advanced). 8s timeout → fall back to raw on failure. **Free tier: ~1,000 RPD**.
- **Secrets:** `keyring-rs` v3 (Windows Credential Manager) + JSON file fallback at `%APPDATA%/com.wispr-fox.app/.keys.json`. File fallback is always-write, so reads never fail even with broken Credential Manager.
- **History:** `rusqlite` (bundled SQLite). Hourly GC by `retention_days` (default 7d) + 500 MB cap.
- **Hotkeys:** `tauri-plugin-global-shortcut`. Defaults F8/F9 (NOT Ctrl+Win+Space — those collide with Win IME picker and various background apps).

## Audio architecture (READ before touching audio)

**Cold-start, fresh stream per F8 press.** No always-hot. Mic indicator OFF between recordings.

Performance: **~25-200ms cold-start setup_ms** when user has disabled "Allow exclusive control" + "Audio enhancements" in Windows mic properties. Without those settings disabled, **Realtek HD driver puts the mic in low-power mode after ~30s idle and wakeup takes 1-5 seconds**. This is a documented Windows driver bug (search: "Realtek audio 2 second delay after idle"). Tell users to disable both settings during onboarding.

The flow:
1. F8 down → query `cpal::default_host().default_input_device().default_input_config()` (~15ms)
2. `build_input_stream(...)` with **explicit 10ms buffer** (`sample_rate / 100`) — NOT default which can be 100ms+ on Realtek
3. `stream.play()` → first audio callback fires within ~13ms on warm hardware
4. F8 up → drop the stream (mic indicator off, device released)

**Why true cold-start works here:** Empirical testing confirmed `cpal` callback fires within 13ms of `play()`. The slow 5-second behavior was the Realtek driver power-down, not cpal/WASAPI. Once user disables enhancements, cold-start matches Chrome's perceived latency.

**Don't try to pause/resume the cpal stream on Realtek** — `pause()` actually stops the WASAPI engine, and `play()` warmup takes 3-5s. Already tested, doesn't work. Drop and rebuild instead.

**Don't use cubeb.** Confirmed via benchmark: cubeb shared mode = 90ms latency vs WASAPI = 37ms. cpal-on-WASAPI is the right backend.

## Rust module map

```
src-tauri/src/
├─ lib.rs                Tauri builder, plugin wiring, tray, single-instance
├─ commands.rs            Thin #[tauri::command] wrappers — no logic
├─ flow.rs                State machine: Idle→Recording→Transcribing→Cleaning→Injecting
├─ audio/
│  ├─ mod.rs              cpal recorder + WAV streaming + silence trim
│  ├─ cues.rs             rodio start/stop dop tones (custom file override supported)
│  └─ devices.rs          Input device enumeration for settings UI
├─ hotkey.rs              global-shortcut wrapper; emits {mode, edge: Down|Up} events
├─ inject/
│  ├─ mod.rs              Dispatcher: try SendInput, fall back to clipboard
│  ├─ sendinput.rs        windows-rs SendInput w/ KEYEVENTF_UNICODE
│  └─ clipboard.rs        Save prior clipboard → write text → Ctrl+V → restore
├─ stt/
│  ├─ mod.rs              trait SttProvider { transcribe(wav, hint_lang) }
│  └─ groq.rs             Multipart POST to whisper-large-v3-turbo
├─ llm/
│  ├─ mod.rs              trait LlmProvider { complete(system, user) }
│  ├─ groq.rs             Chat-completions; model picked per Light/Advanced
│  └─ prompts.rs          LIGHT_SYSTEM and ADVANCED_SYSTEM constants (security boundary!)
├─ clippy.rs              Orchestrates Light vs Advanced; timeout → fall back to raw
├─ history/mod.rs         SQLite schema + CRUD + retention query
├─ secrets.rs             keyring + JSON file fallback wrapper
├─ gc.rs                  tokio interval task: hourly purge by retention_days
├─ settings.rs            Typed wrapper over tauri-plugin-store
└─ tray.rs                Tray icon, menu, recording-pulse animation
```

## Frontend route map (current — to be largely replaced once Stitch design lands)

```
src/routes/
├─ +layout.svelte         Wraps everything, imports app.css
├─ +layout.ts             SSR off, prerender on (adapter-static)
├─ +page.svelte           Home: status pill, last transcript, nav
├─ history/+page.svelte   Stub — needs full rebuild per UI brief below
├─ settings/+page.svelte  Stub — needs full rebuild per UI brief below
└─ onboarding/+page.svelte  3-step: paste Groq key → hotkey preview → mic test
```

## UI rebuild — what the user wants (from feature dictation Q&A)

**The user is feeding the design brief below into Google Stitch.** When they share the Stitch export (HTML/CSS, screenshots, Figma link), port the design into Svelte. Drop location: `D:\Claude Code Projects\wispr-fox\stitch-export\` or `design-refs\`.

### Layout

- **Collapsible left sidebar nav**: Home | History | Settings
- **History is the DEFAULT view** when app opens (not Home — Home is light/optional)
- **Bottom-left of left sidebar**: persistent footer showing
  - Daily token usage (Whisper requests today, e.g. "142 / 2000")
  - Active model (e.g. "whisper-large-v3-turbo / llama-3.3-70b")
- Reduce wasted horizontal whitespace currently in the layout
- Apple-influenced visual feel — minimal, generous whitespace, system fonts, soft borders

### History row anatomy

Each conversation row shows:
- 2 lines of transcript by default, click to expand to full
- **Right-side action buttons**:
  - ▶ play audio (audio file path: `appData/audio/{date}/{uuid}.wav`)
  - 📋 copy text (the cleaned version by default)
  - 🗑 delete (deletes BOTH text row AND associated audio file)
- **If transcription failed**: replace 📋 copy with **🔄 retry** button (audio file is retained even on failure for retry)
- **Variant pager** for Advanced mode: ← / → arrows to flip between
  - **Base variant** = raw transcript
  - **Polished variant** = LLM-cleaned text
  - DB already stores both fields — just needs UI to switch between them

### Cache semantics

- `retention_days` (default 7d) applies to BOTH text and audio together
- Delete row in UI → also delete audio file from disk
- GC sweep is hourly, governed by `retention_days` and `retention_max_mb` (500 MB cap)

### Retry mechanism

New command needed: `retry_transcribe(record_id)` that re-uploads the saved WAV to Whisper. Useful for:
- 429 rate-limit failures
- Network blips
- Trying with a different model after the fact (Settings change → retry old failed ones)

### Floating overlay = ANIMATED CLIPPY CHARACTER

This is the centerpiece UX moment the user wants. A draggable, frameless, always-on-top window with **Clippy the paperclip** in it, doing retro Microsoft Office Assistant-style animations driven by app state:

- **Idle** — Clippy stands there, occasional eye-blink / look-around
- **Listening (F8 held)** — Clippy cups his hand to his ear, eyes attentive
- **Thinking** (transcribing) — Clippy with a thought bubble
- **Writing** (LLM cleaning) — Clippy scribbling on a notepad
- **Pasting** (injection) — Clippy "drops" the text, looks satisfied
- **Sleeping** / hidden if app dormant for X minutes

User can drag Clippy anywhere on the desktop; position remembered.

**Implementation paths (recommend Option A first):**

- **Option A — clippyjs**: There's an open-source library that ships **the original Microsoft Clippy with all his original animations** (Wave, Searching, Thinking, Writing, GetTechy, Congratulate, RestPose, Save, Hide, etc.). Repo: https://github.com/pi0/clippyjs (or successor `@clippy/web`). We integrate the library, map our state machine to `clippy.play("Searching")` etc. ~2 hours of work, looks 100% authentic. **Recommended starting point.**
- **Option B — Lottie**: After Effects → JSON animation. LottieFiles AI can generate from prompt; or commission on Fiverr. Richer modern aesthetic.
- **Option C — Rive**: State-machine-driven. Highest fidelity, requires animator (or learning Rive editor). Best long-term answer.

For the floating window itself: Tauri 2 supports `decorations: false` + `alwaysOnTop: true` + `transparent: true`. Drag region: `data-tauri-drag-region` HTML attribute. Position memory: localStorage or settings store. Show/hide via `WebviewWindow::show()/hide()`.

## Architectural decisions (locked — do NOT change without explicit user approval)

1. Two separate global hotkeys (Light vs Advanced) — currently F8/F9. F10 reserved for future "drafting" mode if separation is needed.
2. SendInput first, clipboard+Ctrl+V fallback (with prior-clipboard restore).
3. Light prompt wraps raw text in `<transcript>...</transcript>` tags with prompt-injection defenses + 40% length-delta tripwire. **This is a security boundary.** See `llm/prompts.rs`.
4. Secrets: keyring primary, JSON file fallback. Both write paths active so reads never fail even with broken Credential Manager.
5. History in SQLite (not JSON store). Purge by retention_days hourly.
6. Audio at `appData/audio/{YYYY-MM-DD}/{clip_id}.wav`. Clips < 300ms discarded. **Audio retained for retry even on failed transcription.**
7. Language hint = auto (do NOT pin to `en` — user mixes Hindi and Indian-accented English).
8. **True cold-start mic** — fresh cpal stream per F8 press. No always-hot, mic indicator off between recordings.
9. Onboarding MUST tell user to disable Windows "Allow exclusive control" + "Audio enhancements" — otherwise Realtek users get 5s cold-start.

## Open feature backlog (prioritized)

### Tier 1 — UI rebuild (next session, big chunk)
1. Wait for user's Stitch export → port into Svelte components
2. Implement the layout described above (left nav collapsible, history default, usage indicator bottom-left)
3. History rows with full action buttons + variant pager + retry
4. Add `retry_transcribe(record_id)` Rust command
5. Wire delete to remove WAV file too

### Tier 2 — animated Clippy
6. Integrate clippyjs (or Lottie/Rive depending on user preference) into a frameless always-on-top Tauri window
7. Wire state machine: Idle / Listening / Thinking / Writing / Pasting → animations
8. Drag-anywhere + remember position

### Tier 3 — power features
9. Win+F8 sticky toggle mode (press once to start, again to stop)
10. F10 distinct "drafting" mode (or settle that F9 is sufficient — user has been ambivalent)
11. Editable prompts UI in Settings (with security warning on Light prompt)
12. Daily usage tracker — count Whisper requests client-side per UTC day, persist to settings store
13. Better model selection — A/B test Whisper variants (large-v3 vs turbo vs distil) and Llama variants in Settings

### Tier 4 — cross-platform
14. macOS port: cpal works on CoreAudio without the Realtek issue. SendInput needs replacing with `CGEvent` + AX. Plan: implement `inject/macos.rs` parallel to `inject/sendinput.rs`. Trait already abstracted.
15. Android: completely separate — overlay-based input, share Rust STT/LLM/secrets crate via UniFFI or rewrite in Kotlin

## Known UX papercuts

- `start_recording failed: recording already in progress` logged as ERROR on key-repeat (Windows sends repeat key-down events while F8 held). Should be DEBUG.
- A few unused-code warnings: `DEFAULT_MODEL` in `stt/groq.rs`, `GroqStt::new`, `MissingKey` enum variants, `drain_errors`.
- `[404] GET /favicon.ico` — needs a favicon in `static/`.
- The Stitch export hasn't landed yet — expect the user to share it via `stitch-export/` folder or paths to screenshots in `design-refs/`.

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

Note: many specifics in that plan have evolved during implementation (LLM model, audio approach, secrets storage, audio cues, planned UI rebuild). Treat as historical context, not current spec. **This file is the current spec.**

## Patterns from sibling project

This project mirrors `D:\Claude Code Projects\md-reader` conventions:
- Rune-based `$state` + class singleton stores
- `lib.rs` → `commands.rs` → domain modules pattern
- adapter-static + SSR-off routing

## Don't waste tokens

The user explicitly asked to stop using `Monitor` to stream tauri-dev logs continuously — kill any active monitors and prefer `Read` on the log file when needed. Same for spawning new background tasks: only do so when actually necessary.
