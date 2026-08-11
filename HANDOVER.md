# wispr-fox — handover

> Single "where are we / how to resume" doc. Read this first, then
> `CLAUDE.md` for the deep architecture + conventions. Everything else
> is a specialist doc (see the map at the bottom).
>
> **Last updated: 2026-08-11** (`v3.1.0` is stable; the Codex-authored
> visual-overhaul release is `v3.2.0-nightly.2`).

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

- **Unreleased `main` (2026-08-11) - meeting workflow revamp.** Diarized
  uploads are first-class meeting rows with a separate `meeting_notes_text`
  version, versioned `speaker_turns`, dynamic `speaker_names`, `is_meeting`,
  and `diarization_enabled` metadata. History now has a consolidated Rerun
  dialog, editable speaker labels, speaker-block rendering, named copy, and a
  full-screen reading mode. Cleanup and Draft/Meeting use separate model
  defaults, and the Meeting Notes prompt is configurable under Writing.
  Selecting diarization automatically moves an incompatible Whisper choice to
  a supported engine and explains why. OpenAI's specialized
  `gpt-4o-transcribe-diarize` path is supported; ordinary OpenAI STT defaults
  to `gpt-transcribe`. Groq defaults have moved from retiring Llama ids to
  GPT-OSS 20B/120B, and Gemini defaults to stable 3.6 Flash.
  - Local SQLite migrations are additive and idempotent.
  - The shared Supabase `notes` table must expose the five fields above before
    this desktop commit is deployed; the web-client change owns that shared
    migration and uses the same names.
  - Verification: `npm run check` is 0 errors/0 warnings, `npm run build`
    succeeds, and `CARGO_BUILD_JOBS=2 cargo check` succeeds with the same nine
    pre-existing dead-code warnings. No installer/tag was produced.

- **Stable: `v3.1.0`** (2026-08-01, "Latest") — promotes the complete
  v3.1.0 nightly line: mic noise reduction, Gemini reliability and re-run
  tools, account/sync hardening, paste reliability, external-mic selection and
  metering, quiet-audio rescue, wide-format upload ingest, diarization, and
  meeting notes. Release notes: `docs/RELEASE_NOTES_v3.1.0.md`.

- **`v3.2.0-nightly.2`** (2026-08-01, Codex) — **personal voice insights,
  responsive Settings refinement, and avatar-gallery polish.** Release notes:
  `docs/RELEASE_NOTES_v3.2.0-nightly.2.md`.
  - Insights derives an on-device recent voice signature from retained raw
    microphone transcripts: pace and consistency, session shape, sentence
    shape, vocabulary breadth, question share, discourse markers, and adjacent
    repetition signals. It excludes uploads, meeting speakers, and transformed
    text, and does not pretend to assess accent or pronunciation from text.
  - The compact Settings header no longer inherits the desktop heading's 360 px
    flex basis as vertical height, removing the windowed-width blank band.
  - One avatar catalog now drives the shell, Settings, and context menu. Names
    are Clippo, Clippy, Blacky, Uru & Gujia, Mochi & Marmalade, Pikachu, Wavy,
    and Siri; user-facing “Companion” copy is now “Avatar”.
  - Picker representations were refined without changing the Codex raster
    packs or Clippy. The validated Codex v2 Mochi & Marmalade atlas is integrated
    byte-for-byte.
  - Verification: `npm run check` is 0 errors/0 warnings, `npm run build`
    succeeds, and `cargo check` succeeds with only pre-existing warnings.

