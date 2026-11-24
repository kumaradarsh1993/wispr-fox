# THETA_MEMORY — wispr-fox (all three clients, append-only)

Detailed log of what was done, why, and what it changed, across desktop (this repo), Android (`../wispr-fox-android`) and web (`../wispr-fox-web`). Add new entries at the bottom; never rewrite old ones. Each repo's `HANDOVER.md` is its current state. The Android and web repos keep shorter logs of their own.

## 2026-11-03 — stable releases, sync consistency check, Android revamp

**Inputs.** Adarsh's dictated brief, kept verbatim in `docs/briefs/2026-11-03 brief - sync check and Android revamp.md`. The checklist and progress log are in the same folder.

**1. Stable releases first.** Adarsh wanted everything current on stable before the next round of changes.
- Desktop `v3.6.0`, cut on `keychain-fix`: Conversations plus one Keychain item per build.
- Android `v2.3.0`, cut on `conversations`.
- Web was already live with Conversations.

**2. The missing phone conversation.** The phone's second upload of the same audio, with speakers renamed, never reached the desktop. Four causes:
- **Android echo overwrite.** Pull applied the phone's own pushed rows back as pulled copies (remote, no audio), so they stopped pushing. Likely the main cause.
- **Stuck retries.** A conversation waiting on a retry of a failed step stayed un-done, so it was never pushed.
- **Strict cursor pull.** The pull read only rows stamped after its cursor. Each client stamps rows with its own clock, so a row from a device whose clock runs behind could be skipped forever.
- **Storage cap.** Desktop's 500 MB audio cap deleted whole rows along with their audio. That's why this Mac held only the 18 rows from 3 November, though retention is 7 days.

The real server state was not inspected. That would need Adarsh's account token, which Kiro doesn't read.

**3. Refresh = consistency check.**
- **Desktop:** `SyncEngine::reconcile` compares the server's index of rows with this device's. It then fetches missing or newer rows, removes rows deleted elsewhere, re-pushes this device's rows the server lacks, and restores this device's lost rows.
- **Android:** the same check, plus repairing rows the echo bug marked as pulled.
- **Web:** it keeps no local copy of other devices' notes. Its check re-sends this tab's notes that failed to save, a failure that was being swallowed before.

**4. Android revamp** (`v2.4.0-nightly.1`).
- Dictation into other apps is opt-in. Turning it off switches the accessibility service off, which is what banking apps detect.
- 90% type scale and tighter spacing.
- Bottom bar: Speak, Upload, History, Settings.
- Upload presets, with the 3 newest Voice Recorder files to pick.
- One copy holds both transcripts: Deepgram with speakers first, then Whisper.
- History actions on the tab row, plus a Refresh button.
- Storage page that separates kept recordings from leftover copies of shared files.

**5. Models check.** Groq, Deepgram and ElevenLabs were verified live, and no changes were needed. The Gemini and OpenAI pages render client-side and couldn't be read.

**Effects.**
- After installing the desktop nightly, the first Refresh should restore this Mac's lost history from the server.
- Phone rows keep syncing after their first push.
- Banking apps stop warning while the switch is off.

**Open.**
- Nothing has been tested on the S23 Ultra or against the live account.
- Adarsh pushes `main` in all three repos himself.
- The web `sync-check` branch isn't live.

## 2026-11-04 — Mac hotkey hotfix (Option+Enter)

- **Input:** Adarsh said Option+Enter started a recording and broke Excel's new line inside a cell. He wanted Option+Space to stay the F8 equivalent and Shift+Option+Space to become the F9 equivalent.
- **Cause:** Option+Enter was the Mac Drafting default. A global shortcut always beats the app in front.
- **Done on this Mac:**
  - waited for 2 quiet minutes with no recording, quit the app, edited `user-prefs.json` (backup next to it), reopened the app and verified the change stuck;
  - Drafting is now Shift+Alt+Space;
  - Force clean moved to Ctrl+Shift+Alt+Space, since Drafting took its key;
  - the sticky versions with Cmd moved the same way.
- **In code (commit `369c215`):** new Mac defaults in `settings.rs`, a one-time migration step 5 (`macHotkeyV3Migrated`) that only touches fields still holding a default, and the README key table.

## 2026-11-23 — Whisper step on long conversations; v3.7.0 stable; release-notes cleanup

- **Input:** a 95-minute phone conversation (65 MB .m4a). Deepgram labelled the speakers, but the Whisper step failed with "file is 65 MB — too large for Groq Whisper in one request". Adarsh had assumed long files were already split.
- **Cause:** splitting only worked on WAV. Phone uploads are .m4a, and `run_accuracy_pass` rejected any non-WAV file over 24 MB before sending anything.
- **Fix (v3.7.0-nightly.2, commit `0c331eb`):**
  - a non-WAV file over 24 MB is converted with macOS `afconvert` to a temporary 16 kHz mono WAV, then split into parts of about 20 MB;
  - the deadline is scaled per part;
  - the real file converted in 2.3 s to 174 MB, which is about 9 parts.
- **Install:** after a 2-minute recording-idle check, this Mac moved to nightly.2 and the signature verified. AppleScript quit was refused (-10004), so the app was stopped with SIGTERM.
- **Stable:** v3.7.0 = v3.7.0-nightly.2, promoted on Adarsh's say-so.
- **Release notes:**
  - 51 nightly, Codex and preview notes moved to `docs/release-notes-archive/`;
  - the stable notes for 3.6.0 and 3.7.0 were rewritten short and user-facing;
  - `release.yml` now appends a short download and first-launch block instead of the long install essay and the provenance line;
  - the published v3.6.0 body was replaced with the short version.
- **Open:**
  - voice-note uploads of long .m4a files with Groq or OpenAI still fail;
  - the Whisper retry on the 95-minute file has not been confirmed;
  - Adarsh pushes `main`.
