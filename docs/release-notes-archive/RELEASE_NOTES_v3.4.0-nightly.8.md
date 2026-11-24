# wispr-fox v3.4.0-nightly.8

**Auto-paste on macOS was throwing your text away silently. And the app can now
tell you what it is instead of you having to guess.**

macOS only. Windows and Linux are untouched.

## Dictated text no longer vanishes

On a Mac without Accessibility permission, wispr-fox typed your transcript into
nothing at all — no paste, no clipboard, no error, no mention in the history
row. It looked like transcription had failed.

macOS accepts synthesized keystrokes from an app that lacks permission and then
quietly discards them. No error comes back, so the code believed it had typed
your text successfully every time. The clipboard fallback that was supposed to
cover this case had never actually been written, and could not have worked
anyway: pressing ⌘V is *also* a synthesized keystroke, dropped by the same rule.

wispr-fox now checks the permission before it tries. Without it, your text goes
to the clipboard and the fox tells you to press ⌘V — you keep the words either
way.

## "Accessibility is switched on and it still doesn't work"

This is the confusing one, and it is not you.

macOS files the permission against the app's *signature*, not its name. These
builds are ad-hoc signed, so the signature changes with every release. After an
update, the old entry stays in the list, still switched on, still saying
"wispr-fox" — while pointing at a build that no longer exists. Turning the
switch off and on again does not rebind it, because the entry itself is stale.

The banner now says this, and carries a **Repair permission** button that
removes the dead entry and asks macOS afresh. After approving, **quit and
reopen wispr-fox** — a permission only reaches an app when it starts.

You can also do it by hand: System Settings → Privacy & Security →
Accessibility → select wispr-fox → **−** → then add it again with **+**.

The permanent fix is signing every build with one unchanging certificate, so
you grant this once and never again. That is set up separately.

## Settings → About → Run diagnostic

A short readout of what you are actually running: the version, where the app
lives on disk, whether macOS really trusts it, and the two numbers that decide
whether the avatar can follow you between desktops. **Copy** puts it on the
clipboard.

This exists because most of wispr-fox is written on a Windows machine, where
none of the above can be observed. Twice now a macOS bug has been diagnosed by
reasoning about it from a distance, and once that reasoning was wrong. A number
read off the running app settles it.

## The avatar following you between desktops

Still being chased, and honestly reported: last build's fix was real but has not
solved it on the reporter's Mac.

Two changes here. The floater is now re-pinned at the moment a dictation
starts — in every avatar mode, not only "While dictating" — and it is pinned
*before* the window is placed rather than after, because the pinning is what
decides which desktop it gets placed on.

Two window flags added last build have been removed. They were tidiness — keep
the avatar out of Mission Control and ⌘\` cycling — never tested on a Mac, and
in a feature that is already misbehaving an untested change only makes the
misbehaviour harder to attribute. What remains is exactly the flag combination
every macOS overlay uses.

If it is still landing on the wrong desktop, **Settings → About → Run
diagnostic** now answers why: `floater level` should read 25 and
`collectionBehavior` should have bits 0 and 8 set. If it does and the avatar is
still stranded, the cause is something other than the pinning and that readout
is what will find it.
