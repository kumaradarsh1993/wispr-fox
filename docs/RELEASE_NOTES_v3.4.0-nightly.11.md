# wispr-fox v3.4.0-nightly.11

**The Accessibility notice stops shouting and stops covering the page.**

## A notice, not a lecture

It was a floating box that sat on top of whatever you were reading, covering
controls, with four buttons and a paragraph of explanation nobody asked for.

Now it is one line with one button. It sits in the page instead of over it, so
nothing is hidden behind it. The explanation moved behind a **Why?** link, for
the once in a hundred times you want it.

It also knows which situation you are in. On a fresh install it says wispr-fox
needs the permission. After an update — where macOS drops a permission you
already gave — it says that instead, and the button says **Repair** rather than
**Grant**. Same fix either way, but the words match what actually happened.

## Still open: the avatar and multiple desktops

Unchanged and not yet solved. **Settings → About → Run diagnostic** reports what
the app can see about it; that readout is the next step, not another guess.

One thing worth ruling out from your side: right-click wispr-fox in the Dock →
**Options → Assign To**. If it says *This Desktop*, macOS is pinning every
window of the app to that one space, and no setting in wispr-fox can override
it.
