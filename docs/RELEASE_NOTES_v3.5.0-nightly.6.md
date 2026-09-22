# wispr-fox v3.5.0-nightly.6

**Pause and resume mid-dictation, and a much calmer Home list.**

## Pause and resume — the new third dictation key

You can now stop talking without ending the recording. Press
**Ctrl+Option+Space** (Windows: **Ctrl+F8**) while dictating and the microphone
is released; press it again and you carry on into the same recording. Finish the
whole thing with **Option+Space** or **Escape**, from either state.

One thing to remember: **add Control to pause.** It is your normal dictation key
plus Control, and it is rebindable in Settings → Dictation.

This is for reviewing a document while narrating commentary — read a paragraph,
pause, think, resume, without the clock running and without the pressure to keep
talking just because something is recording.

**Your earlier speech is kept.** Each stretch is recorded separately and then
joined into one file before transcription, so the provider receives a **single**
request no matter how many times you paused. That matters because Groq bills per
request — five pauses must not cost five uploads.

**The microphone is genuinely off while paused.** Pause tears the capture stream
down rather than muting it, so your OS microphone indicator goes out. A "paused"
state you cannot distinguish from recording would be worse than no pause at all.
The cost is that resuming re-opens the mic (~200 ms), which is invisible next to
the pause it follows.

**The fox tells you what is banked.** It stays on screen, dimmed with a slow
breath and a small pause mark, and its bubble reads *"paused · 3 parts saved ·
Esc to finish"* — so the one question a pause raises is answered without you
having to ask it. Pause and Resume are also in the fox's right-click menu, where
Stop reads "Finish and transcribe" while paused.

**If something goes wrong, the audio survives.** A resume that cannot re-open
the microphone leaves the session paused with everything still banked, so Escape
will still transcribe what you have. And if the app is killed while paused, the
next launch finds the orphaned stretches, welds them back together and repairs
the header — the same class of rescue as the interrupted-recording fix in
nightly.5, now covering pauses too.

## Home cards, quieter

Every card was carrying five competing markers — an Uploaded badge, a platform
badge, a retry counter, a device glyph and a "Failed" pill — plus an (i) button,
a caret button, and four solid action icons. With fourteen rows on screen that is
a wall of furniture, and a marker that appears on *every* row tells you nothing.

- **The note's name now leads the card**, at full size. A list of names reads as
  a list of things you said; a list of timestamps does not. Duration and time
  moved to the right edge, in one aligned column down the list.
- **Five markers became one chip**, shown only when a row is *not* an ordinary
  dictation — Failed, Meeting, Uploaded, From phone, From web, or the name of the
  machine it synced from. So a chip now means "this one is different".
- **Nothing was lost.** Device, source, platform and retry count are labelled
  rows inside **Details** — written out rather than abbreviated to a glyph you
  have to learn.
- **One way to open a row.** The caret button is gone; clicking anywhere on the
  header expands it. Still fully keyboard-operable — Tab to a row, Enter or Space
  to open, with a visible focus ring.
- **Details** is a text link inside the opened row instead of an (i) circle on
  every collapsed one. It still shows a red dot when there is an error to read.
- **Action icons rest at 45% and come up when you hover the row**, or when it has
  keyboard focus, or when it is open. They are dimmed, not hidden: quieting the
  list is the point, but a button you have to hunt for is a worse trade.

**Raw / Cleaned / Drafted tabs stay exactly where they were**, on the collapsed
card. The design brief moved them inside the expanded row; they are the main
thing you actually click on this screen, and hiding a control you reach for
constantly is the same mistake as emptying the sidebar in nightly.2.

## Still to come in the v3.5 line

Settings restructured into Essentials and Advanced, the Insights pass, the
onboarding rework, and the menu-bar popover. See `docs/DESIGN_v3.5.md`.
