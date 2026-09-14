# wispr-fox v3.5.0-nightly.1

**Cleanup on Groq works again, OpenAI cleanup works for the first time in a
while, and the groundwork for the v3.5 redesign is in.**

First nightly of the 3.5 line. The visible changes are two fixes; the rest is
foundation you will only notice as the next few nightlies land.

## Groq: "Cleanup failed — pasted raw text"

Three problems wearing one message.

Groq's Qwen models think out loud *inside* their reply — the chain of thought
arrives wrapped in `<think>` tags in the actual text. The app's "did the model
change the text too much?" guard saw a reply twice as long as the dictation and
rejected it. Raw text pasted, cleanup reported as failed.

Groq also caps replies at 1,024 tokens by default, and on its reasoning models
the thinking spends that budget. A long dictation could use the whole cap
thinking and return **no answer at all**. The app required an answer, so the
reply failed to parse. This one hit the GPT-OSS models too, which is why it
felt random: short dictations worked, long ones did not.

Now the app tells Groq to hide its reasoning and think lightly (cleanup is a
rewrite, not a proof), raises the ceiling to 16k tokens, strips any thinking
that leaks anyway, and reports a clear error on an empty reply instead of a
decode failure. Four tests pin the request shape and the parser edge cases.

## OpenAI: the models we offered did not exist

`gpt-5.4-mini`, `gpt-5.4` and `gpt-5.5` were never OpenAI model ids. Anyone who
picked OpenAI for cleanup got an error on every dictation. The list is now the
live `gpt-5.6` family — Terra (default, fast), Sol (highest quality), Luna
(cheapest). Saved selections migrate automatically.

## Model catalog refresh (checked against official docs, 2026-09-14)

- Groq: Llama models are now Enterprise-only and are no longer offered; old
  Llama selections migrate to GPT-OSS. Qwen 3.8 added. Both Qwens are labelled
  as preview and noticeably pricier than GPT-OSS, which stays on the free tier.
- Gemini: 3.7 Flash and 3.8 Flash added. Everything previously listed is live.
- OpenAI speech: `gpt-transcribe` is the recommended model and stays first.
  The `gpt-4o-*` and `whisper-1` ids are deprecated with a shutdown date of
  2027-02-26; they still work and remain listed for now.
- Deepgram and ElevenLabs: unchanged, everything live.

## Under the hood: v3.5 foundations

- One type scale (six sizes, nothing under 11px), one spacing scale, semantic
  colour tokens for recording kinds. The current UI uses fifteen font sizes
  down to 8px; the next nightlies migrate each surface onto the scale.
- Shared building blocks (`src/lib/ui/`): Button, Field (shows its default),
  Disclosure (the one "Advanced ▸" pattern), Segmented, Kbd.
- The recording status badge follows the theme now. It was hard-coded to grey,
  red and blue regardless of Light, Dark or Retro.
- **macOS:** the window uses an overlay titlebar — the sidebar runs up under
  the traffic lights like Notes or Finder, and the top of the sidebar is the
  drag handle. Windows keeps its native titlebar. If anything sits under the
  traffic lights on your Mac, say so; this is the one visual change to check.

The full plan is in `docs/DESIGN_v3.5.md`.