- **`v3.2.0-nightly.1`** (2026-08-01, Codex) — **desktop visual system,
  navigation, onboarding, settings, History, and Insights overhaul.** Release
  notes: `docs/RELEASE_NOTES_v3.2.0-nightly.1.md`.
  - The old 320 px settings-heavy sidebar is now a 272 px navigation rail with
    one compact everyday-controls card. A marker-gated migration resets the old
    width once, then preserves every user resize.
  - Settings no longer nests a second rail inside the first. Its user-goal IA
    is Voice, AI engines, Writing, Companion, App & data, Account, and Advanced;
    container queries make that navigation respond to the content pane rather
    than the outer window.
  - Onboarding is one watercolor fox-in-the-field journey across Welcome,
    Voice, Try it, and Sync. Skip is persisted independently from key state,
    Gemini/OpenAI brain keys are recognised, named progress replaces anonymous
    dots, and compact account mode hides device/destructive controls.
  - History drops the duplicate analytics widget, searches every text variant,
    and uses explicit semantic row expansion with compact-width reflow. Insights
    shares the same field palette, surfaces, typography, and illustrated empty
    state.
  - Shared radius, shadow, focus, motion, field-green, and warm-paper tokens now
    govern the touched surfaces; global reduced-motion support is in place.
  - Verification: `npm run check` is 0 errors/0 warnings, `npm run build`
    succeeds, and `cargo check` succeeds with the pre-existing Rust dead-code
    warnings. The History route was visually/semantically inspected in the
    lightweight runtime. A native `tauri dev` link was stopped when GNU ld
    reached 3.6 GB RAM on this documented 8 GB host; the CI nightly installer is
    the native test artifact. Dependency refresh removed all high/moderate npm
    advisories; three low `cookie` advisories remain upstream with only a
    breaking/invalid audit-force path.

- **`v3.1.0-nightly.4`** (2026-07-28, this local core machine) — **external
  microphone support + meeting capture.** Driven by the DJI Mic 2 research in
  `../dji-mic-utilization/CONTEXT-FOR-WISPRFOX-AGENT.md` (features F1–F8), plus
  a hotkey-picker bug the owner hit mid-session. Notes:
  `docs/RELEASE_NOTES_v3.1.0-nightly.4.md`. Typechecked (`cargo check` clean,
  `npm run check` 0 errors) and the pure logic is unit-tested; **not run from a
  built binary** — this nightly exists to be tested.
  - **Mic picker** (Settings → Dictation + sidebar). `input_device` on
    `AppSettings`; `audio::resolve_input_device` matches exact-then-prefix and
    **falls back to the system default when the saved device is absent**, which
    is the normal case for an external mic (switched off / unpaired). The
    resolved device is written to the flight recorder.
  - **Handover "hold on" state.** The first cpal callback now publishes
    **`wispr:mic_live`** live (it was only computed at stop, as `mic_ready_ms`).
    The floater holds a waiting presentation — escalating copy + a breathing dim
    on `.clippy-stage` — until audio genuinely flows. With Bluetooth that gap is
    2–10s and everything said inside it never reached the WAV.
  - **Quiet-audio rescue — the highest-value change here.** Too-quiet audio does
    not fail loudly; it returns a plausible transcript with phrases *deleted*.
    Measured fixture: −46.4 dBFS RMS lost a whole sentence to a single "uh";
    +24 dB peak-normalised recovered it with **zero clipped samples**. New
    `audio::level` measures every recording, warns under −40 dBFS, and boosts a
    **copy** under −30 dBFS. The WAV on disk is never modified. Opt out via
    `auto_gain`.
  - **Mic test** (`start_mic_test` → metering-only preview stream) with real
    dBFS against a target band + the measured head-gap. This is the only way to
    catch a **phantom-connected Bluetooth device**: after sleep/wake it keeps its
    connected LED and stays enumerated while sending no audio. Enumeration can't
    see that; a meter can.
  - **Slow-mic guidance is now reachable.** It existed at `flow.rs` but fired
    once per run, only *after* a damaged recording, into a 2-line-clamped bubble.
    Now a permanent Settings section **split by transport** — audio-enhancements
    /exclusive-control for wired, *noise-cancellation-off-before-connecting* for
    Bluetooth. Different mechanisms, different fixes; the wrong advice wastes
    the user's time. `looks_bluetooth()` picks the message at runtime too.
  - **24-bit / 32-bit-float ingest (was a hard blocker).** `chunk.rs` read
    `into_samples::<i16>()` and collected with `?`, so any 24-bit upload over
    20 MB **failed the entire transcription** — that's every external-recorder
    file past ~2.3 min at 48 kHz mono. `denoise.rs` shared the assumption via
    `filter_map(ok)` and silently dropped every sample (noise reduction quietly
    did nothing). New **`audio::wavio`** decodes 16/24/32-bit int + float,
    downmixes to mono, and canonicalises uploads to 16 kHz mono PCM at ingest
    (~9× smaller, no transcript cost).
  - **Diarization + meeting notes.** `SttOptions` replaces the positional lang
    hint across all four providers. Deepgram → `diarize_model=latest`,
    ElevenLabs → `diarize=true`; `stt::speakers` groups words into turns and
    normalises provider speaker ids to first-seen order. Upload dialog gains
    "Label speakers" (**gated on provider capability** — Whisper has no speaker
    model — with the cost difference shown) and "Meeting notes" (prompt override
    on the Drafting path; **not** a new `ClippyMode`, which would be a
    sync-schema migration).
  - **Hotkey rebinding actually works.** Registration is live now
    (`hotkey::install`/`apply`/`suspend` replace boot-only `register`).
    Capturing suspends the global shortcuts first: with F8 still registered the
    OS consumed the keypress and started a *recording*, so the keys users most
    wanted to bind were exactly the ones that couldn't be. Bindings apply
    instantly (no Save button, no restart) and duplicates are rejected by name.
  - ⚠️ **`cargo test` still can't run locally** (the test binary dies with
    STATUS_ENTRYPOINT_NOT_FOUND against Tauri), **and there is no CI test job** —
    `release.yml` only builds. So the 18 new tests live in dependency-free leaf
    modules (`audio::wavio`, `audio::level`, `stt::speakers`) and were run via a
    scratch crate that `#[path]`-includes those files. If you touch that logic,
    re-run them the same way, or add a test job.

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
  commit `113b401`; CI published all-platform installers as Latest). Rolls up
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

