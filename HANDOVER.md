# wispr-fox — handover

> Single "where are we / how to resume" doc. Read this first, then
> `ARCHITECTURE.md` for current topology and the approved target state model,
> and `CLAUDE.md` for conventions and ground rules. Everything else is a
> specialist doc (see the map at the bottom).
>
> **Last updated: 2026-08-25** (**`v3.3.0` is stable and Latest**; the current
> desktop candidate is `v3.4.0-nightly.1`. **`v3.3.0-nightly.2` is burned — it
> froze on the first hotkey press; do not install or test it.**)

## 2026-08-27 — one update module, shared across all four Fox desktop apps

`docs/UPDATES.md` is the contract; **the same `src-tauri/src/updates.rs` and the
same `UpdatePanel.svelte` now ship in wispr-fox, FoxCull, Fox MD and Fox Mark**,
differing only in three constants. Fix a bug in one, copy it to the other three.

What it buys: two channels visible at once, and on Windows an Install button
that downloads, runs the NSIS installer **silently** (`/S /R`) and relaunches —
no wizard, no uninstall/reinstall. macOS and Linux download and open, which is as
far as an unsigned build can honestly go.

Three things that will silently break it, all documented in `docs/UPDATES.md`:
a nightly published as a **draft** is invisible to the API; a **renamed CI
artifact** degrades Install to "no installer for this platform" rather than
erroring; and a **string-compare** version check sorts `nightly.10` below
`nightly.9`. The last two are pinned by `md-reader/tools/updates-selftest`, which
slices the real `updates.rs` rather than restating it — 9/9 passing.

Local to this repo: the old `src-tauri/src/updater.rs` is gone, `commands.rs`
lost its duplicate `version_is_newer`, and Settings → About is now a thin frame
around the shared panel. The event renamed from `wispr:update_progress` to
`update://progress`.


## Where we are (2026-08-25)

**`v3.3.0` promoted to stable** on the user's explicit signal — the five v3.3
nightlies (meetings, adaptive tap-or-hold, the nightly.2 freeze fix, three
false-alarm fixes) plus two layout defects he reported against nightly.5:

- History card text was capped at a fixed `max-width: 88ch`. Measured on an
  1800px window: **45% card fill**, and the same sentence wrapped at the same
  word regardless of window size. Now 95%.
- Insights was itself the scroll container AND `max-width: 1040px; margin: 0
  auto`, so its scrollbar floated mid-pane with dead surface either side. Now
  full-bleed, matching History (`.rows`) and Settings (`.section-body`).
- Insights breakpoints were `@media`, which measures the WINDOW — but the page
  only ever gets window-minus-sidebar. A 1100px window put a 828px pane through
  the ">920px" branch and crushed four stat cards into 176px each. Now
  `@container stats`.

**Rule this produced: in this app a breakpoint is almost always a container
query.** The sidebar is 272px, user-resizable and collapsible, so the window
width is never the pane width. Settings and History already did this correctly;
Insights was the straggler.

**`v3.4.0-nightly.1` — the fleet.** Insights merges analytics across every
signed-in device (the user had two desktops reporting two different "since"
dates and two different totals); history cards show which device produced them;
Settings → Account lists every device with an assignable icon and name. New
`src-tauri/src/sync/fleet.rs`, `src/lib/fleet-store.svelte.ts`,
`src/lib/device-icons.ts`.

**It needed NO Supabase migration** — device identity and per-device rollups
ride on the generic `user_settings` KV that `SYNC_DESIGN.md` already reserves
for "future shared prefs", plus the `devices` table the engine has written
since v3.0.0. Worth remembering before proposing DDL on the user's project: the
KV table absorbs most "we need one more synced field" asks.

### Owed / next

- **Live multi-device test.** The fleet merge was verified against a mocked
  three-device account in a browser, not against real Supabase rows. Sign a
  second machine in and confirm the merged "since" date and totals.
- This is the same class of gap as the delete/purge paths noted below — both
  are protocol code that has only ever been typechecked and CI-built.
- **Codex avatars: nothing to inherit** (checked 2026-08-25). All 8 Codex CLI
  pets are already in `static/pets/`, and the only pet in `~/.codex/pets/` is
  `mochi-marmalade`, which is **byte-identical** (md5) to the shipped copy. Do
  not re-investigate without a new pet appearing there first.

