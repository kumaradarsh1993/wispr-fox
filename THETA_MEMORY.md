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
