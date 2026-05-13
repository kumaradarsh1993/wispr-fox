# v0.3.1 — Hotfix: visual revert + bug squash

Hotfix on top of v0.3.0 reverting a couple of visual regressions and
fixing two real bugs that v0.3.0 introduced or exposed.

---

## 🐛 Fixes

### "Transcription already underway" red box mid-recording

While holding F8 for a long dictation (2+ minutes), a red error toast
would sometimes appear saying recording was already in progress. The
recording itself was fine, but the toast was alarming.

**Root cause:** Windows auto-repeats global hotkey events while you
hold a function key (~30 fires/sec). The first one started the
recording; later repeats hit a guard that errored with "already in
progress" and surfaced the error to Clippy. v0.3.1 silently swallows
auto-repeat events at the dispatcher level — you'll never see this
toast again unless something *actually* tries to start two recordings.

### F9 turning every brief into an email

When you pressed F9 inside Outlook, the LLM was applying full email
format (greeting + body + sign-off) to literally everything you said
— even a paragraph-length reply where you didn't want any greeting.

Two prompts were stacking ("the user is in an email composer" + "if
they say 'draft an email' produce greeting + body + sign-off"). v0.3.1
rewrites both:
- The Drafting system prompt now defaults to **plain paragraph
  output** and only adds email formatting when the brief explicitly
  says "email" / "reply to" / "write to" etc.
- The Email context hint nudges *register* (professional), not
  *format* — bullets and greetings come from the brief, not from the
  app.

### Clippy #1 (Paperclip) visual regressions reverted

The v0.3.0 stylized paperclip got a few additions that made it feel
worse, not better:
- Tick-mark / checkmark morph at completion → reverted
- Green sparkles burst → reverted
- Click-giggle wobble → reverted

The paperclip is back to its **v0.2.0 baseline** — elephant ear,
bigger eyes with catchlights, hover tracking. All future experiments
land on the new **fox skin** instead (coming in a later build).

### History tabs are inline now

The Raw / Cleaned / Drafted tabs used to sit as a separate bar above
each row's text, wasting vertical space. They now live **inline next
to the time + duration metadata**, as a compact iOS-style segmented
control. Same functionality (click a dim tab to generate on demand),
just one row shorter.

---

## 🦊 Coming next

Research is done on the **fox skin** — turns out no off-the-shelf
"realistic fox with 5 named animation states" exists, so the next
step is either curating 5 free Lottie animations and sequencing them,
or commissioning a single Rive file with a state machine. That work
lands in v0.4.0.

---

## ⬇ Get it

Windows: `wispr-fox_0.3.1_x64-setup.exe` below. Install over your
existing v0.3.0 — settings, history, recordings all carry forward.