> **⚠ Multi-machine workflow note.** This repo is worked from more than one
> machine. **Before starting new work, `git fetch` and reconcile the live branch,
> tag, and worktree state.** The original `v3.1.0-nightly.1` cloud branch has
> already landed; do not treat that historical branch as pending. Coordination
> rules live at the bottom of `CLAUDE.md` ("Multi-machine / multi-agent workflow").

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
optional account/sync, no telemetry.

Public repo: <https://github.com/kumaradarsh1993/wispr-fox>

## Current state

- **`v3.4.0-nightly.12` (2026-09-02) — the Spaces diagnosis was WRONG, proved by
  the user's own diagnostic, and the investigation restarts from a measurement.**
  His readout on nightly.8: `floater level: 25`, `collectionBehavior: 257
  (0x101)`, `pinned to all Spaces: true` — **exactly what nightly.7/.8 set out to
  achieve** — and the avatar still only ever appears on the desktop the app
  launched on. **The window configuration was never the problem.** Three
  releases went into fixing it. The one real defect found along the way (tao's
  `set_always_on_top` clobbering the level) stays fixed; it simply was not this.
  - **New `space_probe.rs` samples `NSWindow.isOnActiveSpace` every 2 s** and
    keeps two minutes of history, surfaced in the diagnostic as a
    `1`/`0`/`·` timeline. This splits the two failures that produce an identical
    user report and that NO amount of reading the window's configuration can
    tell apart: **(a)** macOS is not placing the window on the active Space →
    something at the *application* level is overriding the collection behavior;
    **(b)** macOS IS placing it there and nothing is drawn → the transparent +
    `macOSPrivateApi` compositing failure this app already has history with,
    which has nothing to do with Spaces. **Do not ship another fix until this
    timeline has been read.**
  - **`show_floater` now forces hide()→show() when the window is already
    visible** (macOS only). Candidate fix for (b): `show()` on an
    already-visible window is a no-op at the WindowServer level, so a floater
    that has been on screen since launch is never re-registered and stays where
    it was first composited. Same cycle the startup path already uses.
  - ⚠️ **Two more user-reported bugs, both unfixed, both likely related — do not
    treat them as cosmetic.**
    1. **The floater is visible from launch and stays there, with visibility set
       to `auto`.** Windows behaves correctly; only macOS is wrong. It hides
       only after the first dictation completes. This matters beyond the
       annoyance: a permanently-visible window is what makes every `show()` a
       no-op, which is the mechanism behind candidate (b). The startup show in
       `lib.rs` is unconditional because Rust cannot read the localStorage the
       visibility tri-state lives in — **the real fix is to move avatar
       visibility into the Rust settings store** so startup knows the mode.
       That also kills the launch-flash papercut CLAUDE.md already records.
    2. **The whole app stops responding to clicks, twice, recoverable only by
       force-quitting.** Hover highlights still worked; clicks did not register;
       no crash, no error. The suspect is the floater at `NSStatusWindowLevel`
       (25) taking key-window status — Tauri's `show()` is
       `makeKeyAndOrderFront:`, so every show makes the floater key and the
       main window cannot take input. If confirmed, the fix is to stop the
       floater ever becoming key (non-activating panel / `canBecomeKeyWindow`
       false); mouse events do not need key status, only the keyboard does.
       **Not shipped — it is a guess, and this session has already shipped
       three of those.**
  - Also reported: a brief flash of the window border as the avatar exits (not
    the retired opaque-era `.mac` CSS — that only styles the wave skin).

