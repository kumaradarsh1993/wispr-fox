# wispr-fox v3.4.0-nightly.6

**Rerun no longer freezes the app.**

## The freeze

Pressing **Rerun** on a recording that already had a transcript locked up
wispr-fox completely — the confirmation box appeared and then nothing
responded, and the only way out was force-quitting.

The confirmation box itself was the problem. It was a browser-native popup,
and inside a desktop app window that kind of popup blocks the very thread the
app needs to talk to its own backend. Asking the question deadlocked the
answer.

It has been replaced everywhere with a proper OS dialog. Six other places
could have hit the same freeze — Retry, re-running cleanup or a draft,
resetting a prompt, deleting an API key — and all of them are fixed too.

This is also why uploading a file always worked while Rerun did not: the
upload flow never asks you to confirm anything.

## Meeting notes that came back as the raw transcript

When the AI step failed, wispr-fox quietly handed back your raw transcript
instead. The reason was recorded, but only somewhere you had to go looking
for it — so the feature looked like it simply did nothing.

Two changes:

- **It now says so.** The Cleaned / Draft / Meeting notes tabs show a clear
  message when what you are reading is the raw transcript, and why — the model
  timed out, the key was rejected, the provider was rate-limited.
- **Long recordings get the time they need.** The deadline was a flat 90
  seconds, which a 40-minute call blows straight through. It now scales with
  the length of the transcript, up to six minutes. Short dictations still fail
  fast, because there a paste is waiting on the result.

## Unchanged

Windows behaviour is untouched — both fixes are platform-neutral.
