# wispr-fox v3.0.0 — Accounts + cross-device sync

The big one. wispr-fox can now carry your transcripts with you — sign in and
they follow you across desktop, web and mobile. Sign-in is completely optional:
signed out, wispr-fox works exactly as it always has, fully local, no account.

## Sign in — Google or email

There's a new **Account** section in Settings, and a new (skippable) step at the
end of onboarding.

- **Continue with Google** opens your browser, you approve once, and you're
  back — no passwords to manage.
- Prefer email? **Create an account** or sign in with an email and password
  right in the app.
- Not ready? **Continue without an account** is right there, front and centre.
  Nothing changes for you.

## Your transcripts, everywhere

Once you're signed in, your transcripts sync in the background across every
device you use wispr-fox on:

- Everything you've already recorded gets pushed up the first time you sign in,
  and new dictations sync as you make them.
- Transcripts made on other devices show up in your History here, each with a
  small **Desktop / Web / Mobile** badge so you can tell where it came from, and
  your device's name on hover.
- **Sync now** and a "last synced" time live in Settings → Account, so you can
  always see what's happening. Sync is quiet and best-effort — if the network
  hiccups it just tries again, never interrupts you, never slows down dictation.

## Your API keys come along too

Set up Deepgram (or Groq, OpenAI, ElevenLabs, Gemini) on your laptop and it's
ready on your other devices — your provider keys sync securely so you don't have
to paste them in again on every machine.

## Voice recordings stay put — always

**Only text syncs. Your audio never leaves the device that recorded it.** This
is deliberate: it keeps the whole thing free and keeps your voice private.
Transcripts synced from another device show up as text you can read, copy and
search — there's just no audio to play back, because it was never uploaded.

## A calmer, clearer delete

Deleting is reworked so you're always in control of exactly what goes:

- The delete control is now **press-and-hold** — hold it for a moment (a little
  fill sweeps to confirm) and a dialog opens. A quick tap just reminds you to
  hold, so nothing disappears by accident.
- The dialog lets you choose **what** to remove — voice files, transcripts, or
  both — and **where**: **this device only**, or **everywhere** (when you're
  signed in). It spells out the consequence in plain words before you confirm.
- **Everywhere** removes the transcript from all your devices. **This device
  only** clears it here and won't let it sync back. Deleting just the voice
  files frees up disk while keeping the text.

## A note on privacy

Local-only mode is unchanged, byte for byte — if you never sign in, wispr-fox
behaves exactly like it did before, storing everything on your machine and
talking only to the transcription provider you chose with your own key. Signing
in adds transcript + key sync on top; your audio is never part of it.

---

*Nightly build — the first of the v3.0.0 line. If this build shows "Sync not
configured", the sync backend isn't wired into this particular build yet;
everything else works as normal and it behaves as signed-out.*
