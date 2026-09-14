# wispr-fox v3.4.0

**The Mac release.** Fifteen nightlies, two months of daily use, and wispr-fox
on macOS now behaves like an app that was built for it — while Windows gets
faster transcription, one-click updates, and dialogs that stop freezing.

This is the stable roll-up of `v3.4.0-nightly.1` through `nightly.15`. If you
are on any nightly in that range, nothing here is new to you; update anyway so
the app stops reporting itself as a pre-release.

## If you use a Mac

**It opens.** The build is now signed with a single stable identity, so
Gatekeeper's "wispr-fox is damaged and can't be opened" is gone. It also means
the Accessibility permission you grant **stays granted across updates** —
previously every nightly wore a fresh signature and macOS quietly revoked it.

**The fox follows you.** The floating avatar appears on whichever desktop
(Space) you are on, including fullscreen apps, and stays there. It used to be
pinned to one desktop and un-pinned itself every 30 seconds.

**Pasting lands where you are standing.** Dictated text goes into the text box
you were actually in when you pressed the hotkey — not the app the fox happened
to be sitting on. And when pasting *cannot* work (Accessibility missing), the
app tells you in a small notice rather than throwing your words away silently.

**The Mac defaults make sense.** Option+Space to transcribe, Option+Enter to
draft — because F8 on a Mac plays music. The tray icon works, the floater is
transparent, and the Dock icon is gone in favour of the menu bar.

**Transcription is 3–6x faster on long dictations.** Mac microphones record at
48 kHz — three times the data any speech engine actually uses. Recordings are
now shrunk to 16 kHz before upload while the saved original stays full quality.
A 21-minute narration that spent 49 seconds uploading now spends about 15.

## On every platform

**Updates you can find and actually install.** Settings → About shows what
you are on, what is available on both the stable and nightly channels, and an
**Install** button. On Windows it downloads, installs silently and relaunches;
on macOS and Linux it downloads and opens the installer. The About entry
carries a dot when an update is waiting.

**Insights merges across your devices.** Sign in on two machines and Insights
shows one set of totals and one "since" date instead of two disagreeing ones.
History cards say which device produced them, and Settings → Account lists
every device with a name and icon you can set.

**A key to summon the window.** Set a show/hide hotkey in Settings →
Dictation so the main window is one keypress away. Right-clicking anywhere
gives you a proper app menu instead of a browser one.

**The Upload and Rerun dialogs stopped fighting you.** Opening either one no
longer freezes the entire app (seven keychain reads were blocking the UI
thread). Rerun runs when you click Run — the confirmation that used to open
invisibly behind other windows now lives inside the dialog. You can rerun a
speaker-labelled meeting through plain Whisper. "Working…" ends, with a 180
second ceiling on stuck requests, and the upload dialog shows each stage live:
*Transcribing… → Cleaning up… → Drafting… → Writing meeting notes…*

**AI failures are no longer invisible.** When cleanup or drafting fails, the
recording says so and offers a retry, instead of quietly producing nothing.

## Upgrading

Download the installer for your platform from this page, or — if you are on
v3.4.0-nightly.3 or later — open Settings → About and click **Install**.

Mac users on the very first launch of a signed build: if pasting stops working,
go to System Settings → Privacy & Security → Accessibility, remove the stale
wispr-fox entry, add `/Applications/wispr-fox.app`, and restart the app. This
is a one-time step; the signature is stable from here on.
