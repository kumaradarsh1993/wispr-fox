# wispr-fox v3.5.0-nightly.7

**Fixes the Keychain password prompt after every narration. This one is my
fault — it was caused by the keychain fix in nightly.4.**

## What went wrong

Before nightly.4, wispr-fox asked for one Rust library to talk to the macOS
Keychain but compiled it with **no backend**, so every read and write silently
hit a fake in-memory store. nightly.4 wired up the real Keychain — which is what
made your sign-in finally persist.

What that also did was make every read a **real** Keychain read. And
`secrets::get` had no cache: it walked all the way to the OS store on every
single call. A dictation resolves at least two keys — the transcription key, then
the cleanup key — so every narration performed two live Keychain reads. If the
stored item's access list does not already trust this exact app, macOS raises an
authorization prompt for each one.

Two reads, two prompts, on every narration. Exactly what you saw.

None of this was visible before nightly.4 because the fake store never asked
anyone's permission.

## The fix

Secrets are now resolved **once per key per app launch** and held in memory for
the rest of the session. The dictation path no longer touches the Keychain at
all after the first time.

- A *miss* is cached too, so an unconfigured provider does not keep re-asking.
- Saving or deleting a key updates the cache immediately, so a key you just
  typed into Settings is never shadowed by a stale "not configured".
- `invalidate_cache()` exists for sign-out and cross-device sync, where the
  trust story genuinely changes underneath us.

Holding the keys in memory is not a new exposure — the app already holds them in
cleartext while building provider clients and signing requests. What changed is
how often it asks the operating system for them.

Six tests cover the cache, including the dangerous case: a cached "no such key"
must never hide a key you just saved.

## What you should still do once

The prompt can still appear **once after a launch**, because the Keychain items
on your machine were created by an older build whose signing identity no longer
matches. When it appears, click **Always Allow**, not Allow. "Allow" grants
permission for that single read; "Always Allow" writes your app into the item's
access list.

wispr-fox now has a stable signing identity (`Developer ID Application:
wispr-fox self-signed`), so once the access list is written it should stay quiet
across future updates too.

If it somehow keeps asking after that, re-entering your API keys in
Settings → Providers & Keys will re-create the items under the current identity
and settle it permanently — the app becomes the item's owner when it writes it.

## Also in this build

Everything from nightly.6, which was still building when this went out:
pause/resume mid-dictation welded into a single upload, and the calmer Home
cards. See `docs/RELEASE_NOTES_v3.5.0-nightly.6.md`.
