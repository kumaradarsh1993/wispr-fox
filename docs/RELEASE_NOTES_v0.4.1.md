# v0.4.1 — Hotfix: kill the "recording already ongoing" ghost toast for good

The v0.3.1 silencer for Windows' auto-repeat WM_HOTKEY had a race
condition that kept biting: while you were dictating, a red toast
would appear saying "recording already ongoing", Clippy's listening
ear would vanish, and the icon would reset to idle — even though
the audio was still recording fine in the background. Releasing
the key would then correctly run the transcription pipeline,
proving the recording itself was OK; it was just the UI freaking
out.

---

## What was wrong

Windows fires `WM_HOTKEY` ~30 times per second while you hold a
global hotkey (auto-repeat). The v0.3.1 fix swallowed these by
checking `state.active.is_some()` inside the spawned async task —
but that check happened *after* the spawn, so two near-simultaneous
Down events could both see "no active recording yet" and both call
`start_recording_async`. The second one then errored with
"recording already in progress" → that error went out as a
`wispr:flow_error` event → the Clippy front-end's error handler
**force-resets state to idle on any flow_error** → ear disappears,
red toast. The original recording task in the audio thread was
unaffected, so when you released the key, the pipeline ran to
completion normally.

---

## What's fixed

1. **Synchronous debounce gate** at the top of the hotkey dispatcher.
   Any Down event within 150 ms of the previous accepted Down is
   dropped *before* any task is spawned, so there's no longer a
   race window. 150 ms is well above Windows' auto-repeat rate
   (~33 ms) but well below a human's minimum legitimate re-press
   cadence.

2. **"Already in progress" no longer surfaces as a UI error.** Even
   if a stray Down somehow slips past the debounce (e.g. you spam
   F8 and Win+F8 alternately), the resulting "recording already in
   progress" error is now logged at debug level and *not* emitted
   to the front-end. So Clippy stays in the listening state, the
   ear doesn't disappear, no toast. The original recording task
   keeps going as if nothing happened — because nothing useful did.

Both fixes apply to push-to-talk (F8 hold) AND sticky (Win+F8
toggle) hotkey modes.

---

## ⬇ Get it

Windows: `wispr-fox_0.4.1_x64-setup.exe` below. Install over your
v0.4.0 — everything carries over.
