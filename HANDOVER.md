# wispr-fox — handover

> Single "where are we / how to resume" doc. Read this first, then
> `CLAUDE.md` for the deep architecture + conventions. Everything else
> is a specialist doc (see the map at the bottom).
>
> **Last updated: 2026-07-18** (v3.0.0 promoted to stable — accounts +
> cross-device sync, audio-file upload, ownership-scoped delete, and Purge;
> then `v3.1.0-nightly.1` = mic noise reduction, pushed from the cloud/second
> machine — see "Current state" and the multi-machine note directly below).

> **⚠ Multi-machine workflow note.** This repo is now worked from TWO places:
> this local core machine, AND a cloud Claude Code session on a second laptop
> that commits atomic upgrades straight to git. `v3.1.0-nightly.1`
> (`src-tauri/src/audio/denoise.rs` + the noise-reduction setting/UI) was
> authored on the cloud machine and lives on branch
> `claude/laptop-fan-noise-mic-w4k2ho`. **Before starting new work here, `git
> fetch` and reconcile** — that branch/tag may be ahead of your local tree.
> Coordination process for this split is written up at the bottom of CLAUDE.md
> ("Multi-machine / multi-agent workflow").

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

- **`v3.1.0-nightly.3`** (2026-07-28, this local core machine) — **the sign-in
  audit.** Six fixes to accounts/sync + one to the paste path, all reported from
  live use on Windows. Notes: `docs/RELEASE_NOTES_v3.1.0-nightly.3.md`.
  Typechecked (`npm run check` 0 errors, `cargo check` clean); **not run from a
  built binary** — this nightly exists to be tested.
  - **Sign-in didn't survive a restart. Root cause: the refresh token's keyring
    write was never verified.** `sync/auth.rs::save_refresh_token` trusted
    `set_password(..).is_ok()` and then *deleted the fallback file*. Windows
    Credential Manager can accept a write that never persists — the exact
    failure mode `secrets.rs` already guards against with a write+readback (see
    its `set()`). So the session looked healthy all session, and was gone on
    next launch. Now readback-verified; the fallback file is only removed once
    the readback matches.
  - **"Not signed in" on launch even when the session WAS restored.**
    `try_restore_session()` is spawned async and does a network token refresh;
    the webview always won the race to `auth_status()` and got `signed_in:
    false`, with no event to correct it. `AuthStatus` gains `restoring`, Rust
    emits **`wispr:auth_status`** after the restore settles and on every
    sign-in/sign-out, and `account-store` subscribes (from `+layout.svelte`, so
    the listener is live before the restore lands). The panel shows "Restoring
    your session…" instead of the sign-in form.
  - **Models stayed greyed out ("- add key") after signing in on a new device.**
    `sync_settings` wrote pulled keys via `secrets::set` and emitted nothing,
    while every secret-gated control reads `check_secrets` **once on mount**.
    New event **`wispr:secrets_changed`**; the sidebar, Settings → Providers,
    and onboarding all re-read on it.
  - **Concurrent token refresh could revoke the session.** Supabase rotates
    refresh tokens, and `ensure_access_token` had no lock — the 60s poll, a
    post-recording sync, `tombstone_remote` and "Sync now" could each spend the
    same token, which reads as token theft to GoTrue and kills the family.
    Serialized behind a tokio mutex with a re-check after acquiring.
  - **`spawn_background_poll` raced the launch restore.**
    `tokio::time::interval` fires its first tick immediately; the poll won (no
    network) and emitted `signed_out`. First tick is now burned.
  - **Random "Copied to clipboard" instead of pasting (~10%, office laptop).**
    `GetForegroundWindow` transiently returns NULL during any activation
    handoff, giving `current_pid == 0`; `flow.rs` read that as "user navigated
    away" and took the silent-clipboard path with the caret still in the box.
    A pid of 0 is the *absence* of evidence, not evidence of navigation — it
    now retries (`current_foreground_state_settled`, 3×25ms) and, if still
    unreadable, restores the captured window and pastes normally. Logged to the
    flight recorder so it's visible if it recurs.
  - **Transcripts pulled from other devices stopped accepting updates after a
    second sign-in — including tombstones.** Found while auditing the owed
    delete/purge debt, and the most serious of the batch.
    `History::mark_all_done_dirty` (run by `after_sign_in`) marked **every**
    done row dirty, remote rows included. `list_dirty` correctly refuses to push
    remote rows, so `mark_clean` never cleared them and they stayed `dirty = 1`
    permanently — and `upsert_remote` skips any locally-dirty row. From then on
    those rows ignored every cloud update forever: deleting such a transcript on
    the device that owned it never removed this device's copy. Fixed by scoping
    the mark to `remote = 0` (matching `list_dirty`), plus an idempotent repair
    in the schema init that clears `dirty` on remote rows so existing installs
    heal on first launch. **This is a plausible root cause for any "I deleted it
    there and it's still here" report.**
  - ⚠️ **Not addressed:** only API keys sync, not preferences. `SETTINGS_KEYS`
    in `sync/engine.rs` is five key rows; nothing else in `AppSettings` crosses
    devices. Web/Android share the same five names — extending this is a
    three-client change.
  - ⚠️ **The v3.0.0 live delete/purge test is STILL owed** — it needs a built
    binary against a live account and cannot be done from this machine (no local
    builds). The code audit above is not a substitute. Suggested run: two signed-in
    devices → delete a transcript on its owning device → confirm it disappears on
    the other; then Purge from one → confirm both wipe.
  - **Android carries the same `markAllDoneDirty` query mismatch**
    (`RecordingDao.kt:97` has no `remote = 0`, `listDirtyDone` does) but its
    `applyRemoteNote` gates on `updatedAt`, not `dirty` — so there the stuck flag
    is **inert, not corrupting**. Left alone deliberately (separate repo/session);
    worth hardening if anyone ever adds a dirty-guard to the Android pull.

