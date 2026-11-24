# wispr-fox v3.4.0-nightly.12

**A measurement instead of another guess.**

## The avatar and multiple desktops

Three builds went into fixing this and the fix was aimed at the wrong thing.
The diagnostic on the affected Mac reported the window settings as exactly
right — the avatar is configured to appear on every desktop and to sit above
full-screen apps — and it still only ever shows up on the desktop wispr-fox
was launched on.

So the window settings were never the problem, and guessing again is not the
way to find out what is. There are two very different faults that look
identical from the outside: macOS not putting the avatar on your current
desktop, or macOS putting it there and drawing nothing. Only one boolean
separates them, and wispr-fox now samples it every two seconds.

**Settings → About** now explains how to use it: swipe to another desktop,
start and stop a dictation there, come back, and run the diagnostic. The
*desktop history* line covers the last two minutes and says which of the two is
happening. Send it over and the next change will be aimed at something real.

One speculative fix ships alongside it, because it costs nothing: when the
avatar is already on screen, showing it now forces the window to re-register
with macOS rather than doing nothing at all. If the fault is the second kind,
that alone may fix it.

## Known and not yet fixed

- With the avatar set to **While dictating**, it appears at launch on macOS and
  stays until the first dictation finishes. It should not appear at all until
  you start talking. Windows is unaffected.
- The window border flickers briefly as the avatar exits.
- Two reports of the app not responding to clicks until it is force-quit.
  Nothing is being changed for this yet — the leading explanation is a guess,
  and this release is about not shipping those.
