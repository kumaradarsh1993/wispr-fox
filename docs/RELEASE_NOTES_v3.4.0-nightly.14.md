# wispr-fox v3.4.0-nightly.14

**Transcription on Macs is now 3-6x faster. The words were never slow — the
upload was.**

All platforms benefit, Macs most. No visual changes.

## Long dictations no longer crawl

A user with a brand-new M-series MacBook and a decade-old Alienware noticed the
old machine transcribing in 2 seconds what the new one took 8-50 seconds to do.
Same account, same model, same network. The telemetry pointed away from every
usual suspect: no retries, no errors, a healthy 171 Mbps uplink, and Groq
itself answering fast.

The culprit was the audio file. Live dictations upload the microphone's NATIVE
recording rate, and Mac mics capture at 48 kHz — 96 KB for every second of
speech, 3x what most Windows array mics produce. Every speech-to-text provider
immediately downsamples to 16 kHz on arrival, so the extra bytes bought
nothing; they just took 3x longer to send. Past ~3.5 minutes it compounded:
the file crossed the 25 MB request limit and was cut into chunks that uploaded
one after another, each paying its own round-trip. A 21-minute narration
became 6 sequential uploads — 49 seconds of pure transfer for ~2 seconds of
actual transcription. The math fit the user's history almost perfectly: the
whole wait was upload.

The odd part is that the app already knew all this. Files dragged into the
History window have been shrunk to 16 kHz before upload since the meeting
features landed — the code comments even record the "~9x smaller at no cost to
the transcript" measurement. Live dictations, the thing the app exists for,
never got the same treatment.

Now they do. Before upload, a lightweight 16 kHz copy is made and sent; the
recording saved in History remains the full-rate original. Uploads shrink 3x
on 48 kHz mics, and recordings stay in one piece (no chunking) up to ~10.5
minutes. Mics already at 16 kHz are left alone — there is nothing to gain.

## For the curious

Speed still stacks with the model choice: `whisper-large-v3-turbo` (Settings →
Models) remains ~8x faster server-side than the full `whisper-large-v3`, and
Deepgram Nova-3 remains the recommended engine overall. This release fixes the
transfer; those fix the transcribing.