0. **Test `v3.1.0-nightly.4` against the real DJI Mic 2** — this is the whole
   point of that nightly and none of it has been run from a binary. In order:
   pair the TX over Bluetooth with **noise cancellation OFF**, pick it in
   Settings → Dictation, hit **Test microphone** (confirms the meter, the
   head-gap number, and that the device isn't phantom-connected), then dictate
   and watch for the "hold on" state. Then drop
   `../dji-mic-utilization/DJI_01_20260728_174153.WAV` in as an upload — it's the
   −46.4 dBFS fixture, so it exercises the quiet-audio rescue, and a longer
   24-bit recording exercises the ingest transcode. Finally a two-person
   recording with **Label speakers + Meeting notes** on Deepgram or ElevenLabs.
   - **Still unmeasured:** whether the residual ~2 s Bluetooth SCO setup can be
     removed by an opt-in "keep the mic warm" setting (holding the input stream
     open). That contradicts the deliberate cold-start-per-press design and
     costs TX battery + pins the OS mic indicator on, so it's a real
     architectural decision, not a tweak. Deferred until the measured gap with
     NC off is known.
1. **macOS signing** — user adds 3 GitHub secrets + uncomments the env block in
   `.github/workflows/release.yml` per `docs/MACOS_SIGNING.md`. Fixes the
   Accessibility re-grant reset on every update AND the Mac auto-paste report.
   Long-standing, still pending.
2. **v3.0.0 stable — SHIPPED 2026-07-18.** (Done: tagged `v3.0.0` on `113b401`,
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

- **A global shortcut beats the focused webview — always.** The rebind picker
  looked broken for F8/F9 because the OS consumed the keypress and started a
  recording before the DOM listener ever saw it. No amount of
  `preventDefault`/`stopPropagation` helps; the event is never delivered.
  `hotkey::suspend` for the duration of the capture is the only fix, and it must
  be paired in a `finally` (plus `onDestroy`) or a navigation mid-capture strands
  the user with no hotkeys at all.
- **Quiet audio fails SILENTLY, and that is the dangerous part.** It doesn't
  error — it returns a confident-looking transcript with content deleted, and
  neither the user nor the provider reports anything. Any future audio work
  should assume "it produced text" is not evidence it produced the *right* text.
- **Don't assume the recorder's own format.** Several call sites hard-coded
  `into_samples::<i16>()` because that's what we write. Anything a *user* hands
  us is likely 24-bit (every external field recorder defaults to it). Use
  `audio::wavio::read_mono_f32`.
- **"Enumerated" ≠ "working" for Bluetooth input.** After a sleep/wake cycle a
  transmitter can keep its connected LED, stay in the OS device list, and
  deliver no audio at all until power-cycled. Any UI that only checks presence
  will confidently select a dead mic. That's why the mic test exists.
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
