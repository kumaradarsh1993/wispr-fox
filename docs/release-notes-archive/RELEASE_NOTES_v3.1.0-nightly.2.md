# wispr-fox v3.1.0-nightly.2 — Gemini works again, plus re-run cleanup and draft

Every Gemini model had stopped working. Cleanup and drafting would spin, then
quietly hand back the raw transcript. Groq kept working fine, which made it look
like Gemini itself was down. It wasn't — the bug was ours, and this nightly
fixes it. It also adds the thing you'd actually want after a bad result: the
ability to run cleanup or draft again with a different model, without
re-recording or re-transcribing.

## Gemini is fixed

The short version: **wispr-fox wasn't waiting long enough.**

Every cleanup call had a hard 8-second deadline. That was set back when the only
option was Groq's Llama, which answers in a second or two. Today's Gemini models
(2.5 and the whole 3 line) *think before they answer* — a silent reasoning phase
that on its own can outlast 8 seconds. So the deadline fired mid-thought, every
time, on every Gemini model, and you got your raw transcript back with a
`clippy_timeout` note.

Three changes:

- **Gemini is now asked for the least thinking it allows.** Cleanup and drafting
  are style transforms — they gain nothing from a reasoning budget. This alone
  takes most of the delay away.
- **The deadline is per-provider instead of one-size-fits-all.** Fast models
  still fail fast; slower ones get the room they need.
- **Re-running from History gets a much longer deadline** than live dictation.
  There's no paste waiting on it — you clicked a button and you're watching a
  spinner — so it's better to finish than to fail fast.

Two related papercuts went with it: a long Gemini answer that arrived in several
pieces could get cut off after the first piece, and when something did go wrong
the error just said "empty response", which told you nothing. It now names the
actual reason.

## Re-run cleanup and draft

The 3-dot menu on any history card gains **Re-run cleanup** and **Re-run draft**,
sitting next to the existing Re-run transcription. Both show which model they'll
use, so the workflow is: pick a different model in the sidebar, open the menu,
run it again. Each is a fresh pass over the transcript you already have — no new
recording, no second transcription charge.

If a cleanup or draft already exists, it asks before replacing it.

## Choose who names your recordings

Automatic recording titles used to be locked to one small Groq model, with only
an on/off switch buried in General. Titles now have a proper **Service** and
**Model** picker, and the on/off switch has moved to sit with them, under
**Settings → Providers & API keys → Recording titles**.

Nothing changes unless you want it to — the default is the same small fast model
as before, which is still the sensible pick for writing five words.

## Notes

- Your saved Gemini model choice is kept. Model IDs were re-checked against
  Google's current list; a handful of retired ones now roll forward to a working
  model instead of failing.
- If Gemini is still slow for you, `Gemini 2.5 Flash-Lite` is the quickest of
  the free-tier options.
