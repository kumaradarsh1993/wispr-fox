# wispr-fox v2.1.0-nightly.1

A big spring-clean plus two new ways to keep Foxy out of your way. Built from
the July 2026 UX audit — same app, calmer and more consistent.

## New: choose when the avatar appears

The avatar now has its own visibility setting, separate from which character
you pick:

- **Always show** — the classic behaviour.
- **While dictating** — the avatar stays hidden and pops in with a warm little
  entrance the moment you start dictating, then tucks itself away a couple of
  seconds after your text lands. Dictation works exactly the same while it's
  hidden.
- **Hidden** — out of sight until you want it back.

Switch it from the sidebar, Settings → Appearance, or the avatar's right-click
menu. (If you previously set the avatar to "Off", you'll find that's now the
Hidden setting — your character choice is remembered separately.)

## New: Wave bar avatar

A minimal alternative to the characters: a small translucent pill with a live
waveform that dances to your actual voice while you record. No speech bubbles,
no text — just a quiet signal that wispr-fox is listening. It sits top-center
of your screen by default, and you can drag it anywhere. Every avatar's
right-click menu now has **Reset position** to send it back home.

## Fixed

- The app no longer claims "Update available — v2.0.0" when you're already on
  v2.0.0. (An internal version number was left behind during the last release.)
- API keys now move themselves into Windows Credential Manager / the macOS
  Keychain automatically once it's available, instead of staying in the
  fallback file forever. The Security page explains what's stored where.
- Draft mode can no longer be silently broken by an obscure checkbox — the
  toggle that made F9 return raw text instead of a drafted message is gone.
- Usage meters are honest now: Deepgram shows lifetime credit used (not
  pretending to be a daily number), and providers without known free-tier
  limits no longer show a made-up percentage bar.

## Cleaner everywhere

- **One vocabulary.** The two modes are called **Transcribe** (F8) and
  **Draft** (F9) everywhere — onboarding, sidebar, history, settings. Raw /
  Cleaned / Drafted now only refer to the three text versions of a recording.
- **Roomier sidebar.** Slightly wider by default with more breathing room —
  the quick model/avatar controls stay right where they were.
- **Quieter History.** Row buttons appear when you hover instead of crowding
  every row, and the page has less orange shouting for attention. "Clear all"
  is now a subtle button (still needs the 3-second hold, so no accidents).
- **Settings, on a diet.** Shorter descriptions, clearer section names
  (Avatar → Appearance), proper icons in the settings nav, and stale
  cross-references fixed.
- Onboarding says the right thing on Macs now (Keychain, not Windows
  Credential Manager) and is honest that free quotas reset at midnight UTC.

*Nightly build — try it and tell the fox what feels off.*
