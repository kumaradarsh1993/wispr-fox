# wispr-fox v3.4.0-nightly.10

**Grant Accessibility once. It stops resetting after this.**

macOS only. Windows and Linux are untouched.

## The permission finally sticks

Every wispr-fox update until now silently broke auto-paste on a Mac, and the
reason was not obvious from anywhere in the app: macOS files that permission
against the app's *code signature*, and every build was signed with a fresh
throwaway identity. The old permission entry survived the update, still listed,
still switched on — attached to a build that no longer existed. Turning the
switch off and on again could not fix it, because the entry itself was the
stale thing.

Every build is now signed with one certificate that never changes. So the
permission has something constant to attach to.

**This build resets it one last time.** The identity changed, so macOS sees a
new app. Grant Accessibility once more — the **Repair permission** button added
in the last build does it in two clicks — then quit and reopen wispr-fox. From
the next update onward it stays granted and you should never think about this
again.

If you would rather do it by hand: System Settings → Privacy & Security →
Accessibility → select wispr-fox → **−** → add it again with **+**.

## And CI now checks

A build that came out ad-hoc signed would look completely fine — it installs,
it runs, nothing warns you — and would quietly bring the every-update reset
back. So the macOS build now fails outright unless the finished app reports
`Authority=wispr-fox self-signed`.

## Still not fixed: the avatar following you between desktops

Unchanged from the last build, and still being chased. **Settings → About →
Run diagnostic** reports the two numbers that decide it — `floater level`
should be 25 and `collectionBehavior` should have bits 0 and 8 — along with the
version and the app's location on disk. That readout is what will settle it.

## Unchanged

The Gatekeeper warning on first launch stays. This certificate gives the app a
stable identity on your own machine; it is not an Apple-notarized signature, so
macOS still asks once per install — **System Settings → Privacy & Security →
Open Anyway**. Removing that entirely needs a paid Apple Developer account.
