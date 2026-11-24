# wispr-fox v3.4.0-nightly.3

**Updating is now one click, and this time it means it.**

## The Install button actually installs

Settings → About has offered to update wispr-fox for a while, and the text under
it said the app would close, update and reopen. In practice the Windows
installer opened its wizard and waited for you to click Next.

It no longer does. Install now downloads the build, runs the installer silently,
and brings wispr-fox back on the new version. Nothing to click through, nothing
to uninstall first, and your history, settings and API keys are untouched.

## The same update screen in all four apps

wispr-fox, FoxCull, Fox MD and Fox Mark now share one update module rather than
four that drifted apart — same two-channel view, same silent install, same
behaviour when something goes wrong. FoxCull and Fox Mark had no in-app updates
at all before this.

Practically: whatever you learn about updating one of them is true of the others.

## Smaller things

- **The About tab wears a dot when a build is waiting.** One quiet check when
  the app starts. It stays quiet if GitHub can't be reached — an alert that
  fires because you were offline is an alert you learn to ignore.
- **A download that gets cut off is thrown away rather than run.** The finished
  file has to match the size GitHub declared before anything is launched.
- **The progress bar reports real bytes**, and the download streams to disk
  instead of being held in memory twice.

## Known, unchanged

macOS and Linux still download and open the build rather than installing it —
these are unsigned builds, so the last step stays yours.
