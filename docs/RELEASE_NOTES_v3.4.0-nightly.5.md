# wispr-fox v3.4.0-nightly.5

**The Mac build stops feeling broken.**

Four things that made wispr-fox awkward on a Mac, all fixed. Windows is
untouched — every change is macOS-only.

## The fox is a fox again, not a white box

The floater was drawing as a plain white rectangle with square corners around
the avatar. The window had been made opaque some builds ago to dodge a macOS
Sequoia bug, and the replacement background that was supposed to come with it
never actually got written — so the page painted nothing and macOS filled the
gap with white.

Real transparency is back. The Sequoia bug it was avoiding no longer
reproduces on current macOS.

## It stays on top now

The floater kept sliding behind whatever you were working in. It was marked
"always on top", but on macOS that only means *on top of the desktop you are
currently looking at* — the moment you went fullscreen or switched desktops,
it was gone.

It now sits in the same layer as menu-bar extras and follows you across
desktops and fullscreen apps. It still yields to open menus, which is
deliberate.

## The tray icon works

Clicking the wispr-fox icon in the menu bar did nothing at all. With silent
launch on and the avatar hidden, that left no way to get the window back
without quitting and relaunching.

macOS does not bring an app forward when you click a menu-bar icon the way it
does for the Dock — the window was being shown behind everything, invisibly.
Now it comes to the front, and clicking again tucks it away.

## Auto-paste explains itself

Dictation on macOS can only reach other apps once you grant **Accessibility**
permission. Nothing in onboarding said so, so the first dictation looked like
it half-worked: the transcript was right, but the text only ever reached the
clipboard.

The "Try it" step now tells you whether the permission is granted, and takes
you straight to the right settings pane. It notices when you come back.

## F8 and F9, like Windows

macOS was defaulting to Option+Space — which is also **Raycast's** default
hotkey, so for a lot of people it silently never worked. New installs now use
**F8** and **F9**, the same keys as Windows.

On a MacBook those are media keys, so either press **Fn+F8**, or turn on
*System Settings → Keyboard → Keyboard Shortcuts… → Function Keys → "Use F1,
F2, etc. keys as standard function keys"* to use them bare. Settings explains
this next to the key binding.

**Already using Option+Space? Nothing changes for you.** Existing installs keep
the hotkeys you already have; only fresh installs pick up the F-keys. Rebind
any time in Settings → Dictation.

## Known, unchanged

- Accessibility still has to be re-granted after each update, because the build
  signature changes every time. `docs/MACOS_SIGNING.md` covers the one-time
  setup that fixes it.
