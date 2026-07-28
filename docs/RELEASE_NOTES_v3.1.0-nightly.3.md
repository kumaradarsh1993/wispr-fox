# wispr-fox v3.1.0-nightly.3 — signing in actually sticks now

Signing in worked. Staying signed in didn't. Close wispr-fox, open it again, and
the Account page would tell you it had never met you — even though nothing had
gone wrong and your session was perfectly valid. On a brand-new machine, signing
in would quietly fetch all your API keys and then still show every model greyed
out with "add key" next to it.

Four separate bugs were doing this, and none of them were in the sign-in itself.
Signing in with Google was working the whole time.

## You stay signed in now

**The real problem: your sign-in was being thrown away on the way to disk.**

When you sign in, the durable part of your session goes into Windows Credential
Manager. wispr-fox asked Windows to store it, Windows said "done", and wispr-fox
believed it — then deleted its own backup copy on the strength of that answer.
Windows doesn't always mean it. That write can be accepted and then never
actually persist, which is exactly why the API-key storage in this app has
always read the value straight back out to check. The sign-in token wasn't doing
that check.

So the session lived happily in memory for as long as the app stayed open, and
the moment you quit, the only copy that mattered was gone. It now reads the
value back and only drops the backup once it's confirmed — and if Windows can't
be trusted on your machine, the backup simply stays.

## "Not signed in" no longer flashes up when you *are*

Restoring a session takes a network round-trip. The window opens faster than
that, so the Account page was asking "am I signed in?", getting "not yet", and
never asking again. You'd see the sign-in form even when the restore was seconds
from succeeding.

The Account page now says **"Restoring your session…"** while that's happening,
and updates itself the moment the answer lands — no need to navigate away and
back.

## New device: your keys arrive *and* the app notices

Sign in on a machine with no API keys and wispr-fox pulls them down from your
other devices. That part worked. What didn't was everything that depends on
them: the model pickers in the sidebar and in Settings → Providers check for
keys once when they open, and nothing told them to look again. So the keys were
there, usable, and every provider still read "add key".

The pickers now refresh the instant keys arrive from another device — including
during first-run setup, which used to ask you to paste a key it had already
received.

## Two silent logouts you may have hit without knowing why

- **Two things refreshing your session at once could log you out entirely.** The
  background sync, a sync after a recording, a delete, and the "Sync now" button
  could each try to renew the same session credential simultaneously. Renewing
  it twice looks like a stolen credential to the server, and the safe response
  is to invalidate everything. Renewals are now queued so only one ever happens.
- **The background sync was starting before the session had loaded**, deciding
  you were signed out, and announcing it. It now waits its turn.

## Transcripts that stopped syncing after a second sign-in

If you ever signed out and back in, every transcript this device had pulled from
your *other* devices got permanently stuck in a state where it ignored all
further updates — including deletions. Delete such a transcript from the laptop
that made it and your desktop would keep its copy forever.

That's fixed, and existing installs repair themselves on first launch. Nothing
is deleted by the repair; it just unsticks the rows.

## And one that had nothing to do with accounts

Sometimes — maybe one time in ten, most often on a work laptop — dictation would
end with **"Copied to clipboard"** instead of pasting, with your cursor still
sitting in the box you'd just dictated into.

wispr-fox checks which app is in front before pasting, so that dictating and
then switching to Chrome doesn't dump text into your address bar. But Windows
briefly reports *nobody* as the front app during ordinary window handovers, and
wispr-fox was reading that silence as "they've moved on". Being told nothing
isn't the same as being told you left. It now asks again, and if Windows still
won't say, it puts your window back in front and pastes normally.

---

**Worth testing, in order:** sign in, fully quit wispr-fox from the tray, and
start it again — you should come back signed in. Then check that the sidebar's
model pickers aren't greyed out. Then dictate into a few apps and watch for a
stray "Copied to clipboard".
