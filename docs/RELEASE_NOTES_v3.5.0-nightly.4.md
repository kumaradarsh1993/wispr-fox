# wispr-fox v3.5.0-nightly.4

**Your sign-in survives a restart now — and your API keys are finally in the
OS keychain, which the app has been claiming for months without doing it.**

## The bug behind both

wispr-fox asks for one Rust library, `keyring`, to talk to the macOS Keychain,
Windows Credential Manager and the Linux Secret Service. Version 3 of that
library made **every platform backend an opt-in build flag**, and the app was
asking for `keyring = "3"` with no flags. That compiles cleanly. It links no
backend at all and silently substitutes a **mock store that lives inside the
running process**.

Everything follows from that:

- **Sign-in never persisted.** Saving the refresh token "succeeded", and the
  app's own read-it-back safety check passed — because the mock store answers
  from memory inside the same process. Having verified the write, the app
  deleted its encrypted fallback copy. On the next launch the mock store was
  empty and the token was gone. You signed in, it worked all session, and you
  were signed out again the next morning. Every time.
- **API keys were never in the keychain.** They kept working because
  `secrets.rs` falls back to an encrypted file in the app's data folder, and
  it retried the keychain on *every read* — this machine's audit log showed
  **over 12,000 failed keychain writes**. Those retries are also part of why
  opening a dialog used to stall (fixed separately in v3.4.0).
- **The README's privacy claim was wrong.** "API keys live in your OS keychain"
  was not true on any platform. It is now.

The fix is three lines of build configuration: `apple-native` on macOS,
`windows-native` on Windows, `sync-secret-service` on Linux. A comment now sits
where the old dependency was so nobody re-adds it without a backend.

**What you need to do:** sign in once more after updating, and re-enter your
API keys if the app asks. This time they land in the real keychain and stay
there. (Keys currently in the encrypted fallback keep working and migrate on
first read.)

## The window that opened "full screen"

The macOS overlay titlebar added in nightly.1 came with a drag strip at the top
of the sidebar. Tauri maps a **double-click on a drag region to maximize** — so
one stray double-click near the top of the pane maximized the window, and with
no title bar to frame it, every later launch looked like the app opened full
screen. The strip now drags without the zoom behaviour, and showing the window
drops it out of a maximized state left behind by an earlier build.

## Two copies in Spotlight

Nothing to fix in the app: stale local release builds were sitting in
`src-tauri/target/release/bundle/macos/`, and Spotlight shows a bundle's parent
folder as its subtitle — hence "wispr-fox — macos" next to the real
"wispr-fox — Applications". Those build outputs have been removed on this
machine and the build folders are now marked `.metadata_never_index` so future
dev builds never show up in search again. Same for Fox MD.