- **`v3.4.0-nightly.10` (2026-09-02) — macOS builds are signed with a STABLE
  identity; the Accessibility grant stops resetting on every update.** This was
  open thread #1 for months and was written up as work the owner owed on his
  Mac. It did not need a Mac.
  - **Certificate generated with OpenSSL on this Windows box**, not Keychain
    Access: `CN=wispr-fox self-signed`, RSA 2048, `extendedKeyUsage=codeSigning`
    critical, `basicConstraints=CA:true` (matching what Certificate Assistant's
    "Self Signed Root" + "Code Signing" produces, deliberately — that is the
    shape known to work). Valid to **2046-08-27**.
  - ⚠️ **The PKCS#12 must not use OpenSSL 3's default encryption.** macOS
    `security import` cannot read AES-256/PBKDF2 p12 files, and the failure
    surfaces inside tauri-action as an opaque import error. Exported with
    `-keypbe PBE-SHA1-3DES -certpbe PBE-SHA1-3DES -macalg sha1`; verified the
    readback says `pbeWithSHA1And3-KeyTripleDES-CBC`, and that the enclosed key
    and cert share a modulus (a p12 without a matching private key is not an
    identity and `codesign -s` will not find it).
  - Secrets set via `gh secret set` (they are write-only afterwards). The
    **only readable copy** of the cert + password is
    `D:/android-dev/keystores/` alongside the Fox MD Android keystore, with
    `README-wispr-fox.txt` explaining what breaks if it is lost — same
    convention, same folder, so there is one place to look.
  - **CI now asserts the identity**, not just the seal. `codesign --verify`
    passes for an ad-hoc signature too, so a silent fallback would restore the
    every-update reset invisibly. The macOS job fails unless the built app
    reports `Authority=Developer ID Application: wispr-fox self-signed`.
  - ⚠️ **nightly.9 FAILED the macOS build and the error named the wrong thing.**
    `failed to resolve signing identity`, two lines after `1 identity imported`
    — so the p12, password and private key were all correct. The cause is that
    **`tauri-macos-sign::identity::list()` finds a certificate by running
    `security find-certificate -c <prefix>` for seven hard-coded Apple
    prefixes**, and **`Team::from_x509` then requires an
    organizationalUnitName** and silently drops any certificate without one. A
    self-signed cert called `wispr-fox self-signed` is invisible to it. That
    one message covers a missing cert, a wrong password, a keyless p12 AND a
    misnamed subject, so it cannot be used for diagnosis — read
    `crates/tauri-macos-sign/src/keychain/identity.rs`. Regenerated as
    `CN=Developer ID Application: wispr-fox self-signed, OU=wispr-fox,
    O=wispr-fox`; `APPLE_SIGNING_IDENTITY` must be a SUBSTRING of that CN.
  - **Trust was the obvious first guess and it was wrong.** Signing runs
    `codesign -s "<CN>" --keychain <path>`, which resolves by name and does not
    apply the codesigning trust policy — `security find-identity -v -p
    codesigning` is not in the path at all. A trust step is in CI anyway
    (public cert committed at `.github/wispr-fox-signing-cert.pem`, no private
    key) because a CI round trip costs 15 minutes and it removes the unknown.
  - **`docs/MACOS_SIGNING.md` rewritten from a to-do into a record.** It read
    as five manual steps owed by the owner; it is done, and the Keychain
    walkthrough is retained only for replacing the cert.
  - **The identity change resets the grant ONE more time** — expected, and
    called out in the release notes. From the next update it persists.
  - ⚠️ **The Bash-heredoc backslash trap bit twice in one session**, both times
    in content I had just been warned about by this repo's own docs: `\n` in a
    Python heredoc became a real newline inside a JS string literal (caught by
    `npm run check`), and `D:\android-dev` became `D:<BEL>ndroid-dev` inside
    `release.yml` — which would have shipped a workflow containing a control
    character. **`grep -nP for a BEL byte` after any heredoc write, and parse the YAML,
    is the cheap guard.** A workflow with a BEL in it fails in a way that looks
    nothing like the cause.

