# wispr-fox — handover

> Single "where are we / how to resume" doc. Read this first, then
> `CLAUDE.md` for the deep architecture + conventions. Everything else
> is a specialist doc (see the map at the bottom).
>
> **Last updated: 2026-07-17** (v3.0.0-nightly.1 accounts + cross-device sync;
> synced-row history-rail fixes queued for the next nightly).

---

## Sibling apps (one product, three clients)

wispr-fox is three clients sharing one Supabase backend. Keep this block in
sync across all three handovers.

- **Desktop** — `./HANDOVER.md` (Tauri 2 + Rust + Svelte 5, Windows/macOS) ← you are here
- **Web** — `../wispr-fox-web/HANDOVER.md` (SvelteKit on Vercel)
- **Android** — `../wispr-fox-android/HANDOVER.md` (Kotlin + Jetpack Compose)

Shared backend spec: `../wispr-fox-web/docs/SYNC_DESIGN.md` · one-time
account/backend setup: `../wispr-fox-web/SETUP_ACCOUNTS.md` (secrets in the
gitignored `../wispr-fox-web/SECRETS.local.md`).

---

## What it is (one line)

Desktop dictation app for Windows (primary) + macOS (secondary, unsigned).
Press a hotkey, talk, get text pasted into whatever app you're in. Tauri 2 +
SvelteKit + Svelte 5 (runes) + Rust. Bring-your-own AI key, no subscription,
no account, no telemetry.

Public repo: <https://github.com/kumaradarsh1993/wispr-fox>

## Current state

- **Unreleased on `main` (rides the next nightly), 2026-07-17** — synced-row
  history fixes: the action rail stays aligned on rows synced from other
  devices even though they have no local audio to play (`880b828`), and the
  source chips moved next to the version tabs (`3b6b481`). Also on `main`:
  `22cd8f0` includes `user_id` in device/notes/settings writes (fixes the
  "Sync paused — will retry" banner caused by the NOT NULL + RLS check). Not
  yet tagged.
- **Delete policy — migrated to ownership-scoped + Purge (2026-07-17).**
  `delete_recordings(ids)` (`src-tauri/src/commands.rs`) dropped the What/Where
  matrix: a client may delete only the rows it originated (`remote == false`
  locally; `tombstone_remote` scopes the cloud PATCH to `device_id=eq.<this
  device>`). Transcript and audio die together; "delete all" hits only this
  device's rows; `HistoryRow` hides the delete control on remote rows. New
  `purge_account` command (Account panel, hold-to-arm + confirm) stamps
  `user_settings.purged_at` and hard-deletes every note, and `run_sync` applies
  a newer `purged_at` by wiping local history — the account-wide reset that also
  clears orphans. Full protocol in `../wispr-fox-web/docs/SYNC_DESIGN.md`
  ("Purge"). Also fixed a latent bug: `tombstone_remote` never bumped
  `updated_at`, so desktop deletes never reached other devices' pulls — it does
  now. All three clients now share this policy. **Runtime-unverified** (no live
  Supabase run on this box): the delete/purge HTTP paths are typechecked +
  `cargo check`-green, not observed.
- **Nightly: `v3.0.0-nightly.1`** (2026-07-16) — **accounts + cross-device
  sync** (major). Optional sign-in (Google via loopback-PKCE `127.0.0.1:43117`,
  or email/password) against a shared **Supabase** backend. Signed-in clients
  sync *transcripts* + API keys across desktop/web/mobile; audio never leaves
  the device. Reworked delete: press-and-hold → dialog (voice files /
  transcripts × this-device / everywhere, with cloud tombstones). Platform
  badges on rows. **Signed-out mode is byte-identical to before.** Rust
  `src-tauri/src/sync/` (config/auth/engine); history gains
  `platform`/`device_name`/`dirty`/`remote` cols + `sync_meta`/`sync_exclusions`;
  `delete_recordings` command. Backend/setup is shared across all three apps —
  see `../wispr-fox-web/docs/SYNC_DESIGN.md`, `SETUP_ACCOUNTS.md`, and the
  gitignored `../wispr-fox-web/SECRETS.local.md` (Supabase URL/keys, Google
  OAuth). The Supabase URL + anon key are baked into `src/sync/config.rs`.
