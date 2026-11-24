# v3.3.0-nightly.5

**Stops the false "mic dropped mid-recording" alarm, and makes error messages
readable instead of a 3-words-per-line ribbon.**

## The "mic dropped mid-recording" warning was wrong

If you saw a red box saying only ~336s of ~339s was captured and that you should
record again — nothing was lost that you would ever notice, and there was
nothing to record again.

Windows occasionally sheds a capture buffer while recording. Each one is on the
order of ten milliseconds: inaudible, invisible in the transcript, and spread
thinly across a long recording rather than landing in one place. The recording
that triggered this warning was checked frame by frame afterwards — no silence
anywhere in it, no cut, and a clean transcript from the first word to the last.
It lost 1% of its buffers over five and a half minutes.

The old check flagged any shortfall over one second, which on a long dictation
is a rounding error. It now has to be both over a second **and** a meaningful
share of the recording, so a genuine drop — a mic unplugged, taken over by
another app, or a driver reset — still tells you, while ordinary buffer churn
stays quiet. A reported stream fault from the audio device is always shown,
because that is a real signal rather than an inference.

The wording is fixed too. The old message told you to re-record to "get the
rest", which was never possible; audio that was never captured cannot be
recovered by retrying.

## Error messages are no longer squeezed into a ribbon

The floater's message bubble is deliberately narrow during dictation, so status
text sits neatly above the avatar. Errors were being forced through that same
narrow bubble — a full sentence wrapped to two or three words per line, ran
about ten lines, and *still* needed scrolling.

Errors now get their own, much wider bubble with room to breathe, so a message
reads as a few normal lines instead of a scrolling column. This only applies
while an error is actually showing; the floater at rest and during dictation is
exactly as before.

Together with the previous build's fix — the floater no longer hiding itself
while a message is still on screen — messages should now be both readable and
readable *for long enough*.