- **`v3.4.0-nightly.8` (2026-09-02) — macOS auto-paste was discarding text
  silently; Spaces still open.** Two reports from the M-series MacBook Air.
  - **`CGEventPost` succeeds without Accessibility permission and delivers
    nothing.** `inject::inject`'s macOS arm called `macos::send`, got `Ok(())`,
    and reported `Channel::SendInput` — a success — while the transcript
    reached no app, no clipboard and no error. The module doc claimed a
    "falls back to clipboard + Cmd+V if Accessibility hasn't been granted"
    path that **had never been written**, and could not have worked: ⌘V is
    also a CGEvent and is dropped by the same rule. Fixed by checking
    `is_accessibility_trusted()` BEFORE choosing a channel; without it the text
    goes to the clipboard as `Channel::ClipboardNoPaste` and the floater says
    so. **Generalises: an API that cannot fail is not the same as an API that
    worked. Where the OS silently drops the operation, the precondition is the
    only observable — check it, don't infer from the return.**
  - **"Accessibility is enabled and it still doesn't work" is expected, not a
    corruption.** TCC files the grant against the code-signing identity; ad-hoc
    signatures change hash every build; the old entry survives, still switched
    on, bound to a binary that is gone. **Toggling the switch does not rebind
    it** — the entry has to be removed. New `repair_accessibility` command runs
    `tccutil reset Accessibility com.wispr-fox.app` then
    `AXIsProcessTrustedWithOptions(prompt: true)`, surfaced as a **Repair
    permission** button. The banner said the right thing already — inside a
    `title` tooltip nobody hovers. It is body text now.
  - ⚠️ **The real fix is `docs/MACOS_SIGNING.md`** — one stable self-signed
    cert, three GitHub secrets, and the commented block in `release.yml`. Until
    then this recurs on every single update. **Open question for the owner: he
    can do it on the Mac per the doc, or an agent can generate the cert with
    OpenSSL here and set the secrets with `gh secret set`.** Not done either
    way — creating a signing credential on his behalf is his call.
  - **Spaces: NOT fixed, and nightly.7's fix has not been disproved either.**
    The level-clobber it found was real (`set_always_on_top` → bare
    `setLevel: 3`) and is still fixed. What is unknown is whether the pin is
    now taking effect at all on his Mac, or whether he was even running
    nightly.7. Two changes rather than a third theory: `show_floater` now pins
    **before** `show()` (collection behavior is what decides which Space a
    window is placed on, so setting it afterwards is a frame late) and adds
    `orderFrontRegardless`; and the clippy `$effect` calls it on every
    dictation start in **`always`** mode too, which previously touched the
    window only at launch and every 30 s.
  - **Reverted my own nightly.7 addition:** `Stationary` (1 << 4) and
    `IgnoresCycle` (1 << 6) are gone. They were tidiness, never ran on a Mac,
    and sit in the "at most one of" groups AppKit documents. **Adding untested
    flags to a feature that is already failing makes the failure harder to
    attribute, not easier.** Back to the canonical two-bit overlay recipe.
  - **New `platform_diagnostic` command + Settings → About → Run diagnostic.**
    Version, exe path, `.app` bundle path, `AXIsProcessTrusted`, and the
    floater's live `NSWindow.level` / `collectionBehavior` read back off the
    real window, with Copy. **This is the point:** two macOS bugs in a row have
    been diagnosed by reasoning from Windows, and one of those diagnoses was
    wrong. It also settles the cheapest hypothesis nobody could check — whether
    the Mac is even running the build being debugged.