- **Stable: `v2.1.0`** (2026-07-15, "Latest") — promoted from `nightly.10` on the
  user's "promote to stable" signal. The whole v2.1.0 line: pixel pets + wave/siri
  minimal skins, avatar-visibility tri-state, auto-titles, reimagined+rebuilt
  onboarding, per-recording flight recorder, mic-drop detection, mic wake-up
  health-check, and the sleep-blocker fix. (Superseded v2.0.0.)
- **Nightly: `v2.2.0-nightly.1`** (2026-07-15) — **audio file upload**. Drag an
  audio file onto History (or the Upload button → file picker) to transcribe
  voice memos / recordings, not just the mic. Per-batch provider/model + clean-up
  /draft choices in a modal; rows carry an "Uploaded" badge (new `source` column).
  Backend: `Flow::transcribe_file` + `run_upload_pipeline` (no injection, no
  mic-specific telemetry); STT providers now forward uploads with the correct
  MIME per extension (m4a/mp3/etc. — no local transcoding). `transcribe_upload`
  command. See `src/lib/UploadDialog.svelte`.
- **Sibling project: `../wispr-fox-web/`** — browser version (SvelteKit +
  Vercel serverless proxy). Separate repo, now **LIVE** at
  <https://wispr-fox-web.vercel.app> (sign-in replaced the old password gate;
  push to `main` auto-deploys). See its `HANDOVER.md` / `README.md` / `DEPLOY.md`.
- The v2.1.0 nightly cycle that led to stable (kept for history):

  | Nightly | What landed |
  |---|---|
  | .1 | UX-audit batch: Transcribe/Draft naming unified (Raw/Cleaned/Drafted are version tabs only), phantom-update Cargo.toml fix, keyring auto-migration, copy diet, wider sidebar, avatar-visibility tri-state (Always / While-dictating / Hidden), the **wave** skin (live-waveform pill) |
  | .2–.3 | Two rounds rebuilding the minimal avatars: **wave** bars now expand/contract individually with the voice (Wispr-Flow behavior), and the **siri** orb uses the real multi-conic-gradient SiriOrb technique |
  | .4 | Removed "duo"/"duo-hd" (bad design; migrate → oru-gujia); per-skin enter/exit animations (`data-arrive-skin` + `wispr:farewell` on Quit); wave polish |
  | .5 | **Codex pixel pets** as animated avatars (8 sprite-sheet pets in `static/pets/`, `lib/pets.ts` + `SpritePet.svelte`); sidebar picker = 6 featured + "More" tile; **auto-title** (parallel Groq `llama-3.1-8b-instant` names each recording, Settings→General toggle, default ON); history cards restyled (bolded title, right-aligned tabs, hover-glimmer expand chip) |
  | .6 | **Onboarding reimagined** — "Pick your engine": Deepgram (recommended, Nova-3, $200 free credit) vs Groq (free forever), smart 2-click deep-links + live key verify, optional Groq "brain" step on the Deepgram path; animated visual layer (gradient blobs/headline, staggered rise-in, typewriter), reduced-motion gated. README + docs de-Groq'd |
  | .7 | **Per-recording flight recorder** — the (i) panel shows real STT / cleanup / turnaround timings + an event log; pooled HTTP clients |
  | .8 | **Mic-drop detection** — captured-audio sample count vs wall-clock timer exposes a mic that died mid-recording; the card warns plainly instead of pasting a silent partial. (i) panel triangulates recorded vs captured vs provider-processed |
  | .9 | **Sleep-blocker fix** — the audio-cue worker held a rodio `OutputStream` open for the app's lifetime; Windows treats any live WASAPI render stream as "audio in use" and never sleeps. Now opened per cue, dropped after 30 s idle. **Onboarding rebuilt** (user spec): fluid full-bleed layout, zero scrolling at default 1200×800; welcome screen acts out a dictation with a wiggling hotkey cap + SpritePet buddy cycling through the roster + BYOK explainer; setup gates on explicit engine pick, then "I already have a key" vs "Help me get one" fork; cleanup brain is Gemini-recommended, auto-covered on the Groq path |

Every nightly builds all platforms (Win `.exe`/`.msi`, macOS `.dmg`, Linux
AppImage/deb/rpm) on CI from the tagged commit.

## Open threads / what's next

1. **macOS signing** — user adds 3 GitHub secrets + uncomments the env block in
   `.github/workflows/release.yml` per `docs/MACOS_SIGNING.md`. Fixes the
   Accessibility re-grant reset on every update AND the Mac auto-paste report.
   Long-standing, still pending.
