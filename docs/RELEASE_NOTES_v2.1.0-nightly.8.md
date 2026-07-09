# wispr-fox v2.1.0-nightly.8

Catching the "3 minutes in, half a transcript out" bug.

## The likely culprit behind a truncated transcript

If you've ever recorded for a few minutes and got back only the first
30 seconds — this build is about that.

The recording timer measures how long you *held the key*. It does **not**
prove that much audio actually reached the file. If the microphone stream
hiccups partway through — Windows switching the device, a Bluetooth headset
changing modes, an audio "enhancement" kicking in — the app used to keep the
timer running while quietly capturing nothing. You'd get a card that says
"3m 0s" wrapped around 30 seconds of audio, and a transcript cut short with
no explanation. Retrying just re-read the same short file, so it never helped.

Now the app **measures the audio it actually captured** and compares it to the
timer. When they don't match, you get told plainly:

> **Mic dropped mid-recording.** Only ~30s of your 3m 0s recording was
> captured (~150s lost), so this transcript is cut short. Re-record to get
> the rest — retrying won't recover audio that wasn't captured.

No more silent partial results dressed up as success.

## The (i) panel now triangulates

Open any card's (i) and the timeline shows three numbers that pinpoint where
audio went missing, if it did:

- **recorded** — how long the timer ran.
- **captured** — how much audio actually made it into the file.
- **provider processed** — how many seconds the transcription service saw.

If *recorded* and *captured* disagree, your mic dropped (a re-record fixes it).
If they agree but *processed* is short, the problem is upload/transport, not
your mic. Either way you can now see which, instead of guessing.

## Cleanup no longer gives up early

The polish/Draft step had a tight time limit that a longer dictation could
blow past, dropping you back to the raw transcript with a "cleanup timed out"
note even when nothing was really wrong. That limit is now generous enough to
finish a long draft.

*Nightly build. If a transcription comes back short, open its (i) — the
recorded-vs-captured numbers will tell us exactly what happened.*