- **`v3.1.0-nightly.2`** (2026-07-22, this local core machine) — **the Gemini
  fix**, plus re-run cleanup/draft and a title-model picker. Notes:
  `docs/RELEASE_NOTES_v3.1.0-nightly.2.md`. ⚠️ **Owed: one live test against a
  real Gemini key.** The fix typechecks and the diagnosis is solid, but it has
  not been run against Google's API from a built binary. Do that before this
  ports to web/Android or goes anywhere near stable.
  - **Why every Gemini model failed.** `clippy::clean` wrapped every LLM call
    in a flat 8s deadline sized for Groq's Llama (1-3s, no thinking phase).
    Gemini 2.5 and the whole Gemini 3 line have **thinking on by default**, and
    the silent reasoning phase alone outruns 8s — so the timeout fired
    mid-thought on *every* Gemini model and the user got the raw transcript
    back with a `clippy_timeout` note. **Model ids were never the problem**
    (they were re-verified against Google's live model list on 2026-07-22);
    the earlier theory that they were hallucinated is wrong.
  - Fix: `llm/gemini.rs` asks for the minimum thinking each family allows
    (`thinkingLevel` MINIMAL/LOW on 3.x, `thinkingBudget: 0` on 2.5 flash —
    the two fields are mutually exclusive and family-specific, so a 400 while
    sending one retries once without it). `LlmProvider::timeout_hint()` makes
    the deadline per-provider (Gemini 25s, everyone else the old 8s), and
    `clean_with_timeout` + `ON_DEMAND_TIMEOUT` (90s) covers History-initiated
    runs where nothing blocks a paste. Also: join all non-thought parts
    (`parts[0]` alone truncated long drafts), `maxOutputTokens` 2048 → 8192
    (thinking is charged against it), and report the real `finishReason`.
  - **Re-run cleanup / draft** in the HistoryRow kebab, labelled with the model
    they'll use. The backend `generate_alt_version` already regenerated against
    the current provider+model — the UI just never reached it once a version
    existed, so a bad result was unfixable without re-transcribing.
  - **Title picker.** `title_provider`/`title_model` settings (defaulting to
    the previously-hardcoded Groq `llama-3.1-8b-instant`) with a picker in
    Settings → Providers. The `auto_title` toggle **moved there from General**
    so the switch and its model can't drift apart.
  - **Free-tier note for model choice:** Groq caps *tokens*/day
    (`llama-3.3-70b-versatile` = 100k TPD, which is only ~50-200 cleanups),
    Gemini caps *requests*/day. For long dictations Gemini's shape is the
    better free tier. Groq `llama-3.1-8b-instant` (500k TPD / 14,400 RPD) is
    generous and stays the right default for titles.

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
- **In flight: `v3.1.0-nightly.1`** (2026-07-18, branch
  `claude/laptop-fan-noise-mic-w4k2ho`, authored on the cloud/second machine) —
  **mic noise reduction** for fan-heavy laptop built-in mics. Opt-in setting
  `noise_reduction` ("off" default | "on" | "aggressive") in Settings →
  Dictation. "on" = 4th-order Butterworth high-pass @ 90 Hz (kills fan rumble,
  can't touch speech); "aggressive" adds RNNoise via the pure-Rust
  `nnnoiseless` crate (measured SNR 21→42-57 dB on the user's real XPS-13
  samples). New `src-tauri/src/audio/denoise.rs`; wired into `flow.rs` between
  silence-trim and STT on a `spawn_blocking` thread (~130-600× realtime, no
  perceptible latency). Runs on a **temp side-file** the STT request reads —
  the raw WAV in History is never modified (a `TempFileGuard` deletes the copy
  on every pipeline exit), and any denoise error **fails open** to raw audio.
  New `wispr:state` "denoising" drives a "clearing noise…" floater bubble +
  Touch Bar label + a flight-recorder timeline mark with the exact ms. Verified
  `cargo check` + 3 denoise unit tests + `svelte-check` + vite build. Not yet
  merged to `main`; nightly tag pending push.
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
