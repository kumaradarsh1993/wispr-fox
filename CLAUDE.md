# wispr-fox — project memory for Claude Code

> Read on every new Claude Code session in this directory. Update only
> when a decision changes (architecture, conventions, ground rules).
> Day-to-day work goes in commits + release notes + chat, not here.

---

## What this is

Desktop dictation app for Windows (primary) + macOS (secondary,
unsigned). Tauri 2 + SvelteKit + Svelte 5 (runes) + Rust. Press a
hotkey, talk, get text — pasted into whatever app you're in.

Public repo: <https://github.com/kumaradarsh1993/wispr-fox>
Current stable: **v1.0.0** (commit `e89057c`, 2026-05-13). Nightly
channel is ahead at **v1.0.0-nightly.7** (commit `48b5f67`,
2026-05-27) — reliability + visibility fixes awaiting the user's
go-ahead to promote to a stable `v1.1.0`. Single owner, no paid users.

## Architecture (90-second tour)

```
src-tauri/src/
  audio/        cpal capture → hound WAV writer. Cold-start per F8 press.
  inject/       SendInput (Windows) / CGEvent (macOS) text injection.
                + focus.rs   capture/restore HWND across the LLM gap
                + clipboard.rs  fallback paste for long output
                + chunk.rs   WAV split when > 20 MB (Groq's 25 MB cap)
  stt/groq.rs   Whisper Large v3 Turbo (Groq). Multi-chunk if needed.
  llm/          Groq Llama (primary) + Gemini (secondary). Per-mode prompt.
  history/      SQLite. Three text columns: transcript, cleaned_text, drafted_text.
  flow.rs       Top-level state machine. Hotkey → record → STT → LLM → inject.
  hotkey.rs     8 registered combos: F8/F9/F10 + Win+ + Shift+F8 force-clean.
  settings.rs   AppSettings struct. Defaults here; user values in tauri-plugin-store.

src/routes/
  +layout.svelte   Sidebar (brand, hotkey hints, floater picker, usage, hero fox)
  /history/        Rows with Raw/Cleaned/Drafted tabs, kebab menu, search/filter
  /settings/       Provider keys, modes, hotkeys, behaviour, startup, look & feel
  /clippy/         Always-on-top floater. Skin variants: off/fox/stylized/beige/real-clippy
```

## Hotkey model (current, v1.0.0)

| Key | Mode | Behaviour |
|---|---|---|
| **F8** | Light (Transcribe) | Raw transcript OR LLM-cleaned (per `auto_clean_in_light` toggle) |
| **F9** | Drafting | Full LLM rewrite/format per the brief (was F10 in v0.1.x — F10 retired) |
| **Shift+F8** | Light w/ force-clean | One-shot cleanup override, ignores the toggle |
| **Win+F8** / **Win+F9** / **Shift+Win+F8** | Sticky variants | Press once start, press again stop |

**Why F10 was retired:** Windows reserves it as the system menu-
activation key. WM_SYSKEYUP leaks past RegisterHotKey and steals
focus to Outlook's ribbon. Settings store auto-migrates old F10
configs to F9 on first launch.

## STT + LLM providers

- **STT**: Groq Whisper Large v3 Turbo. Free tier: 2000 req/day,
  7200 audio-seconds/hour, 25 MB/file. Files > 20 MB auto-chunk to
  ~3.5 min slices, transcribe serially, concatenate.
- **LLM**: Groq Llama 3.3 70B Versatile by default. Gemini secondary.
  8-second timeout per call; falls back to raw transcript on timeout
  with a `clippy_timeout` note in the history row.
- **Next provider on deck**: Sarvam Saaras v3 for Hindi / Hinglish
  (SOTA on IndicVoices benchmark, ₹1100 free credits ≈ 36 hrs,
  no CC required). Integration sketched but not built — see
  `docs/RELEASE_NOTES_v1.0.0.md` § "still on the roadmap".

## v1.0.0 design system ("Foxy")

- **Palette**: cream `#faf6ec` surfaces, fox orange `#ec7c34` accent,
  warm dark brown `#2b2218` text. CSS vars in `src/app.css`.
- **Font**: Inter primary, system fallback.
- **Mascot**: watercolor fox PNGs at `static/fox/*.png` (19 assets,
  user-provided). Originals also kept in `Fox assets/` at repo root.
- **Floater skin "fox"** is default. State-mapped PNGs cross-fade:
  sitting (idle) → recording (listening, alert ears) → curious
  (thinking/writing, also on hover) → success (paste done).
- **Empty/loading states** use the watercolor fox illustrations.
- **History bottom** carries the `landscape-combined.png` autumn
  pastoral banner with a top-fade mask.

## Persistence model

- **Settings** → tauri-plugin-store @ `%APPDATA%/com.wispr-fox.app/user-prefs.json`
  (FIXED in v0.3.2 — was in-memory-only before that)
- **History DB** → SQLite @ `%APPDATA%/com.wispr-fox.app/history.sqlite`
  Schema: `recordings` table with `transcript`, `cleaned_text`,
  `drafted_text` columns. Idempotent ALTER on app start adds the
  drafted column for users upgrading from v0.1.x.
- **Audio files** → `%APPDATA%/com.wispr-fox.app/audio/YYYY-MM-DD/*.wav`
  Retention: 7 days, 500 MB cap. Sweeper runs hourly.
- **API keys** → Windows Credential Manager (keyring crate). Never
  stored in plaintext or sent anywhere except to the provider whose
  key it is.
- **Skin choice** → localStorage in the Clippy webview.

## Ground rules

These were settled mid-development; don't re-litigate without
explicit user permission:

