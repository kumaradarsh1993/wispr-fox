# v0.2.0 — Reliability, Cream Clippy, and a real history page

The first stable release of wispr-fox. Big batch of UX work, plus
fixes for the things that made earlier builds frustrating.

---

## 🚀 What's new

### Reliable, even when things go wrong

- **No more "stuck Clippy."** Every failure now ends cleanly: history
  marks the recording as errored, Clippy goes back to idle, and you
  get a clear toast like "Network issue", "API key rejected", or
  "Rate limit — wait a minute" instead of an endless spinner.
- **Long recordings work.** Files over ~3 minutes used to fail with
  "file too large." Now they're auto-split, transcribed in parts,
  and stitched back into one clean transcript. Dictate as long as
  you want.
- **Tougher hotkey behavior in Outlook + Teams.** Earlier builds
  sometimes pasted into the wrong place after F9, especially in
  Microsoft apps. wispr-fox now captures your cursor position when
  you start speaking and puts it back exactly there just before
  pasting, even if the app moved focus during the wait.

### Three versions of every recording

Open the **History** page and every recording now has three tabs:

- **Raw** — exactly what Whisper heard
- **Cleaned** — same words, but with spelling, punctuation, and
  paragraphing fixed (no rewriting)
- **Drafted** — your brief turned into a polished email/message/doc

The tabs you didn't originally use are dimmed. **Click any dimmed tab
to generate that version on demand** — no need to re-dictate. Useful
when you pressed F8 (raw) but later wished you'd pressed F9 (draft).

Click anywhere on a row to expand it now — you don't have to hit the
little arrow.

### F8 is now smarter

- **Default:** F8 still gives you a raw transcript, instantly.
- **New toggle in Settings:** turn on "Clean up raw" and every F8
  press automatically polishes spelling, punctuation, and paragraphing
  while keeping your exact words.
- **Shift+F8:** one-shot override. Force the cleanup on for just
  *this* recording without flipping the setting. Useful when you
  usually want raw but occasionally want it cleaner.

F10 is retired by default — it activates the menu bar in Outlook and
caused paste problems. F9 has moved up to take its drafting role.

### Clippy got a glow-up

- **New elephant ear** when listening — unfurls from Clippy's side
  and flaps gently. Replaces the earlier (deservedly criticised)
  metallic-spring attempt.
- **Bigger eyes** with catchlights, so Clippy feels alive.
- **Eyes follow your cursor** when you hover over the floater. Clippy
  notices you.
- **New "Cream" skin** — a beige/warm-brown variant of the paperclip
  that reads better on dark wallpapers. Switch in the sidebar.
- The Chippy potato-chip skin is gone (it didn't earn its keep).

### Better defaults

- **Silent startup.** Launch and only the tray icon + Clippy appear;
  the main window stays hidden so you're not nagged. Double-click
  Clippy or left-click the tray icon to open Settings/History when
  you need them. Opt out in Settings → Startup → "Open silently".
- **Launch at login** is now a one-toggle setting (Settings →
  Startup → "Launch wispr-fox at login"). Survives reboots.
- **Dictation stays on your clipboard.** After every recording, the
  cleaned text remains on your clipboard so Ctrl+V works as a backup
  delivery method even if the auto-paste landed somewhere odd.
- **If you've switched apps** during the LLM wait, wispr-fox no
  longer yanks you back. It silently copies the result and tells
  Clippy to show "Copied to clipboard 📋". Press Ctrl+V wherever you
  are now. Toggle this off if you prefer the old yank-back behavior.

### Quality of life

- **Version number under the brand** in the sidebar — you always
  know which build you're running.
- **"Today's usage" panel shows a countdown** to the next limit
  reset in *your* local time, not UTC.
- **"Clear all"** now spells out exactly what it deletes (text,
  audio, all rows) before you confirm.
- **Refresh button** is honest about what it does: just reload the
  list, no re-transcription.
- **Soft floor shadow under Clippy** pulses while listening — applies
  to all skins now, not just the paperclip.

---

## ⬇ Get it

Windows: download **`wispr-fox_0.2.0_x64-setup.exe`** below.

The installer puts it in your user folder — no admin needed. Run,
click through, done. First launch walks you through pasting your Groq
API key (free tier is plenty).

---

## 🔄 Upgrading from an earlier build

- Your existing recordings and settings carry over — nothing is
  deleted on install.
- The first launch auto-migrates a few stored values: old F10
  drafting recordings move to the new "Drafted" tab, and your hotkey
  settings update from F8/F9/F10 to F8/F9 (F10 retired).
- If your old F8 hotkey behavior felt fine, nothing changes for you.
  Shift+F8 is purely additive.

---

## 🐛 Known limits

- macOS build isn't code-signed yet — first install needs a
  right-click → Open dance. Full walkthrough in the README.
- Same-window click-away inside Electron apps (Slack, Discord) can
  still cause the paste to land in the wrong control if you clicked
  somewhere else while the LLM was thinking. We don't fight Electron's
  internal focus manager. Workaround: just stay put, or use the
  clipboard fallback (it's always on by default).

---

Issues or feedback → open a [GitHub issue](https://github.com/kumaradarsh1993/wispr-fox/issues).
