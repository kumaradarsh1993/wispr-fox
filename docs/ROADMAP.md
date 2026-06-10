# wispr-fox roadmap

Public plan for what's coming. Nothing here is a hard commitment — single-
maintainer project, scope can shift.

## 🟢 Next up

These are queued, scoped, and likely to land in the next 1–2 releases.

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

- ✅ **v1.4.0-nightly.4** — **floater corner-glitch properly masked + a
  safety toggle**. Root cause of the glitch that survived nightly.2: after a
  native resize the webview re-rasterizes asynchronously, so for a few frames
  the old content sits anchored to the window's moved top-left (avatar
  teleports up-left on grow, down-right on shrink, clipped — exactly as
  reported). No window flag can make that synchronous, so the avatar now
  hides for one painted frame before the resize and fades back (~150ms) after
  — a soft blink that blends into the avatar's own state cross-fade. Plus
  **Settings → Appearance → "Floater window"**: Compact (dynamic, default) vs
  Full box (classic v1.3 fixed size, zero transitions) — pick whichever reads
  better to your eye.
- ✅ **v1.4.0-nightly.3** — hand-drawn **pastel meadow SVG** for the bottom of
  History (rolling sage hills, white/orange/blush flowers, grass tufts, one
  butterfly — matches the design-playbook mock, ~5 KB, scales crisply at any
  width). Replaces the autumn PNG banner there; auto-dims to a faint
  silhouette in the dark themes.
- ✅ **v1.4.0-nightly.2** — **fixed the grow glitch** from nightly.1: pressing
  F8 made the floater window briefly paint a smeared/shifted frame while it
  expanded for the bubble. Windows was blitting the old window pixels into
  the new (taller) geometry before the webview repainted; `SWP_NOCOPYBITS`
  discards them so the worst case is one transparent frame hidden inside the
  avatar's own cross-fade. **History page restyled to the design playbook
  mock**: recordings are floating rounded cards on the cream canvas (gaps,
  not rule lines), the header sits directly on the surface instead of its own
  white block, date groups are label+count chips, and the watercolor meadow
  is now pinned to the bottom of the pane as an ambient strip the cards
  scroll over (was only visible at the very end of the list).
- ✅ **v1.4.0-nightly.1** — **floater no longer hogs screen space**: the
  window now sits in a tight box around the character and grows upward only
  while a speech bubble is actually showing, shrinking back right after. The
  old model permanently reserved the bubble's headroom — an invisible
  always-on-top band that covered chat messages and ate clicks above the
  avatar's head. UI polish batch: the stats strip on History now lives inside
  the page header (aligned to the same grid, no more edge-to-edge floating
  card), sidebar nav icons are crisp theme-aware SVGs instead of emoji, and
  the "Auto" theme's dark mode now matches the warm fireside dark palette
  (it was still the retired grey one). Bundle diet: dropped ~7 MB of unused
  landscape PNGs from the installer.
- ✅ **v1.3.0-nightly.11** — double-click the avatar opens the main window
  again (the move-drag was grabbing the OS move-loop on mousedown and eating
  the double-click; now dragging only starts once the pointer actually moves a
  few px, so a plain double-click gets through). Speech bubble box widened
  further per preference (roomier, fewer wraps).
- ✅ **v1.3.0-nightly.10** — **Analytics dashboard**: a new Stats page +
  at-a-glance widget on top of History showing lifetime time-saved (vs typing
  at 40 wpm), words & sessions per day, speaking speed, and a day-streak, with
  a 7/30/90-day bar chart. Totals are kept in a dedicated `daily_stats` table
  that survives the 7-day retention purge AND app updates. **macOS: durable
  Accessibility fix** — CI now signs every build with a stable (self-signed)
  identity so the Accessibility grant persists across updates instead of
  breaking on each one (one-time setup in `docs/MACOS_SIGNING.md`; no paid
  Apple cert needed). Floater: more gap between the speech bubble and the
  avatar's head, more top room + a wider bubble so long lines no longer clip,
  and the same dialog bubble now shows on the classic Clippy skin too. Sidebar:
  the S/M/L size buttons now stack vertically in the collapsed rail so they fit.
- ✅ **v1.3.0-nightly.9** — tightened every avatar's box (less L/R + bottom
  padding, much smaller overall — e.g. cat 198×252 → 174×190); the bubble
  now anchors just above each character's head and grows upward, so it hugs
  the head at rest instead of floating with a gap. macOS Accessibility
  banner now explains the real cause (unsigned-update invalidates the grant)
  + how to re-grant. (Durable fix = stable code-signing in CI — proposed.)
- ✅ **v1.3.0-nightly.8** — Mac hotkeys simplified to a single ⌥ chord
  (⌥Space dictate, ⌥Enter draft, ⌘ for sticky) instead of the ⌃⌥ three-key
  combos — works out of the box, no macOS "standard function keys" setting
  needed; existing Mac installs auto-migrate. "Clear all" in History is now
  press-and-hold-3s (no modal) and hard-deletes the .wav files on disk (the
  whole audio dir), not just the DB rows.
- ✅ **v1.3.0-nightly.7** — reverted the floater to stable's model: ONE fixed
  box per avatar, no resize on dictation state (bubble lives inside it), and
  S/M/L scale is the only thing that resizes the window (scaling everything
  in proportion). Killed the double-click-fills-screen bug for good by
  dropping `data-tauri-drag-region` (manual drag instead — no maximize path).
- ✅ **v1.3.0-nightly.6** — right-click menu no longer cropped: the menu
  renders inside the floater window and doesn't scale, so at Small/idle it
  was clipped to ~2 rows. Opening it now grows the window to a fixed
  menu-sized box (whatever the scale/state) and anchors the menu where it
  can't be clipped; it shrinks back on close.
- ✅ **v1.3.0-nightly.5** — floater fixes: (1) double-clicking the avatar no
  longer blows the transparent window up to fill the whole screen
  (`data-tauri-drag-region` was toggling maximize — now `maximizable:false`);
  (2) much smoother resize — the window is moved+resized in ONE atomic
  Win32 `SetWindowPos` instead of two paints, and the size-lock no longer
  causes an intermediate clamp; (3) the speech bubble scales with the floater
  scale (so Small no longer clips/overflows it), is wider (fewer line wraps),
  and anchors per-skin (the cat's bubble sat too high).
- ✅ **v1.3.0-nightly.4** — floater jump + bubble polish: bottom-centre
  anchor so the character holds its spot while the window grows upward for
  the bubble (no more downward jump on F8 / settle-back when done); the
  speech bubble now anchors above the head and grows upward with a
  guaranteed buffer, so long "marathon" text never covers the face; window
  sizes derived from each skin's art footprint (so S/M/L scales the box
  too); and the window is size-locked (min==max) so you can't accidentally
  drag-resize it.
- ✅ **v1.3.0-nightly.3** — the real fix: the floater webview's JS window
  API was broken (`outerSize()` rejected, so the resize aborted before
  `setSize` ever ran — the debug overlay showed `got 0×0`). Moved the whole
  resize into a native Rust command (`resize_floater`), centre-anchored,
  which returns the actual size back for the overlay. The box now resizes.
- ✅ **v1.3.0-nightly.2** — floater resize **actually works now**: the
  window was `resizable: false`, which makes programmatic `setSize` a
  silent no-op on Windows, so the box was frozen at 190×210 the whole
  time (hence "too big, never changes with S/M/L"). Made it resizable,
  tightened every skin's boxes, and added a **debug overlay** (Settings →
  Appearance → "Show floater debug overlay") that draws the window bounds
  + a live requested-vs-actual size readout for tuning.
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
