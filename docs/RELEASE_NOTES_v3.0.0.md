# wispr-fox v3.0.0

The big one. Since v2.1.0, wispr-fox grew from a single-machine dictation
tool into something that follows you across every device you use it on — and
this build makes that whole line stable.

## Your dictation, on every device

Sign in (optional — Google or email/password) and your transcripts and API
keys travel with you across the desktop app, the phone app, and the web app.
Talk on your laptop, and it's in your history on your phone. Set your keys
once; every signed-in device has them.

**Your audio never leaves the machine it was recorded on.** Only the text
syncs. Signed out, nothing changes — the app works exactly as it always did,
no account, no cloud, bring-your-own-key.

## Transcribe audio files, not just your voice

Drag an audio file onto your history — or use the Upload button — and
wispr-fox runs it through the same transcribe → clean → draft pipeline as
live dictation. Voice memos, call recordings, anything your machine can play.
Pick the provider and cleanup style per batch; uploaded items are badged so
you can tell them apart.

## Delete now means "mine"

Deleting a transcript used to open a little matrix of choices — this device
or everywhere, the audio or the text. That was more decision than anyone
wanted, and across synced devices it had become risky: a "delete all" could
reach out and wipe history you'd recorded somewhere else.

The rule is one line now: **you can delete what this device recorded, and
nothing else.** Deleting a transcript takes its recording with it.
Transcripts that came from your phone or the web app show in your history,
but there's no delete button on them here — they belong to those devices.
"Delete all" clears only what this desktop made.

## Purge — the clean-slate button

That rule left one gap: a transcript recorded on a device you no longer have
— an old phone, a reinstalled machine — could never be deleted, because no
remaining device "owns" it.

**Settings → Account → Purge** closes it. Purge wipes every transcript on
your account, on every device, including ones you no longer have. Each of
your other devices clears its own copy the next time it syncs. It asks twice
— a press-and-hold, then a confirm — because it reaches further than anything
else in the app and can't be undone. Try it deliberately the first time.

## Also in this build

- Settings sections are reordered into one consistent shape shared with the
  phone and web apps — Account sits near the bottom, where account things
  live, with Purge at its foot.
- A quiet fix: deletes made on the desktop weren't always reaching your other
  devices. They are now.
- Synced-history polish — action buttons stay aligned on rows that came from
  other devices, and each row shows a small chip for where it was recorded.

---

*wispr-fox is three apps sharing one backend: this desktop app, the
[Android app](https://github.com/kumaradarsh1993/wispr-fox-android), and the
[web app](https://wispr-fox-web.vercel.app). They all share the delete and
purge behavior described above.*
