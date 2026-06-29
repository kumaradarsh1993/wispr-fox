# wispr-fox — Getting Started (Claude Code handover)

> Fresh Claude Code session: **read this whole file first**, then start work.
> If you're a human: this is a WisprFlow-style dictation tool — press F8 / F9 / F10, speak, your words appear in the focused text field.

## Current state (2026-05-11)

App is **feature-complete and daily-driven** on Windows. The user has
been using it as their primary input method for 10+ days, including
$1,500 of API token spend stress-testing the prompt + provider stack.

What works today, end-to-end:

- **3 hotkeys** with distinct behaviors:
  - F8 (Light) → raw transcript, no LLM cleanup by default
  - F9 (Advanced) → strict copy-edit (grammar/spelling only, preserves voice)
  - F10 (Drafting) → speak a brief, get a polished draft back
- **Push-to-talk + sticky-toggle**: Win+F8/F9/F10 are dedicated sticky-invoke
  hotkeys (press once start, press again stop). Per-mode sticky default
  toggle in Settings.
- **True cold-start audio** (~25-200ms setup_ms with Realtek enhancements off)
- **Two providers**: Groq (Whisper + Llama 3.3 70B) and Google Gemini
  (2.5 Pro for LLM). Per-mode provider+model can differ. Test-connection
  button validates saved keys without re-typing.
- **Real Microsoft Clippy** sprite via vendored+patched clippyts (the
  dynamic-import pattern doesn't survive Vite; we vendor the package
  and patch the agent loader to use a static map). Plus 3 alt skins:
  Off, generic Paperclip, "Chippy" potato-chip mascot.
- **Themes**: light, dark, retro (Windows 98 vibes). CSS custom
  properties + `[data-theme]` attribute.
- **History UI** with date dividers, inline 2-line preview + expand,
  variant pager (raw ↔ polished) as inline pill, retry button on
  failed rows, delete (removes audio file too).
- **Editable system prompts** per mode in Settings, with Reset
  button that restores the baked-in default fetched from Rust via
  `get_default_prompts`.
- **Onboarding** — 6-step Clippy-guided flow with skin picker.
  Forces light theme during onboarding for visual consistency.
- **Tray + minimize-to-tray** — closing the window hides instead of
  quitting. Real quit goes through tray menu.
- **Autostart** toggle (`tauri-plugin-autostart`).
- **Audio cues** — custom start/stop sounds, upload via file picker,
  on/off toggle.

Repo: `D:\Claude Code Projects\wispr-fox`, branch `main`. Cross-platform
release CI matrix in `.github/workflows/release.yml` (Windows + macOS
arm64 + macOS Intel + Linux). **Only Windows is exercised**; Mac/Linux
builds are wired but untested.

## What's NOT done

- **macOS text injection**: there's no `inject/macos.rs` yet. Mac build
  falls back to the clipboard+Cmd+V path. Works, but slower than the
  Windows `SendInput` flow. Estimated 1-2 days to write a CGEvent + AX
  implementation.
- **Streaming Whisper**: Groq doesn't expose it. Once they do, wire it.
- **Self-hosted custom-endpoint provider**: ~50 LOC to add a
  user-configurable OpenAI-compatible endpoint provider (for Ollama
  / vLLM / local Whisper.cpp servers). Not started.
