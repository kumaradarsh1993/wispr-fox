# wispr-fox v3.5 — design revamp brief

> Status: **direction approved 2026-09-14 — all four §9 decisions taken as
> recommended** (overlay titlebar on macOS; tray left-click opens the popover;
> key-first onboarding with Groq preselected and the demo as climax; "Clear
> all" moves to Settings › App & data › Danger zone). Written from three
> line-referenced code surveys of the current UI (shell + History,
> Settings + Insights, onboarding). Ships as the `v3.5.0-nightly.*` line.
> The floater/avatar overlay is **out of scope** — it works.

## 1. One-line vision

**A field notebook for your voice.** Warm paper, a fox in the meadow — but the
chrome is quiet, the type is disciplined, and every screen has one job. Think
Apple Notes' restraint with Bear's warmth, not a dashboard.

What stays: the palette (paper cream `--bg-surface`, oat-linen sidebar,
fox-orange accent, warm brown text), Inter, the watercolor fox, the day-grouped
history. What changes: density, hierarchy, consistency, and where things live.

## 2. What the surveys found (the honest state)

**Shell.** Sidebar (272px) carries seven jobs: nav, hotkey card, "Quick
controls" (provider, polish toggle, mic `<select>`, avatar picker + On/Auto/Off),
usage bars with UTC countdown, "Replay onboarding", hero fox. Account is not in
the shell at all. Native titlebar; content sits directly under it on macOS.

**History card.** 9–11 controls on the collapsed surface (up to 15 with menu):
caret, (i) inspector, 3–4 version tabs, Play, Copy, Reading, ⋮. Two competing
"expand" affordances (caret vs (i)). A dead `{#if false}` menu block. An
`advanced` filter with no pill. Meta strip crams up to 8 items on one line with
`·` separators. Page-level delete is press-and-hold; row delete is a menu item.

**Visual system.** ~15 font sizes (8, 9, 9.5, 10, 10.5, 11, 12, 12.5, 13, 14,
15, 17, 19, 21 + clamp). Radius tokens exist but code hardcodes 6/8/9/10/14.
No spacing scale. ~15 one-off hex colours in the two history files;
`StatusBadge` uses Tailwind `bg-red-100` and ignores the theme entirely.

**Settings.** 57 controls: ~10 beginner-essential, ~20 intermediate, ~19
advanced, ~8 that should not be user-facing (debug overlay, key event log,
file paths, floater diagnostics, legacy hotkey, legacy prompt). Voice and AI
engines pages each mix 3–4 unrelated concerns. Defaults are never shown.
"Advanced" in the nav is actually the key-storage page.

**Insights.** Fine structurally; "Active days · since X" duplicates the header;
voice portrait is device-local while everything above it is fleet-merged, with
a caveat that's easy to miss.

**Onboarding.** 4 screens. "Try it now" is `disabled={!keySaved}` — the magic
moment is gated behind leaving the app to create a provider account. macOS
Accessibility is only mentioned on screen 3. A returning signed-in user on a
new device can be routed back into key-paste because the root redirect reads
secrets once before sync lands. Skip is free and leaves an empty app. The
welcome copy name-drops avatars that don't exist in `/pets/` (and two that
carry trademark risk).

**Tray.** Native 5-item menu. No popover window exists.

## 3. Foundations (Phase 0 — everything else builds on this)

- **Type scale, 6 steps + display:** `--fs-xs 11` (meta only) · `--fs-sm 12`
  · `--fs-base 13` · `--fs-md 15` · `--fs-lg 18` · `--fs-xl 24` ·
  `--fs-display clamp(24px, 3vw, 32px)`. Nothing below 11px. Line-height
  1.45 body / 1.2 headings. Tabular numerals for durations and stats.
- **Spacing scale:** `--sp-1 4` · `2 8` · `3 12` · `4 16` · `6 24` · `8 32` ·
  `12 48`. Cards pad `--sp-4`, sections gap `--sp-6`.
- **Radius, tokens only:** controls `--radius-sm 8`, inner cards `--radius-md
  12`, cards `--radius-lg 16`, pills `999`. Delete `--radius-xl`.
- **Semantic mode tokens:** `--mode-transcribe`, `--mode-draft`,
  `--mode-meeting`, `--mode-upload`, `--mode-error`, each with `-fade`. Replace
  every one-off hex. Rewrite `StatusBadge` on tokens so it themes.
- **Primitives** (`src/lib/ui/`): `Button` (primary/secondary/ghost/danger,
  sm/md), `Segmented`, `Card`, `Field` (label + help + control + shown
  default), `Disclosure` (the one "Advanced" pattern, replaces ad-hoc
  `<details>`), `Toast` (global, replaces per-page banners), `Kbd` (hotkey
  glyphs). Every page consumes these; no page hand-rolls a button again.
