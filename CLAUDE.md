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
**Current stable: `v1.3.0`** (Latest, 2026-06-06) — user-confirmed working,
promoted from `v1.3.0-nightly.11`. Single owner, no paid users.

**What v1.3.0 shipped** (this is the live baseline; details below):
- **Analytics dashboard** — `/stats` page + a widget on top of History
  (time saved vs typing @40wpm, words/sessions per day, speaking speed, day
  streak, 7/30/90-day chart). Backed by a lifetime `daily_stats` SQLite table
  that is NOT pruned by retention.
- **Floater = ONE fixed box per avatar** (reverted from the dynamic-resize
  experiment). Box never resizes on dictation state; only the S/M/L scale (and
  the right-click menu) changes window size. Bubble anchored above the head,
  grows upward. Double-click avatar → opens main window. (Full model below.)
- **macOS hotkeys = ⌥-based** (⌥Space dictate / ⌥Enter draft / ⌘ for sticky)
  since nightly.8-v2, NOT the old ⌃⌥ chords. (Hotkey section below is updated.)
- **macOS durable Accessibility signing** — infrastructure ready but NOT yet
  enabled (the three signing env lines in `release.yml` are COMMENTED; CI fails
  the mac build if they're set without the secrets). One-time enablement steps
  in `docs/MACOS_SIGNING.md`. **Pending the user adding 3 GitHub secrets.**

Nightly channel reached `v1.3.0-nightly.11` before promotion. History of the
nightly.1→11 work is in `docs/ROADMAP.md` "Done — recent".

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
  history/      SQLite. recordings (transcript/cleaned_text/drafted_text) +
                daily_stats (lifetime analytics rollup, NOT pruned by GC).
  flow.rs       Top-level state machine. Hotkey → record → STT → LLM → inject.
                Tallies daily_stats once per completed recording (record_session).
  hotkey.rs     8 registered combos: F8/F9/F10 + Win+ + Shift+F8 force-clean.
  power.rs      Cross-platform resume detector (wall-clock gap) + JS ping state.
  touchbar.rs   macOS Touch Bar UI (character picker + mode buttons + timer).
  settings.rs   AppSettings struct. Defaults here; user values in tauri-plugin-store.

src/routes/
  +layout.svelte   Sidebar (brand, hotkey hints, floater picker+S/M/L, usage, fox)
  /history/        Rows w/ Raw/Cleaned/Drafted tabs + StatsWidget at the top
  /stats/          Analytics dashboard (lib/stats.ts derivations, stats-store)
  /settings/       Provider keys, modes, hotkeys, behaviour, startup, look & feel
  /clippy/         Always-on-top floater. Skins: off/fox/stylized/real-clippy/cat/cat-lab
                   Custom right-click menu (FloaterContextMenu) replaces webview default.

src/lib/
  stats.ts                analytics derivation (time-saved, streak, gap-fill)
  stats-store.svelte.ts   loads stats_summary, refreshes on flow idle
  StatsWidget.svelte      compact home-page strip → links to /stats
  floater-scale.svelte.ts S/M/L scale store + floater-debug toggle store
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

**macOS uses ⌥-based defaults** (current scheme since the nightly.8 "v2"
remap; the old ⌃⌥ three-key chords are gone per user request for a single
near-function-key binding): **⌥Space = dictate (Light)**, **⌥Enter = draft**,
**Shift+⌥Space = force-clean**, and **⌘+** the same combo for the sticky
variants (`Super+Alt+Space` etc.). Rationale: the Mac function row sends
media/volume by default, and there's no clean single-function-key (F5=Dictation,
F4=Spotlight, F1–F3=system), so ⌥Space is the closest "one press" that works
without the "use F1/F2 as standard function keys" setting. Defaults set
per-platform in `AppSettings::default` via `cfg!(target_os="macos")`. TWO
marker-gated migrations in `settings-store.svelte.ts`: `macHotkeyMigrated`
(F8/F9 → Mac) and `macHotkeyV2Migrated` (old ⌃⌥ chords → ⌥Space scheme). The UI
renders combos via `src/lib/hotkey-display.ts` `prettyHotkey()` (emits ⌥/⌘/⇧/⌃
symbols on Mac; Space/Enter pass through) — single source of truth, no hardcoded
key strings in user-facing copy.

