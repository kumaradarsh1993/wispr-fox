# v3.4.0-nightly.2

**Updates you can actually find, a key to summon the window, and no more
browser menu.**

## About — one screen that answers "what am I on?"

There's a new **Settings → About**. It replaces a "Check for updates" block
that lived at the bottom of App & data, where you had to already know it
existed and scroll to reach it — and which reported only *one* version,
whichever GitHub happened to list first.

About shows all three answers side by side:

- **What you're running**, with a stable/nightly badge.
- **The latest stable**, with its date and a line on what's in it.
- **The latest nightly**, but only when it's actually newer than the stable —
  a nightly that a stable has already overtaken isn't an upgrade, and offering
  it would just move you backwards.

## Install without going to GitHub

Press **Install** and wispr-fox downloads the right installer for your machine,
shows a real progress bar, and runs it. No release page, no picking the right
file out of six, no wondering which one is current.

- **Windows:** installs in place. The app closes so the installer can replace
  files it's using, then reopens.
- **macOS:** downloads and opens the .dmg. The builds aren't code-signed, so
  dragging to Applications stays a manual step — the screen says so rather than
  pretending otherwise.

Downloads come only from this project's own GitHub releases over HTTPS, and the
finished file has to match the size GitHub declared before anything is launched
— a truncated download gets deleted instead of run.

You can move between stable and nightly freely, in either direction. Your
history, settings and API keys are untouched by an update.

## Summon the window from anywhere

**Win+F8** shows the main window, and hides it again. No more going to the
system tray and double-clicking the icon.

It's a proper toggle: if the window is open but buried behind something else,
the key **raises** it rather than hiding it. (A naive show/hide reads as broken
there — press once, nothing seems to happen; press again, it opens.) It never
starts a recording, and it's rebindable in Settings → Voice.

**On macOS it's Cmd+Shift+Space.** The Mac function row sends media keys, so an
F-key was out for the same reason F8/F9 were dropped there. Everything obvious
is already taken: Cmd+Space is Spotlight, Ctrl+Space switches input source,
Cmd+Option+Space is Finder search, Ctrl+Cmd+Space is the emoji picker, and
Option+Space is already this app's dictate key (and Alfred's trigger).
Cmd+Shift+Space is free, and isn't bound in Word, Chrome, WhatsApp or Claude.

Worth knowing: a global shortcut beats whatever app you're in, so a bad default
doesn't just fail quietly — it takes the key away from that app. That's why the
default is cautious, and why every binding here is rebindable.

## Right-click behaves like an app, not a web page

Right-clicking anywhere used to bring up the webview's own menu — Back, Reload,
Save as, Print, Inspect. Those are browser commands showing through, and "Save
as…" on a transcript card doesn't mean anything.

Now right-click gives you a menu only where there's something real to offer:

- **In a text box** — Cut, Copy, Paste, Select all, with only the ones that
  apply at that moment.
- **Over selected text** — Copy.
- **Anywhere else** — nothing at all. A menu of greyed-out items is worse than
  no menu.

The floater keeps its own right-click menu (skin, size, position) unchanged.

## Validation

- Svelte diagnostics: 330 files, 0 errors, 0 warnings.
- Rust: `cargo check` clean.
- The About screen, both release cards, the new hotkey row, and every branch of
  the right-click menu were driven and measured in a live browser. Paste was
  verified to fire the input event that the text box actually listens to, and
  the menu was checked to stay inside the window when opened hard against the
  bottom-right corner.
- Updater logic: 4/4 unit tests, including the download host allowlist rejecting
  lookalike domains. They run in CI; also verified in an isolated harness built
  from the shipped source, because the app crate's test binary can't launch on
  the dev machine.
