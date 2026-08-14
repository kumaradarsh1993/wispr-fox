# v3.3.0-nightly.3

**Fixes the nightly.2 freeze. If you installed nightly.2, replace it with this build.**

## The headline

On nightly.2, the very first press of F8 (or F9, or Shift+F8) froze the app.
Nothing appeared to happen: no floater, no recording, the tray icon stopped
responding, and the main window would not open. The app never crashed — it just
stopped answering, so the tray icon sat there looking healthy.

That is fixed. Dictation, the floater, the tray menu, and the main window all
respond normally again from the first keypress onward.

## What was actually wrong

nightly.2 rebuilt dictation around a single serialized coordinator, and as part
of that it moved the "arm Escape-to-stop" step inside the serialized section.
That section runs inside the global-shortcut callback — and the shortcut library
runs our callback while it is still holding its own internal lock on the
shortcut registry. Arming Escape asks that same registry whether Escape is
already registered, so the first keypress made the app wait on a lock it was
already holding, on the thread that also drives the tray and every window.

One press was enough to wedge it permanently.

Escape arming now happens off that thread. Ordering is still guaranteed — each
transition stamps a revision, and an arm that arrives late is discarded rather
than re-arming a session that already stopped — so the race that motivated the
original change stays closed.

Nothing else about the adaptive tap-or-hold behaviour changed: release before
700 ms to latch, hold past 700 ms and release to stop and send, and any
dictation key or Escape stops a running session.

## Note on auto-paste, if you have been seeing "Copied to clipboard"

This is not a bug and it is not new, but it surprises people, so: when the app
finishes transcribing, it compares the app you are in *now* against the app you
were in when you started talking. If you switched apps mid-dictation, it
deliberately does **not** steal your focus back — it leaves the text on the
clipboard and tells you so, rather than typing into whatever you moved on to.

If you would rather it always return to the box you started in and paste there,
turn on **Settings → Dictation → "Pull back on navigation"**. Long dictations are
where you notice this most, because they are the ones you are most likely to
wander off during.