**Escape stops a recording in flight** (since nightly.9, both platforms).
Registered dynamically via the global-shortcut plugin when
`start_recording_async` completes; unregistered the moment
`finish_recording_async` enters. The narrow window avoids stealing Escape
from focused apps the rest of the time.

**macOS launch fix** (nightly.9): switched to `.build()` + `.run(handler)`
to handle `RunEvent::Reopen` — Dock-icon clicks now re-show a hidden main
window (without this, clicking the Dock on a running wispr-fox where the
main window was previously hidden did nothing — reported on M4 Pro).
`open_silently` defaults to `false` on macOS so first launch surfaces the
main window automatically (Mac users don't have the menu-bar muscle memory
to find tray-only apps the way Windows users find the system tray).

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

## Floater model (current — v1.4.0 two-box model)

The floater (`/clippy`) went through a painful dynamic-resize experiment
(nightly.1→7), was reverted to ONE fixed box for v1.3.0, then — at the user's
explicit request post-v1.3.0 ("the transparent background obstructs my
messages") — moved to a **constrained two-box model** in v1.4.0. The v1.3.0
fixed box permanently reserved the ~110px bubble band above the head, an
invisible always-on-top dead zone that covered content and ate clicks. Rules:

- **TWO boxes per avatar: REST (tight around the character) and TALK (adds
  the bubble band above the head).** `boxFor(skin, talking)` from the per-skin
  `ART` table (`{w,h,head}`). The window grows upward when a bubble (state or
  toast) appears and shrinks back ~350ms after it hides (debounced `talking`
  state — fade-out finishes first; mid-pipeline state hops never resize).
  This is NOT the old per-state experiment: resize keys off bubble visibility
  only, and works because the resize is one atomic native SetWindowPos,
  bottom-centre anchored.
- **S/M/L scale (and the open right-click menu) also change window size.**
  Scale store: `lib/floater-scale.svelte.ts`; everything (window, avatar,
  bubble, text) multiplies by `--fscale` together.
- **Resize is a NATIVE Rust command** (`commands.rs::resize_floater`,
  bottom-center anchored). JS `outerSize()`/`setSize()` THROW in the floater
  webview — do NOT resize from JS. `clippy` window is `resizable:true,
  maximizable:false`.
- **No `data-tauri-drag-region`** (its built-in dbl-click-maximize blew the
  transparent window fullscreen). Drag is manual with a 4px movement threshold
  so a plain double-click still reaches `ondblclick` → opens the main window.
- **Bubble** anchors `HEAD_GAP` px above the head and grows upward; box width
  `BUBBLE_W`/bubble `max-width` tuned wide per user pref. Same bubble shows on
  ALL skins incl. real-clippy. Tuning knobs are all consts at the top of the
  `<script>` — change a number, not the structure.

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
6. **Avatar SDK is frozen at manifest v1** (`docs/AVATAR_SDK.md`). Any
   change to the avatar contract bumps `manifestVersion` to 2 with a
   migration guide. Until the loader/manager UI ships, built-in
   skins remain hardcoded in `src/routes/clippy/+page.svelte`.

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
- **macOS platform notes** (audited nightly.8):
  - **Floater is OPAQUE on macOS as of nightly.11** — `transparent + macOSPrivateApi`
    rendered as a zero-alpha ghost surface on macOS Sequoia / M4 Pro (the avatar
    SVG painted, but the WindowServer never composited the window). Tactical
    retreat: `tauri.macos.conf.json` overrides set `macOSPrivateApi: false` +
    clippy `transparent: false` + `shadow: true`. CSS in `/clippy` adds a
    Mac-detected `data-platform="macos"` attribute and paints the warm cream
    `--bg-card` background + 14px border-radius so the floater visually matches
    the rest of the app. Windows keeps the gorgeous transparent floater
    untouched — only Mac is affected by the override. Re-enable transparency
    once we land a proven Sequoia ghost-window workaround (candidates: explicit
    `setOpaque:NO` via objc2, `NSBackingStoreBuffered` reconfig, defer-window-
    creation pattern).
  - **Floater positioning is in PHYSICAL pixels everywhere** (since nightly.12).
    `availableMonitors()` / `primaryMonitor()` / `outerPosition()` all return
    PHYSICAL px. Earlier code passed those values into `setPosition(new
    LogicalPosition(...))`, which Tauri internally multiplied AGAIN by the
    scale factor. On a 2× Retina Mac that placed the window ~1700 px past
    the right edge of the screen — the avatar painted, the JS heartbeat fired,
    the user just couldn't see anything because it was off-monitor. Fixed in
    both Rust setup (positions BEFORE show() using `PhysicalPosition::new`)
    and in `src/routes/clippy/+page.svelte` + `FloaterContextMenu.svelte`
    (everything now uses `PhysicalPosition` consistently). DO NOT switch any
    of these back to `LogicalPosition` without re-deriving the scale-factor
    conversion — Tauri 2's coordinate API silently miscomputes the conversion
    when the source and sink disagree.
  - **Auto-paste requires Accessibility permission** (CGEvent inject +
    the Cmd+V fallback both need it). `accessibility_ok` command checks
    `AXIsProcessTrusted`; a dismissible layout banner deep-links to the
    Settings pane. Until granted, text lands on clipboard only.
  - **Mic permission** is declared in `src-tauri/Info.plist`
    (`NSMicrophoneUsageDescription`) — present and working.
  - **`force_repaint` size-nudge is Windows-only** (`#[cfg(windows)]`):
    WebView2 DComp surface-loss is a Windows thing; nudging a Mac
    transparent window just causes jitter.
  - **`inject/focus.rs` is stubbed on non-Windows**: no "· AppName"
    label, no focus-restore, no app-tone for drafting. Graceful
    degrade to plain inject. Future work: AXUIElement/NSWorkspace.
- **Live2D fox roadmap**: Live2D Sample models (Wankoromochi,
  Tororo/Hijiki, Hiyori, Mao) are the legally clean path under the
  Live2D Sample License (free + commercial OK under the revenue
  ceiling). Booth/itch "free" foxes have licence ambiguity — would
  need written commercial confirmation from author before shipping.

## Backlog (rough priority order)

> **NOT pursuing**: corporate-proxy / alternate-STT routing
> (`groq_base_url`, Replicate, DeepInfra, Cloudflare Worker, etc.).
> Investigated 2026-06-04 after Zscaler blocked `api.groq.com` on the
> user's company Mac. User went the IT-exception route instead; we stay
> single-track on Groq. Don't reopen without explicit user permission.

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

*Last touched: v1.3.0 stable ship-day (analytics dashboard, fixed-box floater,*
*macOS ⌥ hotkeys, durable-signing infra pending secrets), by Claude Code*
*session. Update when conventions or architecture change — not on every fix.*

## Open threads (post-v1.3.0, for the next session)

1. **Enable macOS signing** — user to add 3 GitHub secrets + uncomment the env
   block in `release.yml` per `docs/MACOS_SIGNING.md`. Until then mac builds are
   unsigned and the Accessibility grant resets each update (known/expected).
2. **macOS auto-paste** — was reported not working even with Accessibility
   granted; root cause is the unsigned-update grant reset (item 1 fixes it).
   Re-verify after signing is on. Mac inject code is `inject/macos.rs`
   (cfg-gated, can't cargo-check on the Windows dev box — change blind + lean on
   CI, or test on the user's Mac).
3. **Avatar plugin loader / SDK v2** — parked (ROADMAP "Next up").
4. **Touch Bar** — code exists (`touchbar.rs`); auto-detect + toggle polish low
   priority.