- **Linux**: builds in CI, never run.
- **Android**: separate project at `D:\Claude Code Projects\wispr-fox-android\`
  (PRD in place, no code yet).

## What this is

A **push-to-talk dictation app** for Windows (Mac soon). Press a hotkey
anywhere → record mic → transcribe via Whisper → optionally clean up
with an LLM → paste into the focused text field via Win32 SendInput
(or clipboard fallback).

Three modes (in order of how aggressively the LLM touches your text):

- **Light (F8)** — raw transcript by default. LLM cleanup OFF by default.
  When LLM is enabled, prompt is strictly bounded: punctuation/capitalization
  only, transcript wrapped in `<transcript>...</transcript>` for
  prompt-injection defense.
- **Advanced (F9)** — strict copy-edit: grammar, spelling, sentence
  structure. **Never drafts, expands, or reduces.** Preserves the user's
  voice. (Rewritten from earlier draft-style prompt that was eating into
  F10's purpose.)
- **Drafting (F10)** — speak a brief like "reply to my boss about the
  meeting", get back a complete polished output. Prompt explicitly
  forbids asking clarifying questions back.

## Stack

- **Shell:** Tauri 2 (Rust) + SvelteKit (adapter-static) + Svelte 5 runes
- **CSS:** Plain CSS with custom properties for theming, no Tailwind
- **Audio capture:** `cpal` → 16 kHz mono i16 WAV via `hound`. **True
  cold-start** — fresh stream per F8 press (see Audio architecture).
- **Audio cues:** `rodio` plays 360 Hz sine "dop" at start + stop. User
  overrides via `%APPDATA%/com.wispr-fox.app/sounds/` files (upload
  through Settings UI).
- **Text injection:** `windows-rs` `SendInput` (Unicode events, chunked)
  → clipboard+Ctrl+V fallback via `arboard`.
- **STT:** Groq `whisper-large-v3-turbo`.
- **LLM cleanup:** Groq `llama-3.3-70b-versatile` OR Google Gemini
  2.5 Pro. Switchable globally per the `llm_provider` / `llm_model`
  setting (with legacy per-mode fields kept for back-compat).
- **Secrets:** `keyring-rs` v3 first. If a verified keyring write fails,
  Windows uses a DPAPI-encrypted fallback at
  `%APPDATA%/com.wispr-fox.app/.keys.enc.json`; legacy `.keys.json`
  files are migration-only. Keys exist per provider (`groq_*`,
  `openai_*`, `deepgram_stt`, `elevenlabs_stt`, `gemini_llm`).
- **History:** `rusqlite` (bundled SQLite). Hourly GC by `retention_days`
  (default 7d) + 500 MB cap. Audio file deleted when row deleted.
- **Hotkeys:** `tauri-plugin-global-shortcut`. 6 registrations: 3 main
  push-to-talk + 3 sticky-invoke (`Super+F8/F9/F10`).
- **Clippy sprite:** Vendored `clippyts` (`src/lib/clippyjs-vendor/`)
  with patched static agent map. The original library uses dynamic
  imports that Vite can't statically analyze.
- **Tray:** `tauri-plugin` tray icon, minimize-to-tray on close.
- **Autostart:** `tauri-plugin-autostart`.

## Audio architecture (READ before touching audio)

**Cold-start, fresh stream per F8 press.** No always-hot. Mic
indicator OFF between recordings.

Performance: **~25-200ms cold-start setup_ms** when user has disabled
"Allow exclusive control" + "Audio enhancements" in Windows mic
properties. Without those settings disabled, **Realtek HD driver
puts the mic in low-power mode after ~30s idle and wakeup takes
1-5 seconds**. This is a documented Windows driver bug. Onboarding
calls this out.

The flow:
1. F8 down → query `cpal::default_host().default_input_device().default_input_config()` (~15ms)
2. `build_input_stream(...)` with **explicit 10ms buffer** (`sample_rate / 100`)
   — NOT default which can be 100ms+ on Realtek
3. `stream.play()` → first audio callback fires within ~13ms on warm hardware
4. F8 up → drop the stream (mic indicator off, device released)

**Why cold-start works here:** Empirical testing confirmed `cpal` callback
fires within 13ms of `play()`. The slow 5-second behavior was the Realtek
driver power-down, not cpal/WASAPI.

**Don't try to pause/resume the cpal stream on Realtek** — `pause()`
actually stops the WASAPI engine, and `play()` warmup takes 3-5s.
Already tested, doesn't work. Drop and rebuild instead.

**Don't use cubeb.** Confirmed: cubeb shared mode = 90ms latency vs
WASAPI = 37ms. cpal-on-WASAPI is the right backend.

## Rust module map

```
src-tauri/src/
├─ lib.rs                Tauri builder, plugin wiring, tray, single-instance
├─ commands.rs            #[tauri::command] wrappers — no logic
├─ flow.rs                State machine: Idle→Recording→Transcribing→Cleaning→Injecting
│                        Also: custom_prompt_for() helper, sticky resolution
├─ audio/
│  ├─ mod.rs              cpal recorder + WAV streaming + silence trim
│  ├─ cues.rs             rodio start/stop tones (custom file override)
│  └─ devices.rs          Input device enumeration
├─ hotkey.rs              Registers 6 hotkeys; HotkeyEvent has sticky_invoke flag
├─ inject/
│  ├─ mod.rs              Dispatcher: try SendInput, fall back to clipboard
│  ├─ sendinput.rs        windows-rs SendInput w/ KEYEVENTF_UNICODE
│  └─ clipboard.rs        Save prior → write → Ctrl+V → restore
├─ stt/
│  ├─ mod.rs              trait SttProvider { transcribe(wav, hint_lang) }
│  └─ groq.rs             Multipart POST to whisper-large-v3-turbo
├─ llm/
│  ├─ mod.rs              trait LlmProvider; build_llm_provider(id, model)
│  ├─ groq.rs             Groq chat-completions
│  ├─ gemini.rs           Gemini generateContent
│  └─ prompts.rs          LIGHT_SYSTEM, ADVANCED_SYSTEM, DRAFTING_SYSTEM
│                        + DEFAULT_PROMPTS map exposed via get_default_prompts
├─ clippy.rs              clean() takes Option<system_override> for custom prompts
├─ history/mod.rs         SQLite schema + CRUD + retention query
├─ secrets.rs             keyring + JSON fallback; per-provider entries
├─ gc.rs                  Hourly retention sweep
├─ settings.rs            AppSettings struct + tauri-plugin-store wrapper
├─ usage.rs               Daily STT/LLM call counter (rolled over UTC)
└─ tray.rs                Tray icon, menu, recording-pulse animation
```

## Frontend route map

```
src/routes/
├─ +layout.svelte          App shell: sidebar nav, theme attribute setter
├─ +layout.ts              SSR off, prerender on
├─ +page.svelte            Home: status pill, last transcript, quick actions
├─ history/+page.svelte    History list with date dividers, variant pager, retry
├─ settings/+page.svelte   7 sections: Provider & Keys, Models, Hotkeys,
│                          Audio cues, Look & Feel, Retention, Compare providers
└─ onboarding/+page.svelte 6-step Clippy-guided flow

