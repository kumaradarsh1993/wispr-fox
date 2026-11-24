# wispr-fox v3.1.0-nightly.4

External microphones — picking one, proving it works, and stopping it from
quietly eating your words. Plus meeting transcription with speaker labels, and
a hotkey picker that finally lets you press F8.

## Pick your microphone

wispr-fox used to record from whatever Windows called the default. Now you
choose.

- **Settings → Dictation** has an input-device picker, and there's a compact one
  in the sidebar next to the model pickers for when you switch inputs often.
- If the mic you picked isn't connected when you press the hotkey — switched
  off, Bluetooth unpaired — wispr-fox records with the system default and tells
  you afterwards. **A missing mic never costs you a dictation.**
- The mic that actually ran is written into the recording's flight recorder, so
  "why did it use the laptop mic?" is answerable after the fact.

## Test your mic before it costs you a recording

A new **Test microphone** button in Settings → Dictation opens a live meter
showing real dBFS against a target band, plus how long the mic took to start.

This exists because a device being *listed* doesn't mean it *works*. After your
laptop sleeps, a Bluetooth mic can keep its connected light, stay in Windows'
device list, and send no audio at all. A device list can't see that. A meter
can, in about two seconds.

## "Hold on" — the avatar waits for the mic

Pressing the hotkey didn't mean recording had started. On Bluetooth it's 2–10
seconds later, and everything said in that gap was silently lost.

The avatar now shows an explicit **waiting** state from the moment you press the
key until audio is genuinely reaching the recorder, then switches to recording.
Escalating copy — "hold on — waking the mic…" through "mic is taking a while —
check it's on and connected" — so a long wait explains itself instead of looking
like the app hanging.

## Quiet recordings don't silently lose words any more

This was the nastiest bug of the lot, and it never announced itself.

Audio that comes in too quiet doesn't fail. It transcribes into a
plausible-looking transcript with whole phrases **deleted** — no error, nothing
in the UI, no way to tell. On a real external-mic clip measured at −46 dBFS, an
entire spoken sentence collapsed into a single "uh".

wispr-fox now measures every recording's level and, when it's quiet enough to be
at risk, boosts a copy before sending it for transcription. On that same clip
the boost recovered the dropped sentence with zero clipping. **The recording
saved on your device is never modified** — only the copy that gets uploaded. The
level is recorded per-recording too, so a bad transcript can be explained later.

You'll also get a plain warning when a recording was very quiet, because fixing
it at the mic beats relying on a software boost every time. Toggle in
Settings → Dictation if you'd rather it left your audio alone.

## Slow-mic help you can actually find

The advice for a slow mic existed, but it fired once per app run, only *after* a
damaged recording, in a floater bubble capped at two lines. It's now a
permanent, readable section in Settings → Dictation — and it distinguishes the
two causes, which have completely different fixes:

- **Built-in or wired mic** — turn off audio enhancements and exclusive control
  in Windows Sound settings. Typically takes a ~5 second wake-up to instant.
- **Bluetooth mic** — a different mechanism entirely; the enhancements fix does
  nothing. The dominant delay is the mic negotiating itself out of noise
  cancellation, which it must do because NC can't run while streaming over
  Bluetooth. Turn NC off *before* connecting.

## Long recordings from external recorders now work

Recording a meeting on a field recorder and dropping the file in was broken
above about two and a half minutes, and failed outright rather than degrading.

External recorders write 24-bit audio by default. Anything past ~20 MB got
split for upload by code that assumed 16-bit — so the whole transcription
failed. An hour-long meeting never had a chance. Noise reduction had the same
assumption and silently did nothing on those files.

Uploaded audio is now converted to a standard format on the way in. This fixes
the failure and shrinks the upload roughly 9× with no cost to transcript
quality. 24-bit, 32-bit float, stereo, and high sample rates are all handled.

## Meeting notes, with who said what

Record a meeting on your phone or a lav mic, drop the file in, get usable notes.

The upload dialog gains two options:

- **Label speakers** — splits the transcript into "Speaker 1 / Speaker 2" turns.
  Available on Deepgram and ElevenLabs; greyed out with the reason on Groq and
  OpenAI, whose Whisper models have no speaker model at all. The cost difference
  is shown at the point of choice — it's included free on ElevenLabs and billed
  as a per-minute add-on on Deepgram.
- **Meeting notes** — summary, key points, decisions, action items with owners,
  and open questions. With speaker labels on, action items get attributed. With
  them off, it deliberately won't guess who said what.

## The hotkey picker works now

Trying to rebind to F8 or F9 did nothing — because the existing global hotkey
grabbed the keypress and started a *recording* before the picker could see it.
The keys you most wanted to bind were exactly the ones that couldn't be.

Your hotkeys are now paused for as long as the picker is open, the way every app
with a rebind screen does it. Also:

- **Bindings apply instantly.** No "Save hotkeys" button, no restart.
- **Duplicate bindings are caught**, with a message naming what already uses the
  key, instead of silently registering a hotkey that never fires.
