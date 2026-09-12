# wispr-fox v3.4.0-nightly.15

**The Upload and Rerun dialogs stopped fighting you.** Five bugs, one theme:
the app did work but refused to tell you about it — and sometimes froze while
not telling you.

All platforms. Windows and macOS both had every one of these; the fixes are
shared code.

## Opening a dialog no longer freezes the whole app

Every time the Upload or Rerun dialog opened, the app checked which API keys
you have saved — seven reads of the OS keychain (macOS Keychain / Windows
Credential Manager), executed on the same thread that draws the window and
services every click and hotkey. Keychain reads go through a system security
service and can stall for seconds; while they stalled, the entire app was a
statue. Dictation hotkeys, the floater, other buttons — all dead until the
keychain answered.

The check now runs on a background thread. The dialog opens instantly and the
rest of the app never waits on it.

## Rerun actually runs

Clicking **Run** on a recording that already had a transcript popped a native
OS confirmation box ("Replace the current transcript?"). Since the app became
a tray-resident background app in nightly.13 — the change that lets the fox
follow you across Spaces — macOS routinely opened that box *behind* every
other window. The app then waited forever for an answer to a question you
never saw. That was the "press Rerun and nothing happens."

The confirmation now lives inside the dialog itself: click **Run selected**
once and it arms — "This will replace the current transcript. Click again to
continue." — click **Yes, replace & run** and it goes. No native dialogs
anywhere in the flow, on either platform.

## You can rerun a diarized recording through Whisper now

A "helpful" rule forced the engine back to Deepgram whenever speaker labels
were on and the selected engine couldn't do them. Because a previously
diarized recording opens the Rerun dialog with labels already checked, every
attempt to pick Groq snapped instantly back to Nova models. You could not get
a plain Whisper transcript of a meeting, period.

The rule now runs the other way: your engine choice wins, and speaker labels
switch off with a note explaining why. Ticking the labels box back on still
auto-selects a diarizing engine, as before.

## "Working…" now ends

Two separate bugs wore the same costume.

The rerun path had **no time limit** on the transcription request — the upload
path capped it at 180 seconds, reruns waited forever. A request that hung left
the dialog saying "Working…" with the close button disabled until you killed
the app, and the recording itself frozen on "Transcribing" in History. Reruns
now get the same 180-second ceiling, and a failed or timed-out rerun marks the
recording as failed — with the retry button available — instead of freezing it.

The upload dialog had the opposite problem: the work finished but the finish
screen self-destructed. A reactivity slip wiped the "3 transcribed ✓ — added
to your history" state the instant the batch completed, so the dialog appeared
to hang on a job it had already done.

## The dialog now tells you what it's doing

The pipeline always knew its stage; it just never said. Uploads now show it
live on the file being processed — **Transcribing… → Cleaning up… →
Drafting… → Writing meeting notes…** — with an *n/N done* counter in the
footer for multi-file batches. Reruns report each step as it starts, as they
already did, and now can't get stuck pretending.

## For the curious

The freeze mechanism in the first fix is worth knowing: in Tauri, a
synchronous command runs on the app's main thread. `check_secrets` was
synchronous and each `secrets::has()` is a full keychain round-trip. Seven of
those behind a single dialog-open is a UI freeze written in invisible ink.
The command is now `async` with the keychain work in `spawn_blocking`. If you
ever add a command that touches the keychain, disk, or network: make it async.
