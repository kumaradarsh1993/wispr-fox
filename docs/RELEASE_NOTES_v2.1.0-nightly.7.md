# wispr-fox v2.1.0-nightly.7

Every recording now keeps a stopwatch.

## Why was that slow? Now you can see.

If a transcription ever takes longer than it should, the answer used to be
a shrug. This build turns the **(i) details** button on each history card
into a little flight recorder for the run:

- **STT time**, **Cleanup time**, and **Turnaround** (recording-stopped →
  text-pasted) — the actual wall-clock numbers, not guesses. A transcription
  that dragged shows its seconds right there, and anything unusually slow gets
  flagged.
- A **Timeline** of the run underneath: when the audio finalized, when the
  request went out to Deepgram/Groq, how long it took, whether it succeeded
  or failed, and how the text was delivered. Failed and timed-out runs record
  their timeline too, so a bad run explains itself instead of vanishing.

So next time a clip feels slow, open its (i) and you'll see exactly which
stage ate the time — the transcription request, the cleanup pass, or neither.

## A quiet speedup, too

The app used to open a brand-new network connection to the transcription
service for **every single dictation** — a full handshake each time. It now
keeps a warm connection pool and reuses it, so back-to-back dictations skip
that setup cost. It won't fix a slow network, but it trims the overhead that
was there on every request.

## Note

The timeline only exists for recordings made from this build onward — older
cards will say so. Nothing else about your history changes.

*Nightly build. If you hit a slow transcription, the (i) button now has the
receipts — that's the data that tells us where to look next.*