src/lib/
├─ api.ts                  Typed wrappers around Rust commands + event subscriptions
├─ settings-store.svelte.ts  Runes-backed AppSettings store
├─ HotkeyCapture.svelte    Modal-overlay key recorder (intercepts capture-phase)
├─ ClippyDialog.svelte     Reusable Clippy + speech bubble
├─ SkinIcon.svelte         Renders preview for each skin option
├─ ClippyFloater.svelte    Always-on-top window content with state→animation mapping
└─ clippyjs-vendor/        Patched clippyts (static agent map, manual positioning)
```

## Settings model

Important fields in `AppSettings` (Rust) / `AppSettings` (TS):

```
light_hotkey / advanced_hotkey / drafting_hotkey       # main push-to-talk
light_sticky_hotkey / advanced_sticky_hotkey / ...     # Super+F8/F9/F10 sticky-invoke
sticky_light / sticky_advanced / sticky_drafting       # per-mode sticky default
auto_clean_in_light                                    # default false (F8 raw)
auto_clean_in_advanced / auto_clean_in_drafting        # default true
stt_provider / stt_model                               # global STT
llm_provider / llm_model                               # global LLM
light_provider / advanced_provider / drafting_provider # legacy, unused by UI
custom_light_prompt / custom_advanced_prompt / custom_drafting_prompt
theme                                                   # light | dark | retro
start_sound / stop_sound / cues_enabled
retention_days / retention_max_mb
autostart
```

## Architectural decisions (locked — don't change without explicit approval)

1. Three global hotkeys (F8/F9/F10) + three sticky-invoke variants
   (`Super+F8/F9/F10`). User explicitly didn't want Ctrl+Win+Space-style
   combos (collide with Windows IME picker).
2. SendInput first, clipboard+Ctrl+V fallback (with prior-clipboard restore).
3. Light prompt wraps raw text in `<transcript>...</transcript>` with
   prompt-injection defenses + length-delta tripwire. **Security boundary.**
4. **F8 LLM cleanup OFF by default.** User explicitly didn't want F8
   touched by the LLM.
5. **F9 is strict copy-edit only.** Earlier draft-style F9 prompt was
   rewritten because it overlapped F10's drafting role.
6. Secrets: keyring primary, encrypted local fallback only after verified
   keyring failure. Separate entries per provider (`groq_stt`,
   `groq_llm`, `openai_stt`, `openai_llm`, `deepgram_stt`,
   `elevenlabs_stt`, `gemini_llm`).
7. History in SQLite. Hourly GC. Audio + DB row deleted together.
8. Audio at `appData/audio/{YYYY-MM-DD}/{clip_id}.wav`. Clips < 300ms
   discarded. Audio retained on transcription failure for retry.
9. Language hint = auto (do NOT pin to `en` — user code-switches
   Hindi + Indian-accented English).
10. **True cold-start mic** — fresh cpal stream per F8 press.
11. Onboarding tells user to disable Windows "Allow exclusive control"
    + "Audio enhancements" — otherwise Realtek users get 5s cold-start.
12. **Real Clippy sprite** via vendored clippyts. User explicitly
    rejected hand-built SVG alternatives. The vendoring + patch are
    intentional — don't try to switch to the npm package, dynamic
    imports break under Vite.
13. **Clippy animation min-dwell**: 1.4s minimum per state to avoid
    flicker. Single animation per (state, mode) — no random pool. User
    feedback: random cycling was disruptive while speaking.

## Lessons baked into the code

If you find yourself wanting to undo any of these, **stop and re-read
the rationale**:

- **Don't add streaming "live transcription" with partial results.** Groq
  doesn't support it. Faking it client-side adds complexity without
  helping the user — final-result latency is already ~0.5s for 10s clips.
- **Don't move LLM cleanup back onto F8 by default.** User burned through
  $150/day during the first week largely because F8 was calling the LLM
  on every press. Default-off is intentional.
- **Don't shrink the variant pager back into a 3-row block.** User
  explicitly requested the inline pill style — `‹ POLISHED ›` — to
  reclaim vertical space.
- **Don't switch HotkeyCapture back to inline (non-modal) recording.**
  Keys leak to other inputs. The modal overlay is the fix.
- **Don't add the `language` parameter to Whisper requests.** Auto-detect
  is the right default for code-switching users.
- **Don't try to share Clippy positioning logic with clippyts defaults.**
  clippyts positions at 80% of viewport which is offscreen in our small
  windows. We override after agent load.

## Open feature backlog

### Tier 1 — Mac

1. Write `inject/macos.rs` using `CGEventCreateKeyboardEvent` + AX
   focus query. Mirror the SendInput chunking pattern.
2. Test cold-start audio on macOS — `cpal` uses CoreAudio there, no
   Realtek issue, expect single-digit ms.
3. Code-sign + notarize for distribution.

### Tier 2 — Enterprise / self-hosted

4. Add `CustomOpenAi` provider variant: user-supplied base URL +
   model name + optional API key. Routes through the same
   `LlmProvider` trait. Useful for Ollama, vLLM, on-prem.
5. Optional: parallel `CustomWhisper` STT variant for whisper.cpp servers.

### Tier 3 — Quality of life

6. Streaming partial transcripts (gated on Groq supporting it).
7. Per-mode model override in UI (the legacy fields are still in
   settings.rs, just not wired).
8. Better error surfaces in History rows (currently just "error" — show
   the actual reason).
9. Linux smoke test pass.

### Tier 4 — Distribution

10. Code-sign Windows installer (currently SmartScreen warns).
11. Submit to winget / Homebrew.

## Known UX papercuts

- `start_recording failed: recording already in progress` on key-repeat
  is still logged as ERROR. Should be DEBUG.
- A few `unused_code` warnings in legacy per-mode-provider paths.
- macOS first-launch needs right-click → Open (unsigned).

## How to run

```bash
# Prerequisites: Node 20+, Rust 1.75+, MSVC C++ Build Tools (Windows)
npm install
npm run tauri dev          # development
npm run tauri build        # production
```

If you hit a `target/` cache error referencing `C:\Claude Code Projects\...`
after the C:→D: move:
```bash
cd src-tauri && cargo clean
```

## Sibling docs in this repo

- [README.md](./README.md) — end-user GitHub README. Keep this in sync
  with shipped features.
- [LINKEDIN_POST.md](./LINKEDIN_POST.md) — three launch-post variations.
- [skins/SPEC.md](./skins/SPEC.md) + [skins/BRIEF.md](./skins/BRIEF.md) —
  for commissioning Rive/Lottie animators.

## Sibling project

`D:\Claude Code Projects\wispr-fox-android\` — Android version, separate
codebase (Kotlin + Compose), no Clippy, overlay-bubble activation. PRD
in place, implementation not started. See its `DESKTOP_LINKAGE.md` for
which pieces are shared.

## Don't waste tokens

The user explicitly asked: stop using `Monitor` to stream tauri-dev logs
continuously. Read the log file when needed. Only spawn background tasks
when actually necessary.
