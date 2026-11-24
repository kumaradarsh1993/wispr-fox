# wispr-fox v3.0.0-nightly.2 — Sync actually syncs now

A focused fix for the sync that shipped in nightly.1.

## "Sync paused — will retry" is fixed

If you signed in on nightly.1 and saw **"Sync paused — will retry"** sitting
under your account (even right after a clean sign-in), that's fixed. Sign-in
itself was fine — the very first step of the sync, registering your device with
the cloud, was being rejected, so the whole cycle stopped and quietly kept
retrying against the same wall.

Sign in again (or hit **Sync now** in Settings → Account) and it should move to
**Syncing…** and then settle on a normal "last synced" time. Your existing
transcripts push up on that first successful sync, and anything from your other
devices pulls down.

## API keys now sync from desktop too

The same underlying issue was quietly blocking your API keys from syncing up
from the desktop, so set up a provider here and it's now available on your web
and mobile apps as well — no re-pasting.

Nothing else changed — local-only mode, dictation, and everything from nightly.1
behave exactly as before.

---

*Nightly build — v3.0.0 line. Signed out, wispr-fox works fully local as always.*