- **`v3.4.0-nightly.7` (2026-09-02) — the macOS floater was pinned to the wrong
  desktop, because a 30 s watchdog kept un-pinning it.** User report (M-series
  MacBook Air): with several full-screen apps open, pressing the dictate key
  showed the avatar on the desktop wispr-fox was first opened on, never on the
  one being dictated into — so there was no visible sign a recording was live.
  - **nightly.5's fix was real and it worked for 30 seconds.**
    `macos_pin_floater` sets `NSStatusWindowLevel` (25) plus a collection
    behavior of `CanJoinAllSpaces | FullScreenAuxiliary`. The Layer-2 watchdog
    in `lib.rs` then re-asserted `set_always_on_top(true)` every 30 s — and
    **tao implements that as a bare `setLevel: NSFloatingWindowLevel`**
    (`tao-0.35.2/src/platform_impl/macos/window.rs:1391`), which resets the
    level to 3. Level 3 cannot paint over another app's full-screen Space, so
    from t+30 s onward the floater was stuck on its birth Space for the rest of
    the session. Nothing logged, nothing failed — the call reads as harmless.
  - **Generalises past this repo: a setter named for an intent can be
    implemented as an assignment.** `set_always_on_top` sounds additive and is
    destructive. Any place we reach past a framework API with raw platform
    calls, the framework's own API can silently stomp them — so the raw state
    needs one owner, and every other caller has to go through it.
  - Fix: `pin_floater()` is now the single entry point (macOS → the objc2 pin;
    elsewhere → `set_always_on_top`). The watchdog, the resume recovery
    (`power.rs`), `force_repaint`, the Reopen handler and startup all call it.
  - **The pin is also re-applied at show time**, via a new `show_floater`
    command that the floater's JS calls instead of `getCurrentWindow().show()`.
    In `auto` visibility the window is hidden between dictations, so the Space
    it was pinned to at launch is not the Space the user is on when they next
    press the key. `avatar-visibility.svelte.ts` and `skin-store.svelte.ts` use
    it too.
  - Collection behavior gained `Stationary` (1 << 4) and `IgnoresCycle`
    (1 << 6) — it is an overlay, not a document window, so it should be out of
    Mission Control and Cmd-` cycling. The pin now **reads back** the level and
    behavior AppKit actually kept and logs both; that is the only evidence a
    Windows dev box can get about a Mac-only call.
  - **Guarded, not just fixed:** `floater_pin_guard` in `lib.rs` asserts on the
    source text that `set_always_on_top` appears exactly once in the crate
    (inside `pin_floater`) and that `show_floater` re-pins. It runs in the
    `cargo test --lib` CI step on Linux, so it protects the Mac from a
    Windows-authored change even though the code it guards is `cfg`'d out here.
  - ⚠️ **Unverified on hardware from this machine.** The changed code is all
    `#[cfg(target_os = "macos")]`, so `cargo check` on Windows never compiles
    it; `cargo check --all-targets` and `npm run check` are clean, and the
    macOS CI job on the tag is the first real compile. Ask the user to confirm
    on the MacBook before this line is promoted to stable.

- **`v3.3.0-nightly.5` (2026-08-19) — the capture-gap alarm was measuring the
  wrong thing, and error bubbles were unreadable.** User report: a red box said
  "only 336s of 339s captured, record again", rendered ~3 words per line over
  ~10 lines and still scrolling.
  - **Diagnosed from the WAV, and one hypothesis died on the evidence.** First
    scan claimed 9,723 zero-runs of exactly 20 ms — a scary "driver feeding
    silent buffers" story. It was a bug in the scan (`start` was never reset on
    a short run, so every zero-to-nonzero transition ≥960 samples away got
    counted). Re-checked: **the longest zero run in the file is 4 samples.**
    No silence, no zero-fill.
  - **Where the audio went:** nowhere findable, because it is not one hole.
    3.39 s short over 339 s = 1.0%, with no envelope discontinuity anywhere and
    a coherent transcript start to end. That is WASAPI shedding individual
    ~10 ms capture buffers — the cpal callback writes the WAV inline
    (`wav.write_sample` under a mutex on the real-time thread), so any disk or
    scheduler stall costs buffers. cpal reported no stream error, which is why
    `stream_errored` was false.
  - **The baseline proves the arithmetic is otherwise sound:** every healthy
    recording reads exactly **-0.21 s** (captured EXCEEDS duration) regardless
    of length, 0.1 s to 692 s, because `duration_ms` is snapshotted before the
    220 ms tail drain. A constant offset, not drift.
  - Fix: `audio::is_capture_gap(duration_ms, captured_ms, stream_errored)` —
    shortfall must clear BOTH `CAPTURE_GAP_FLOOR_MS` (1 s) and
    `CAPTURE_GAP_PERCENT` (3%) of duration; a cpal stream error always reports.
    Replaces the flat 1000 ms in `flow.rs` and the flat 750 ms in `audio/mod.rs`
    (which had drifted apart from each other). Message reworded — the old one
    told the user to re-record to "get the rest", which was never possible.
  - **Error bubbles get their own geometry.** `BUBBLE_W` (226) is deliberately
    narrow for status text and the user has explicitly asked to keep it that
    way; errors now use `ERROR_BUBBLE_W` (430) + `ERROR_TEXT_MAX_H` 180→260,
    applied only while an error is on screen. `boxFor` takes the width as a
    parameter. **The CSS `max-width` must track `ERROR_BUBBLE_W`** — the window
    sizes off the constant, so a narrower max-width silently re-creates the
    ribbon.
  - **Generalise: a threshold on an absolute quantity that scales with duration
    will fire on the longest, most valuable recordings.** Loss that is
    proportional needs a proportional test.
  - Verified: `cargo check --all-targets` clean, `npm run check` 0/0, and the
    five new `capture_gap_tests` — built from the real flight-recorder numbers
    and **extracted from the source file rather than retyped** — pass in a
    scratch crate; CI's `verify` job runs them for real.

