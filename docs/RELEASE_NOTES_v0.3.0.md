# v0.3.0 — Clippy gets smarter, Settings gets cleaner

Backlog cleanup release. Builds on v0.2.0 with Clippy that knows what
app you're in, a sturdier UI when things go wrong, and a settings page
that finally puts each mode's stuff in one place.

---

## 🎯 The headline feature: F9 now matches the app you're in

Press F9 while you're in **Outlook** and the LLM gives you a formal
email — greeting, body, sign-off. Press F9 in **WhatsApp Web** with
the same brief and you get *"hey can we move it to 4 pm?"* — casual,
lowercase, no fluff.

wispr-fox does this by quietly noticing which app the cursor was in
when you started talking, classifying it into a coarse bucket (email
/ casual chat / work chat / social post / document / code / AI chat),
and nudging the LLM toward the right register. Only the bucket name
("email", "chat", etc.) is sent to the LLM — never your window title
or process name.

Affects F9 drafting only. F8 transcription always preserves your
exact voice. Toggle off any time in **Settings → Hotkeys → Adapt
tone to active app**.

### What we classify today

| App | Bucket |
|---|---|
| Outlook, Thunderbird, Gmail in browser | Email |
| WhatsApp, Telegram, Signal | Casual chat |
| Slack, Teams, Discord | Work chat |
| LinkedIn, X/Twitter (in browser) | Social public |
| Reddit, Hacker News | Social casual |
| Word, Notion, Obsidian, Google Docs | Document |
| VS Code, Cursor, JetBrains IDEs | Code |
| ChatGPT, Claude.ai, Gemini | AI chat |
| Anything else | Default (no adaptation) |

---

## ✨ Other improvements

### Clippy is more alive

- **Paperclip → checkmark morph** when dictation completes. The
  paperclip body unfurls into a green tick, sparkles burst, then
  it morphs back ready for the next recording. Stylized + Cream
  skins both.
- **Click on Clippy** to make him giggle (a quick rotate-and-bounce).
  Double-click still opens the main window — clicks don't conflict.
- **Watchdog timer** as a last-ditch safety net: if Clippy ever
  appears stuck in "thinking" / "writing" for more than 90 seconds
  without backend progress, the floater auto-resets to idle and
  shows a "Took too long" toast. The Rust pipeline already catches
  every known failure path; this is for the unknown unknowns.

### History page

- **Tabs are now centered** above the body, not left-aligned.
- All other v0.2.0 history features unchanged (Raw / Cleaned /
  Drafted, click-anywhere-to-expand, on-demand generation).

### Settings page reorganized

The Modes section in **Settings → Models** is now self-contained per
mode. Each mode block shows:

1. Its hotkey (main + sticky-invoke)
2. The "default to sticky" checkbox
3. The LLM cleanup toggle
4. The full editable system prompt

You can configure F8 or F9 end-to-end without bouncing between two
settings sections. The standalone Hotkeys section is still there for
the cross-cutting bindings (Shift+F8 force-clean) and the behaviour
toggles.

### Recovery from crashes

- **Startup scan** for stranded recordings — if wispr-fox was killed
  or crashed mid-pipeline, any recordings stuck at "transcribing" /
  "cleaning" / "injecting" are now automatically marked as errored
  on next launch so the Retry button works on them. Previously they
  could sit in limbo indefinitely.

---

## ⬇ Get it

Windows: download **`wispr-fox_0.3.0_x64-setup.exe`** below.

Settings, history, and API keys carry over from v0.2.0. The startup
recovery scan will run once on first launch — if you had any stranded
recordings from a crash, they'll show up as failed and clicking Retry
will pick up where they left off.

---

## 🐛 Still pending

- **macOS DMG** — no Mac binaries on Releases yet. CI configuration
  is incomplete. Will land in a future build.
- **Hover micro-behaviors** beyond eye-tracking + click-giggle (laugh,
  idle animations) — deferred. Current set is enough for "Clippy
  feels alive" without being distracting.
- **Same-window click-away inside Electron** (Slack/Discord/Teams)
  can still drop the paste in the wrong control if you click
  somewhere else mid-LLM. We don't fight Electron's focus framework;
  clipboard fallback (always on by default) is the workaround.

---

Issues or feedback → open a [GitHub issue](https://github.com/kumaradarsh1993/wispr-fox/issues).
