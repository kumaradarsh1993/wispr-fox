# wispr-fox v3.4.0-nightly.13

**The fox finally follows you across Mac desktops — including fullscreen apps.
And pasting now lands in the box you're actually standing in.**

macOS only. Windows and Linux are untouched.

## The avatar rides along when you switch Spaces

Since the first Mac build, the floater only ever lived on the desktop the app
launched on. Swipe into a fullscreen app and dictate: the recording ran, the
sound cues played — and the fox, with everything it wanted to tell you, was
performing to an empty room three Spaces away.

Three releases of pinning work had already produced the textbook overlay
recipe — window level 25, `CanJoinAllSpaces`, `FullScreenAuxiliary` — and a
diagnostic probe proved the settings were accepted. This release found out why
none of it mattered: **macOS accepts those flags on a regular window and then
ignores them.** `isOnActiveSpace` read `false` after every Space switch, pin or
no pin. The WindowServer only honours join-all-Spaces for a different kind of
window entirely.

So the floater is now that kind of window: a **non-activating panel** owned by
an **agent app** — the same construction every menu-bar overlay utility uses.
The moment it changed class, following started working: regular desktops,
fullscreen apps, fast multi-Space swipes. A workspace observer double-checks
each switch and repairs the rare miss, and the fox does a small hop when it
lands so you know it came with you.

Two things look different on purpose:

- **The Dock icon is gone.** Agent apps don't get one — that's the trade that
  buys fullscreen access. wispr-fox was already tray-first; the menu-bar icon
  and the hotkeys are unchanged.
- **The fox hops** when you arrive on a Space. If your Mac has "Reduce motion"
  on, it skips the hop.

## Paste goes where your cursor is — on every Space

Converting to a panel exposed a second bug within minutes: the panel type is
built for Spotlight-style launchers, so it *grabs keyboard focus* when shown.
Every recording start yanked the caret out of the box you were dictating into,
and the transcript pasted into whatever app had focus before — usually
something on desktop 1. The panel now refuses keyboard focus permanently, the
way the old window did. Your caret stays put; the paste lands where you are.

## The blinking is gone too

The floater used to force a repaint every time its webview "became visible" —
a repair written for dead-after-sleep surfaces. Now that the fox travels, that
event fires on every Space arrival, and the repair (a hide-and-show cycle) was
itself the flicker you saw on each swipe. On macOS that trigger is retired;
sleep recovery still has three other watchdogs. Windows keeps the repaint,
where it heals resume glitches and was never visible in the first place.

## For self-builders

Local `npm run tauri build` on a fresh Mac needs Accessibility permission
re-granted per build (ad-hoc signatures — see nightly.8's notes for the
repair flow).
