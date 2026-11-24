# wispr-fox v3.0.0-nightly.3

More work on the accounts + cross-device line — this build is about **delete
that behaves the same everywhere**, and a way to wipe the slate clean.

## Delete now means "mine"

Deleting a transcript used to offer a little matrix of choices — this device
or everywhere, the audio or the text. That was more decision than anyone
wanted, and on the web it had become genuinely dangerous: a "delete all" could
reach across and wipe history you'd recorded on another machine.

So the rule is one line now: **you can delete what this device recorded, and
nothing else.** Deleting a transcript takes its recording with it. Transcripts
that came from your phone or the web app show up in your history, but there's
no delete button on them here — they belong to those devices. "Delete all"
clears only what this desktop made.

## Purge — the clean-slate button

There was a gap in that rule: a transcript recorded on a device you no longer
have — an old phone, a reinstalled machine — could never be deleted by anyone,
because no remaining device "owns" it.

**Settings → Account → Purge** closes that gap. It wipes every transcript on
your account, on every device, including ones you no longer have. Each of your
other devices clears its own copy the next time it syncs. It asks twice — a
press-and-hold, then a confirm — because it reaches further than anything else
in the app and can't be undone.

## Also in this build

- A quiet but real fix: deletes made on the desktop weren't always reaching
  your other devices. They are now.
- Settings sections are reordered into a consistent shape shared with the web
  and phone apps — Account sits near the bottom, where account things live.
- The synced-history polish from the last couple of pushes (aligned action
  buttons, source chips beside the version tabs) rides along here.

---

*A note on testing: the account and sync paths in this build are checked by the
compiler and the type system, but this is a nightly — the delete and purge
round-trips haven't been exercised against a live account yet. Purge especially
is worth a careful first try, since it's designed to reach every device.*
