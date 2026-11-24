# wispr-fox v3.5.0-nightly.2

**Home, part one: the sidebar becomes a rail, the header becomes one line, and
what you're about to dictate with is one glance away.**

Second nightly of the v3.5 redesign (`docs/DESIGN_v3.5.md`). This one is
visible. The recording cards themselves are unchanged — they are nightly.3.

## The sidebar is a rail now

It used to do seven jobs: navigation, a hotkey card, "Quick controls" with an
engine label, a polish toggle, a microphone picker and an avatar picker, a
usage meter with a UTC countdown, a "Replay onboarding" link, and a decorative
fox. It now does two: **Home · Insights · Settings**, and an **account chip**
at the bottom that shows who you are (or "Sign in") and takes you to sync.

Nothing was deleted, everything moved to where it is used:

- Engine, polish and avatar visibility → the new **status strip** on Home.
- Microphone → Settings › Microphone (it was always there too).
- Today's quota → a card at the top of **Insights**.
- Replay onboarding → Settings › About (next nightly).
- The version number → Settings › About, where it already was.

## One header row on Home

**Home** · a wide search box (**⌘K** / **Ctrl+K** jumps to it from anywhere
on the page) · **Upload** · folder · reload. The kicker, subtitle, count chip,
"View insights" link and peeking fox are gone. So is **Clear all** — a
press-and-hold delete-everything button does not belong next to your notes.
It lives in **Settings › App & data › Danger zone** now, same dialog, same
"this device / everywhere" choice.

## The status strip

Directly under the header, one quiet line:

> Listening with **Nova-3** · Polish **On** · Fox **While dictating** · Dictate ⌥ Space

Each piece is live and clickable: the engine link opens engine settings, the
polish switch toggles cleanup for the next dictation, the fox opens a small
menu for Always / While dictating / Hidden (and "Change avatar…"), and the
shortcut opens shortcut settings. This is the advanced user's fast path; a
beginner can ignore it and it still tells them what will happen.

## Filters that say what you mean

**All · Dictations · Drafts · Meetings · Uploads · Failed.** Two of those are
new — meetings and uploads had no filter before — and one internal mode that
had a filter type but no button is gone.

## Housekeeping

A stray draft release (`v3.4.0-nightly.9`, from a failed run weeks ago) was
deleted. It was never visible to users or to the in-app updater.

## Check on your machine

- Collapse the sidebar (click the fox): the rail should show three icons and
  your initial, nothing clipped.
- Press ⌘K / Ctrl+K on Home: the search box should focus.
- Toggle Polish in the strip, dictate: the next result should honour it.