- **macOS window chrome:** `titleBarStyle: "Overlay"` + `hiddenTitle` so the
  sidebar runs under the traffic lights (Notes/Finder look). Windows keeps
  native decorations — a custom Windows titlebar is a maintenance sink.
  *(Decision 1.)*
- **Motion:** keep `--motion-fast/base` and the standard ease; one shared
  enter/exit for menus, sheets and toasts; card hover lifts 1px, nothing
  bounces.

## 4. Home (History) — Phase 1

**Sidebar → rail.** 220px expanded / 56px collapsed. Contents, top to bottom:
brand (fox + name, no version), nav **Home · Insights · Settings**, spacer,
**account chip** (avatar/initial + name or "Sign in"), with the update dot on
Settings. Everything else leaves the sidebar: quick controls move to the
menu-bar popover and the Home status strip; usage bars move to Insights;
hero fox and "Replay onboarding" go (replay lives in Settings › About).

**Home header, one row.** Page title "Home" *(or "Field notes")* · **Search**
(wide, `⌘K`/`Ctrl+K`, placeholder "Search your notes…") · **Upload** (secondary
button; drag-drop still works anywhere). "Clear all" leaves the header for
Settings › App & data › Danger zone. *(Decision 4.)*

**Status strip** (new, directly under the header, one quiet line): `Listening
with Nova-3 · Polishing with GPT-OSS 20B · Fox: On · ⌥Space to dictate`. Each
segment is a click-to-change popover picker. This is the advanced user's fast
path and replaces the sidebar quick controls without stealing vertical space.

**This-week strip** (new, optional, collapsible): three numbers — minutes
dictated · words · time saved vs typing — plus "See insights →". This is the
Insights de-emphasis: glanceable on Home, full page one click away.

**Filter chips** rename from modes to what the user thinks about: **All ·
Dictations · Drafts · Meetings · Uploads · Failed**. (Drops the unreachable
`advanced` filter.)

**Card, redesigned.** Collapsed = two lines:
```
Title of the note (auto-title, medium weight)                       4:32 pm
Best available text, two-line clamp…                                   ⧉ ▶ ⋮
```
- "Best available" = Drafted for drafts, Meeting notes for meetings, else
  Cleaned, else Raw. Version tabs move into the expanded state as a
  `Segmented`; collapsed shows one small mode chip only when it isn't a plain
  dictation (Draft / Meeting / Uploaded / Failed).
- Actions **reveal on hover/focus** (Copy, Play, ⋮) — keyboard reachable, no
  permanent button rail. Whole row toggles expand; no separate caret.
- Expanded = full text, `Segmented` for versions, action row, and one
  **Details** `Disclosure` that absorbs the (i) inspector (timings, providers,
  device, file, id) and the "Delete" ghost-danger button. Rerun / Name speakers
  stay in ⋮.
- Remove: dead `{#if false}` block, retry-count badge (into Details), dot
  separators, the device chip on collapsed (into Details unless multi-device
  and different from this one).
- Day groups keep sticky headers; count chip goes.

## 5. Settings — Phase 2

**Two tiers, one pattern.** Every page shows its Essentials open and its
Advanced inside a single `Disclosure` at the bottom ("Advanced ▸"). Beginners
never see a textarea of prompt text; advanced users know exactly where to look.

**Nav (renamed, reordered):** Microphone & shortcuts · Transcription & AI ·
Writing style · Avatar & look · App & data · Account · About. "Advanced"
(key storage/audit) is folded into App & data › Advanced › Keys & security.

Essentials per page (everything else → Advanced on that page):
- **Microphone & shortcuts:** input device + Test · Transcribe / Draft /
  Show-hide hotkeys · audio cues. *Advanced:* force-clean hotkey, clipboard
  backup, pull-focus, adapt-tone, noise reduction, quiet-boost, mic-latency
  guidance. Legacy "Advanced cleanup" hotkey hidden unless bound.
- **Transcription & AI:** ONE "Who transcribes you" picker (service; model
  behind a "Change model" link) · spoken language · "Tidy up my words" toggle
  · API keys for the chosen services only. *Advanced:* cleanup/draft/title
  service+model pickers, every key card, connection tests.
- **Writing style:** clean-transcribe toggle · tone presets. *Advanced:*
  prompt editors, each behind "Customise prompt" with the injection warning
  shown *before* the textarea. Legacy prompt removed.
- **Avatar & look:** skin grid · visibility · theme. *Advanced:* size, window
  box. Debug overlay removed from user surfaces.