- **`v3.3.0-nightly.4` (2026-08-16) — the "Mic was very quiet" warning was
  crying wolf.** User report: a red box saying the mic was very quiet appeared
  on roughly every other dictation, while every transcript came back perfect.
  - **Measured before theorising, and the first hypothesis was wrong.** The
    guess was that whole-file RMS gets dragged under the line by pauses, so
    speech-active RMS would fix it. Implemented in a scratch script over all 41
    retained WAVs: it moved the warning count 3 → 2. Not the cause. His
    "silence" is room tone sitting within 25 dB of his speech, so ~100% of
    frames count as active on many recordings. The idea was dropped rather than
    shipped as an unvalidated metric.
  - **Actual cause.** His mic's whole operating range is -33 to -40 dBFS RMS
    (median -38.1, n=41) and `QUIET_RMS_DBFS` is -40, so 26 of 41 recordings sit
    within 3 dB of the threshold — a coin flip, not a rare event. Decisively:
    **every one of those recordings was rescued**, boosted 4-21 dB to a -3 dBFS
    peak before transcription. The warning was evaluated on `outcome.stats`,
    i.e. the level *before* the boost that had already fixed it.
  - Fix: `LevelStats::with_gain(gain_db)` and the warn decision now runs on the
    audio speech-to-text actually receives. `quiet_warning` takes the applied
    gain instead of a `rescued: bool`, so the surviving case — a boost clamped
    by `MAX_GAIN_DB` that left the audio quiet anyway — says so explicitly.
    Rescue behaviour, thresholds, and the on-disk recording are all untouched.
  - **Also fixed: the floater auto-hid while a message was still on screen.**
    The auto-mode visibility effect keyed only on `flowState`, so
    `AUTO_HIDE_GRACE_MS` (1.8 s) tore the bubble away while an error toast was
    pinned for `ERROR_TOAST_MIN_MS` (15 s). That is the "truncated" symptom, and
    it silently cut off genuine errors too, not just this warning.
  - **Not a nightly.2/3 regression** — `git log -S quiet_warning` puts it in
    `18dd25f` (external mic support + quiet-audio rescue), latent since v3.1.0.
  - The other two mic detectors were checked against the same telemetry and left
    alone: wake-up runs 0.04-0.52 s (never near `SLOW_MIC_MS`), and the
    mic-dropped check fired once in 30 recordings on a recording that genuinely
    lost ~7 s of audio. Both correct.
  - **Generalise: a warning must be evaluated on the state AFTER the automatic
    remedy, not before it.** Warning about a condition the app just fixed is
    pure noise, and it buries the one case that needed attention.
  - Verified: `cargo check --all-targets` clean (only the pre-existing
    `audio_path` dead-code warning), `npm run check` 0/0, and four new
    `level.rs` cases — built from his real flight-recorder triples — run green
    in a scratch crate; CI's `verify` job runs them for real.

