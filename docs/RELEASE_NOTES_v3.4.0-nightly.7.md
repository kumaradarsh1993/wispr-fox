# wispr-fox v3.4.0-nightly.7

**The avatar shows up where you actually are.**

macOS only. Windows and Linux are untouched.

## The fox appears on the desktop you're working on

If you use full-screen apps and swipe between them, the avatar was almost
certainly appearing on the wrong one — pinned to whichever desktop wispr-fox
happened to be opened on, no matter which desktop you were dictating into. It
was still recording; you just had no way to see it.

It now follows you. Start a dictation from any desktop or full-screen app and
the avatar appears there, and it stays with you if you swipe to another one
mid-dictation. There is no setting to turn on.

### What was actually wrong

Two builds ago the floater was taught to sit above full-screen apps and join
every desktop. That worked — for the first 30 seconds of each launch.

A background safety net re-asserts "stay on top" every 30 seconds, to recover
the floater after the machine sleeps or the graphics stack resets. On macOS
that particular call does something narrower than its name suggests: it
rewrites the window's layer from scratch, which threw away the desktop pinning
the fix had installed. So the avatar was correctly pinned at launch, quietly
unpinned half a minute later, and stayed that way for the rest of the session
— which is why it looked like the fix had never shipped at all.

The safety net now re-applies the full pin rather than the partial one, and the
pin is re-applied at the moment the avatar is shown, so it lands on the desktop
you are on right then. Both paths are now covered by a check that fails the
build if anyone reintroduces the narrower call.

The avatar also no longer clutters Mission Control or Cmd-` window cycling —
it is an overlay, not a window you switch to.
