# wispr-fox — handover

> Single "where are we / how to resume" doc. Read this first, then
> `CLAUDE.md` for the deep architecture + conventions. Everything else
> is a specialist doc (see the map at the bottom).
>
> **Last updated: 2026-07-18** (v3.0.0 promoted to stable — accounts +
> cross-device sync, audio-file upload, ownership-scoped delete, and Purge).

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

- **Stable: `v3.0.0`** (2026-07-18, "Latest") — the major accounts + sync line,
  promoted to stable on the user's "bump to stable" signal (tagged `v3.0.0` on
  commit `be3e5a1`; CI published all-platform installers as Latest). Rolls up
  everything the v3.0.0 / v2.2.0 nightlies carried:
  - **Accounts + cross-device sync.** Optional sign-in (Google via loopback-PKCE
    `127.0.0.1:43117`, or email/password) against a shared **Supabase** backend.
    Signed-in clients sync *transcripts* + API keys across desktop/web/mobile;
    **audio never leaves the device**. **Signed-out mode is byte-identical to
    before.** Rust `src-tauri/src/sync/` (config/auth/engine); history gains
    `platform`/`device_name`/`dirty`/`remote`/`updated_at` cols + `sync_meta`/
    `sync_exclusions`. Supabase URL + anon key baked into `src/sync/config.rs`;
    no service-role key anywhere. `22cd8f0` includes `user_id` on every
    device/notes/settings write (fixes the "Sync paused — will retry" banner
    from the NOT NULL + RLS check).
  - **Audio file upload.** Drag an audio file onto History (or Upload → picker)
    to transcribe voice memos / call recordings, not just the mic. Per-batch
    provider/model + clean-up/draft in a modal; rows carry an "Uploaded" badge
    (`source` column). `Flow::transcribe_file` + `run_upload_pipeline`;
    `transcribe_upload` command; `src/lib/UploadDialog.svelte`. STT providers
    forward uploads with the correct MIME per extension (no local transcoding).
  - **Ownership-scoped delete.** `delete_recordings(ids)`
    (`src-tauri/src/commands.rs`) replaced the old What/Where matrix: a client
    may delete only the rows it originated (`remote == false` locally;
    `tombstone_remote` scopes the cloud PATCH to `device_id=eq.<this device>`).
    Transcript + audio die together; "delete all" hits only this device's rows;
    `HistoryRow` hides the delete control on remote rows. Fixed a latent bug:
    `tombstone_remote` never bumped `updated_at`, so desktop deletes never
    reached other devices' pulls — it does now.
  - **Purge** (`purge_account`, Account panel, hold-to-arm + confirm) stamps
    `user_settings.purged_at`, hard-deletes every note, and `run_sync` applies a
    newer `purged_at` by wiping local history — the account-wide reset that also
    clears orphans. Full protocol in `../wispr-fox-web/docs/SYNC_DESIGN.md`
    ("Purge"). Settings sections reordered to the canonical cross-client shape
    (Account near the bottom, Purge at its foot).
  - **All three clients share the delete/purge policy** (desktop v3.0.0, Android
    v2.1.0, web live). ⚠️ **Runtime-unverified when promoted** — the delete/
    purge/tombstone HTTP paths are typechecked + `cargo check`-green and CI-built,
    but were never exercised against a live Supabase account before the stable
    tag. Purge is irreversible cross-device data loss by design; a first
    deliberate live test is still owed. One sharp edge, intended per spec:
    signing an existing device with local history into an already-purged account
    wipes that device's local history to match the reset.
- **Prior stable: `v2.1.0`** (2026-07-15) — pixel pets + wave/siri minimal skins,
  avatar-visibility tri-state, auto-titles, reimagined+rebuilt onboarding,
  per-recording flight recorder, mic-drop detection, mic wake-up health-check,
  the sleep-blocker fix. (Superseded v2.0.0; now superseded by v3.0.0.)
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
2. **v3.0.0 stable — SHIPPED 2026-07-18.** (Done: tagged `v3.0.0` on `be3e5a1`,
   CI published all-platform installers as Latest.) First owed follow-up is a
   deliberate **live** test of the delete/purge round-trips (they went stable
   runtime-unverified) before relying on them.
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