- **App & data:** launch at login · retention. *Advanced:* open silently,
  storage cap, cue-sound upload + folder, Keys & security, **Danger zone**
  (Clear all recordings, Purge account).
- **Account:** unchanged content, new primitives.
- **About:** version + update panel · links. *Advanced:* "Troubleshooting"
  with diagnostics + copy button, Replay onboarding.

**Every control shows its default** (`Field` renders "Default: On") and is
written in plain language (copy table: "LLM cleanup" → "Tidy up my words",
"STT service" → "Transcription service", "Language hint" → "Spoken language",
"Aggressive (RNNoise)" → "Strong — removes steady background noise",
"Keyring verified" → "Stored securely by your OS", etc.).

## 6. Insights — Phase 2b (light touch)

Keep the page. Add a **Highlights** row at the top (best day, longest streak,
favourite mode, longest dictation) — the "highlights that come out decently".
Remove the duplicated "since" line. Give the voice portrait a persistent
"this device only" badge in its header. Move the sidebar usage bars here as a
small "Today's quota" card. Feed the Home this-week strip from the same store.

## 7. Onboarding — Phase 3

A zero-key trial would need a backend paying for inference; rejected for now
*(Decision 3)*. Instead: make key-first **fast and honest**, and put the
try-it moment at the climax.

**Fresh user (5 screens, ~2 min if they have a key):**
1. **Welcome** — one sentence + the live hero loop. Job: set the expectation.
2. **Permissions** — microphone + (macOS) Accessibility, inline, with the
   "why" in one line. Job: make dictation actually land, before the demo.
3. **Choose who transcribes you** — outcome-framed cards, **Groq preselected**
   ("free forever, one key"), Deepgram as "best quality, $200 credit". Key
   paste + verify inline with the 3-minute help path. Job: make it permanent.
4. **Try it** — focused box; Finish lights up when one dictation lands
   (soft gate; Skip still exists but routes to a "Finish setup" banner on
   Home, not a dead app). Job: the magic moment.
5. **Sync (optional)** — sign in or continue. Job: cross-device upsell.

**Returning user, new device:** root shows a 1–2 s "Setting up…" splash that
awaits the first secrets/sync event; keys present → straight to Home with a
"Welcome back — ⌥Space anywhere" toast; else → screen 3.

Copy fixes: replace "engine/BYOK/cleanup brain" with outcomes; list only
shipped avatars; drop the trademarked names from the roster line.

## 8. Menu-bar popover — Phase 4 (new surface)

New window `popover` (route `/popover`, 340×460, borderless, transparent,
non-activating NSPanel on macOS, positioned under the tray icon; on Windows
anchored above the tray). **Left-click on the tray icon opens it** instead of
toggling the main window *(Decision 2)*; right-click keeps the native menu.

```
🦊  Ready · ⌥Space to dictate                      [Fox: On ▾]
─────────────────────────────────────────────────
Recent
  Meeting with design team              2:10 pm   ⧉
  Reply to Priya about the launch      11:48 am   ⧉
  Grocery list                           9:02 am   ⧉
  Draft: quarterly update              Yesterday   ⧉
─────────────────────────────────────────────────
Transcribe with  [Nova-3 ▾]     Polish  [On]
─────────────────────────────────────────────────
Open wispr-fox        Settings                Quit
```
Copy is one click; rows open the note in the main window. State line reflects
the recording FSM live (Listening… / Transcribing… / Ready).

## 9. Decisions needed

1. **macOS overlay titlebar** (sidebar under traffic lights). Recommend **yes**.
2. **Tray left-click → popover** (main window via popover "Open" or the
   show/hide hotkey). Recommend **yes**.
3. **No zero-key trial** in onboarding; key-first but Groq preselected and the
   demo as climax. Recommend **yes** (no backend cost, honest).
4. **"Clear all" leaves the Home header** for Settings › App & data › Danger
   zone. Recommend **yes**.

## 10. Build order → nightlies

| Nightly | Scope | Also carries |
|---|---|---|
| 3.5.0-n.1 | Phase 0 foundations (tokens, primitives, StatusBadge, overlay titlebar) | Groq cleanup fix, model catalog refresh (OpenAI ids were never valid) |
| n.2 | Phase 1 Home: rail, header, status strip, card redesign, filters | dead-UI removal |
| n.3 | Phase 2 Settings two-tier + copy; 2b Insights highlights | |
| n.4 | Phase 3 onboarding + sync-race fix | |
| n.5 | Phase 4 popover window | |
| **3.5.0** | stable on the user's signal after a week of daily use | |

Each nightly is independently usable; nothing is half-migrated at a tag.
