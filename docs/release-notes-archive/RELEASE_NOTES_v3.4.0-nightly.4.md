# wispr-fox v3.4.0-nightly.4

**The Mac build opens now.**

## "wispr-fox is damaged" is fixed

Every macOS build before this one was rejected on launch with *"wispr-fox is
damaged and can't be opened. You should move it to the Trash."* Nothing was
actually damaged — the download was fine and the disk image was fine.

macOS will not run an app unless the app bundle is sealed with a code
signature. Our builds sealed the program inside the app but never the app
itself, and macOS reads that mismatch as a file that was tampered with in
transit. That is a hard stop: unlike the ordinary "unidentified developer"
warning, it offers no way through. Right-click → Open did not help, because
Apple removed that bypass in macOS 15 (Sequoia).

The build is now signed properly, so macOS treats it like any other app from a
developer outside the App Store.

Windows was never affected — it has no equivalent check, which is why this went
unnoticed for so long.

## What you'll see on first launch

The app is signed but not notarized, so Gatekeeper still stops it **once**:

1. Drag wispr-fox to Applications and double-click it.
2. macOS says it cannot verify the developer — click **Done**.
3. Open **System Settings → Privacy & Security**, scroll to the Security
   section, and click **Open Anyway** next to wispr-fox.

It launches normally from then on. If you prefer one line in Terminal:

```
xattr -dr com.apple.quarantine /Applications/wispr-fox.app
```

Removing that prompt entirely requires Apple notarization, which needs a paid
Apple Developer account. That is a separate decision, not a bug.

## Known, unchanged

- **Accessibility still has to be re-granted after each update on macOS.** The
  signature identity changes on every build, and macOS ties the permission to
  that identity. `docs/MACOS_SIGNING.md` describes the one-time setup that
  fixes it.
- Microphone behaviour is deliberately untouched. The signing change was made
  in the one configuration that adds no new runtime restrictions, specifically
  so dictation kept working.
- macOS and Linux still download and open a build rather than installing it.

## Also in this build

- The release pipeline now refuses to publish a macOS build whose bundle is
  unsigned, so this cannot silently come back.
- `CLAUDE.md` records the constraint for future work, since development happens
  mostly on Windows where nothing catches it.
