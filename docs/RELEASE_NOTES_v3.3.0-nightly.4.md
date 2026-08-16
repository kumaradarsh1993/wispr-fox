# v3.3.0-nightly.4

**Stops the "Mic was very quiet" warning from crying wolf, and stops the floater
from yanking any warning off screen while you are still reading it.**

## The red "Mic was very quiet" box that kept appearing

If you saw this on roughly every other dictation while your transcripts came
back perfectly — that was the app's fault, not your microphone's.

wispr-fox measures every recording's level. When a recording is quiet enough to
be at risk, it automatically boosts a copy to a healthy level *before* sending it
for transcription, which is why the text kept coming back fine. The bug was that
the "too quiet" warning was checked against the level **before** that boost. So
a mic that sits naturally close to the warning line would set off a red alert on
a problem the app had already fixed, several times a day, with nothing for you
to do about it.

The warning is now measured on the audio that speech-to-text actually receives.
In practice that means:

- **Quiet recording, boost worked** — no warning. This is the common case, and
  it was the entire source of the false alarms.
- **Quiet recording, boost could not fully recover it** — you still get the
  warning, and it now tells you the boost was applied and was not enough, which
  is the case where moving the mic closer genuinely matters.
- **Auto-boost turned off** (Settings → Dictation → "Boost audio that came in
  too quiet") — unchanged: you are warned on the real level, as before.

Nothing about the audio itself changed. The recording on disk is still never
modified, and the boost behaviour is exactly what it was.

## Warnings no longer get cut off mid-sentence

The floater hides itself about two seconds after a dictation finishes when
avatar visibility is set to "auto". It was doing that on a timer that ignored
whatever was on screen — so a warning or an error message that had just appeared
would vanish part-read. That is why messages looked oddly truncated.

The floater now waits for the message to finish before it hides. This applies to
every notice, not just the quiet warning, so genuine errors are readable too.

## Not changed, for the record

The two other microphone checks were looked at and are working correctly:

- **Slow mic wake-up** — measured at 0.04–0.5 s on recent recordings, far below
  the threshold that warns. It has not been firing.
- **Mic dropped mid-recording** — fired once in the last thirty recordings, on a
  recording that really did lose about seven seconds of audio. Rare and real,
  which is exactly how it is meant to behave.