1. **Nightlies auto-build on CI; stable needs a signal.** After a
   coherent set of changes, Claude may commit, tag `v*-nightly.N`,
   and push — GitHub Actions builds all-platform installers on the
   web. No need to ask first, and do NOT run `npm run tauri build`
   locally (the user runs a loaded machine; keep builds off it).
   Promotion to a **stable** release (marked Latest, no `-nightly`
   suffix) happens ONLY on an explicit *"promote to stable"* / *"ship
   it"* signal. (Decided 2026-05-27.)
2. **Batch fixes before shipping.** Single-bug commits are fine, but
   wait for explicit ship signal before pushing release. Multiple
   small fixes group into one version bump.
3. **Bug fixes get patch bumps; UX shifts get minor bumps; major
   identity / breaking-config changes get major bumps.** v1.0.0
   was the "looks like its name now" milestone — visual identity
   shipped + reliability proven.
4. **Production releases get an installer attached on the GitHub
   release page** (`wispr-fox_X.Y.Z_x64-setup.exe`). README's
   "Download" section auto-links to `releases/latest`.
5. **Release notes are user-friendly, not commit-style.** Group by
   what users will *notice* — not by file changed. Examples in
   `docs/RELEASE_NOTES_v*.md`.

## Known constraints / gotchas

- **Build profile**: `lto = "thin"`, `codegen-units = 16` in
  `[profile.release]`. Full LTO + codegen-units=1 OOM'd rustc on
  this 8 GB machine — see commit `1cb6724`. Don't tighten back to
  full LTO without confirming the host has 16+ GB RAM.
- **macOS DMG**: CI now builds `wispr-fox_1.0.0_aarch64.dmg` (Apple
  Silicon) on every release tag, alongside Windows NSIS `.exe` + MSI
  and Linux AppImage/deb/rpm. So every nightly ships all platforms in
  lockstep — no platform drifts behind. Unsigned: first macOS launch
  needs right-click → Open, or `xattr -dr com.apple.quarantine
  /Applications/wispr-fox.app`. No Intel (x86_64) Mac build yet.
- **Cubism SDK license**: free for indie under ¥10M/yr revenue.
  Document this in `LICENSE.md` if the Live2D-fox skin ever ships
  (see roadmap).
- **Electron same-window paste**: Slack/Discord/Teams have their
  own focus management that overrides Win32 `SetFocus` from outside.
  We don't fight it — clipboard fallback (default on) is the answer.
- **Live2D fox roadmap**: Live2D Sample models (Wankoromochi,
  Tororo/Hijiki, Hiyori, Mao) are the legally clean path under the
  Live2D Sample License (free + commercial OK under the revenue
  ceiling). Booth/itch "free" foxes have licence ambiguity — would
  need written commercial confirmation from author before shipping.

## Backlog (rough priority order)

1. **Sarvam Saaras v3 as second STT provider** — add behind a
   provider-toggle, default Groq for English, Sarvam for Hindi.
   Concrete recipe in `RELEASE_NOTES_v1.0.0.md`.
2. **Live2D fox skin** — derisk with the free Live2D Sample fox
   models; if the feel is right, commission a bespoke Live2D fox
   later. Integration plan in last session's chat (one new skin
   variant; existing skins untouched).
3. **App icon swap** — replace generic `icons/*.png` with the
   watercolor fox favicon. Needs `.ico` regeneration. The current
   icon is unchanged from v0.0.1.
4. **Macros & per-app tone v2** — extend the existing app-context
   classifier (Outlook → email register, WhatsApp → casual, etc.)
   with user-editable rules.
5. **Fullscreen-mode click bug** — Tauri-on-Windows quirk with
   `decorations: true` + true borderless fullscreen makes Settings
   sub-menu buttons unclickable. Reported, not reproducible cleanly,
   parked.
6. **macOS CI for DMG builds** — separate concern.

## Useful commands

```powershell
# Dev mode
npm run tauri dev

# Build (release exe + NSIS installer)
npm run tauri build -- --bundles nsis
# Output: src-tauri/target/release/bundle/nsis/wispr-fox_X.Y.Z_x64-setup.exe

# Frontend only
npm run build         # vite → build/

# Releases
gh release list --limit 5
gh release view vX.Y.Z
gh release create vX.Y.Z <installer-path> --title "..." --notes-file docs/RELEASE_NOTES_vX.Y.Z.md --latest
```

## Where things live on disk

```
~/.claude/projects/D--Claude-Code-Projects/   ← Claude Code session transcripts + memory
                                                (this dir; survives Claude Desktop wipes)
%APPDATA%/com.wispr-fox.app/                  ← wispr-fox runtime data
  audio/        recordings
  history.sqlite  history DB
  user-prefs.json settings (since v0.3.2)

D:\Claude Code Projects\wispr-fox\            ← source tree
  static/fox/   bundled watercolor assets
  Fox assets/   original asset library (kept for reference)
  docs/         release notes
  CLAUDE.md     this file
```

## How to resume after a /clear or session loss

1. `cd "D:\Claude Code Projects\wispr-fox"`
2. `claude --resume` (if you remember the session) OR fresh `claude`
   in the project dir (this CLAUDE.md is auto-loaded)
3. Say *"current state?"* — I'll run `git status`, `git log -5`,
   `gh release list --limit 3`, and summarise what's pending.
4. State your task. I'll pick up from this CLAUDE.md + the live git/
   release state.

---

*Last touched: v1.0.0 ship-day, by Claude Code session*
*`09de8006-a294-4d39-b5b9-aac2b760764a`. Update when conventions or*
*architecture change — not on every fix.*