2. **v2.1.0 stable — SHIPPED 2026-07-15.** (Done: tagged `v2.1.0` on the
   nightly.10 commit, CI published it as Latest.) The v2.2.0 line is now the
   active nightly cycle, opened by the audio-upload feature.
3. **Pet importer + one original pet** (from the pets work) — ship an
   importer (Settings → Appearance "Import pet", Codex `pet.json` format, or
   fetch-from-CDN into appdata) rather than bundling more third-party art, and
   commission ONE original pixel pet ("Foxel", orange fox, same 8×9 grid) as a
   built-in. Keep the Tauri-security baseline: explicit fs scope, no wildcard.
4. **Avatar plugin loader / SDK v2** — load user-authored avatars from
   `%APPDATA%\com.wispr-fox.app\avatars\` per `docs/AVATAR_SDK.md`. Parked.
5. **Sarvam Saaras v3** as a Hindi-friendly second STT provider — backlog.
6. **App icon swap** — still the generic v0.0.1 icon; needs `.ico` regen.

## Gotchas that have bitten us (don't repeat)

- **Never `npm run tauri build` locally** — 8 GB RAM, rustc OOMs on full LTO.
  Build profile stays `lto = "thin"`, `codegen-units = 16`. CI builds
  everything on tag push. `cargo check` locally is fine.
- **PowerShell 5.1 + `git commit -m "…double quotes…"` shreds the args** and
  the commit fails — but a following `git tag`/`git push` can half-succeed and
  leave the tag on the WRONG commit. Always commit multi-line messages via
  `git commit -F <file>` (write the message to a file first). Prefer the Bash
  tool for git here.
- **CI requires `docs/RELEASE_NOTES_<tag>.md` to exist at the tag** or the
  workflow hard-fails (it reads only that one file — it does NOT glob, so old
  notes can be pruned safely once their tag is built).
- **The browser preview harness reports the hidden tab as `visibilityState:
  "hidden"` (rAF never fires) and `0×0` viewport** — screenshots time out and
  layout is unmeasurable. Verify avatar/floater work structurally via
  `preview_eval` with synthetic timestamps, and show the user a live
  `show_widget` replica.
- **Floater resize is a native Rust command only** (`resize_floater`,
  bottom-center anchored). JS `setSize()`/`outerSize()` throw in that webview.
  On macOS the floater is opaque (transparent = zero-alpha ghost on Sequoia);
  positioning is in PHYSICAL px everywhere (Retina double-scaled it off-screen
  before the fix). See CLAUDE.md "macOS platform notes".

## Release discipline (settled — don't re-litigate)

- Nightlies auto-build on CI: commit + tag `v*-nightly.N` + push, no need to
  ask. **Stable promotion needs an explicit user signal** ("ship it" /
  "promote to stable") — never pre-emptively flip `--prerelease=false --latest`.
- Batch fixes into one version bump. Release notes are user-friendly prose,
  grouped by what users notice — not commit-style.
- Both Windows AND macOS artifacts come from the same tagged commit.

## Doc map (the minimal set)

| Doc | What it's for |
|---|---|
| `HANDOVER.md` | This file — current state + how to resume |
| `CLAUDE.md` | Deep architecture, conventions, ground rules, gotchas (auto-loaded each session) |
| `README.md` | Public GitHub front door (end-user) |
| `GETTING_STARTED.md` | Dev setup / build-from-source notes |
| `docs/ROADMAP.md` | Forward plan (Next / Likely / Maybe / Not happening) |
| `docs/AVATAR_SDK.md` | Avatar manifest contract + enter/exit hook |
| `docs/MACOS_SIGNING.md` | One-time macOS signing enablement steps |
| `docs/IMAGES.md` | README screenshot naming/sizing |
| `docs/RELEASE_NOTES_v*.md` | Per-release user-facing notes (CI reads these at tag time) |
| `static/pets/README.md` | Codex-pet attribution + fan-use disclaimer (legal — keep) |

## How to resume after a session loss

1. `cd "D:\Claude Code Projects\wispr-fox"`
2. Fresh `claude` in the project dir (CLAUDE.md auto-loads).
3. Ask *"current state?"* — run `git status`, `git log -5`,
   `gh release list --limit 3`, and reconcile against this file.
4. State the task and go.