- **`v3.3.0-nightly.3` (2026-08-14) — unfreezes nightly.2.** nightly.2 wedged
  permanently on the FIRST hotkey press: no floater, no recording, no history
  row, tray menu and main window unresponsive, but the process still alive so
  the tray icon looked healthy. Root cause: nightly.2 moved `arm_escape_stop` /
  `disarm_escape_stop` into `Flow::prepare_action`, which runs synchronously
  inside the global-shortcut callback. `tauri-plugin-global-shortcut` 2.3.1
  invokes handlers **while holding its `shortcuts: Mutex<HashMap<..>>`**
  (`set_event_handler` → `shortcuts_.lock().unwrap().get(&e.id)` → `handler(..)`
  with the guard still alive), and `arm_escape_stop` immediately calls
  `is_registered`, which re-locks that same non-reentrant `std::sync::Mutex` on
  the same thread. On Windows that thread is the main/event-loop thread, which
  is why the tray and windows died too. In nightly.1 the same two helpers were
  called from inside `start_recording_async` / `finish_recording_async` — i.e.
  spawned tasks, never the callback thread — which is why only nightly.2 broke.
  - Fix: `prepare_action` now records the intent and applies it off-thread.
    Ordering is preserved by revision (`EscapeIntent::record` / `claim`) so a
    late arm cannot re-arm a session a newer stop already ended — the hazard
    that put arm/disarm in the serialized section in the first place.
  - **Generalise: never call a callback-registry API from inside its own
    callback.** The lock is invisible at the call site; only the plugin source
    shows it. `cargo check`, `npm run check` and the six coordinator tests all
    passed on nightly.2 because none of them go through the real plugin — this
    class of bug is runtime-only and first-keypress-only.
  - Verified: `cargo check --all-targets` clean, `npm run check` 0/0, and the new
    `stale_escape_applier_cannot_rearm_after_a_newer_stop` case executed in a
    scratch crate (the main crate can't link tests locally — GNU ld export
    limit; CI's new `verify` job runs `cargo test --lib`).

- **`v3.3.0-nightly.2` (2026-08-14, Codex) — BURNED, do not install.** Froze on
  the first hotkey press; superseded by nightly.3 above. Its feature work is
  otherwise intact and carried forward. Original description follows — adaptive
  tap-or-hold dictation. Each mode now uses one shortcut: release before 700 ms to latch,
  or hold for 700 ms and release to stop and send. A second configured
  dictation key or Escape stops the original session. Starting, recording, and
  processing remain owned by one serialized, revisioned coordinator, so a cold
  microphone cannot let key-up overtake key-down or let a new run overlap the
  prior pipeline. Microphone readiness is capture-generation scoped, custom
  bindings are re-applied after restart without breaking key capture, and
  terminal errors are complete, scrollable, accessible, and dismissible on
  every floater skin.
  - Verification before release: `npm run check` is 0 errors/0 warnings;
    `npm run build -- --logLevel warn`, `cargo check`, and `cargo check --tests`
    pass. The pure adaptive reducer is 7/7; six coordinator scenarios compile
    locally and execute in release CI through `cargo test --lib`.
  - Runtime QA remains for physical Windows key edges, cold/Bluetooth startup,
    Escape during Starting, floater reload/resync, and very long errors across
    skins. Release notes: `docs/RELEASE_NOTES_v3.3.0-nightly.2.md`.

- **`v3.3.0-nightly.1` (2026-08-11, Codex) - meeting workflow revamp.** Diarized
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
  - Speaker-name edits and regenerated AI versions on rows pulled from another
    device are desktop-local annotations for now. They deliberately do not set
    sync `dirty`, because remote-origin rows are never pushed by this client;
    marking them dirty would block future cloud updates and tombstones.
  - Verification before release: `npm run check` is 0 errors/0 warnings,
    `npm run build` succeeds, and `CARGO_BUILD_JOBS=2 cargo check` succeeds
    with the same nine pre-existing dead-code warnings. Release notes:
    `docs/RELEASE_NOTES_v3.3.0-nightly.1.md`.

- **Stable: `v3.2.0`** (2026-08-11, "Latest") — promotes the complete
  Codex-authored v3.2 nightly line: the fox-in-the-field visual and navigation
  overhaul, rebuilt onboarding, responsive Settings and History, on-device
  recent voice insights, and avatar-gallery refinement. GitHub Actions built
  and published Windows, Apple Silicon macOS, and Linux artifacts from one
  tagged commit. Release notes: `docs/RELEASE_NOTES_v3.2.0.md`.

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
- **Historical implementation record: `v3.1.0-nightly.1`** (2026-07-18, branch
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
  `cargo check` + 3 denoise unit tests + `svelte-check` + vite build. It has
  since merged and shipped; the branch is not pending.
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
| `ARCHITECTURE.md` | Current module/data/trust/release topology + approved target recording FSM |
| `CLAUDE.md` | Conventions, ground rules, platform notes, gotchas (auto-loaded each session) |
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
