# LinkedIn post — wispr-fox launch

Three variations, pick whichever lands best. All meet the same brief:
**Clippy is back. Free WisprFlow alternative. Open source. Fast.
Configurable. $1,500 burned to learn what it actually costs.**

---

## Variation A — the receipts-first version (RECOMMENDED)

I spent **$1,500 on Groq + Gemini API tokens in 10 days** so you
don't have to wonder whether dictation can replace typing.

It can. With one footnote.

For the last week and a half I've been daily-driving **wispr-fox** —
a tool I built because WisprFlow is $15/mo, locked to their model
choice, and very politely refuses to deal with Indian-accented
English code-switching into Hindi.

What I shipped:

→ Press F8: raw transcript, no AI cleanup. The fastest path from
  thought to text.
→ Press F9: strict grammar + spelling pass. Preserves your voice.
→ Press F10: dictate a brief, get a polished draft back.
→ Bring your own Groq or Gemini key. Zero subscription.
→ Whisper Large v3 Turbo handles my accent + Hindi mid-sentence
  without choking.
→ Real Microsoft Clippy is back. Yes, the sprite. Yes, he blinks.
  No, he will not warn you about your letter.

The footnote: at heavy daily use I burned through ~$150/day in
tokens before tuning prompts. After cutting unnecessary LLM calls
on the raw-transcript hotkey and shortening system prompts, I'm
at **under $5/day** for the same throughput.

Open source. Windows-first, Mac coming. Bring-your-own-API-key.
No telemetry. No "Pro" tier.

If your job involves writing more than typing — engineers writing
docs, PMs writing specs, founders writing investor updates — try it.

Link in comments. Clippy says hi.

#OpenSource #Productivity #BuildInPublic

---

## Variation B — the Clippy-forward version

It looks like you're trying to write a document.

Would you like help with that?

For the first time in 25 years, the answer is yes.

**wispr-fox** — a free, open-source alternative to WisprFlow that
brings back the original Microsoft Clippy as your dictation
companion. Press F8, talk, get text. Press F10, give a brief, get
a draft. Bring your own Groq or Gemini key. No subscription. No
telemetry. No "Pro" tier.

I built it because:
1. WisprFlow doesn't handle Indian English + Hindi code-switching.
2. $15/mo for a wrapper around Whisper felt steep.
3. I genuinely missed Clippy.

10 days of daily use, $1,500 in API spend to stress-test it, and
it's now my primary input method for everything that isn't code.

Free. Open source. Windows now, Mac soon.

Comments for link.

---

## Variation C — the engineering-flex version

Things I learned building a sub-second dictation tool in 6 weeks:

→ Whisper hallucinates "thank you" / "gracias" when your audio
  buffer is empty. Realtek's "Audio enhancements" power-down the
  mic between recordings. Disable it. 5-second improvement.

→ `cpal` cold-start on Windows is ~25ms with a 10ms buffer config.
  Always-hot capture isn't worth the privacy ick.

→ Groq's Whisper Large v3 Turbo is 5× cheaper than OpenAI's and
  faster end-to-end for short clips. (~0.5s for 10 seconds of audio.)

→ Tauri 2 + Svelte 5 + Rust is genuinely the leanest desktop stack
  I've used. 14MB installer, 35MB resident.

→ Indian-accented English transcription works **better** when you
  leave `language` unset and let Whisper auto-detect. Pinning it to
  `en` makes Hindi sections garbage.

→ Vite cannot statically analyze clippyts' dynamic imports. You
  have to vendor the package and patch one function. (Worth it —
  the real sprite is irreplaceable.)

I packaged all of the above as **wispr-fox**. Free, open-source,
Windows-first WisprFlow alternative with bring-your-own-key for
Groq or Gemini.

Burned $1,500 in tokens daily-driving it for 10 days as a torture
test before sharing. It survived.

Link in comments. Clippy approves.

#Tauri #Rust #Whisper #OpenSource

---

## Posting checklist

- [ ] Pick variation (A recommended for broad reach, B for nostalgia
      crowd, C for engineering circles)
- [ ] Put GitHub link in the **first comment**, not the post (LinkedIn
      throttles posts with outbound links)
- [ ] Pin the comment with the link
- [ ] Attach: short loom or 30s screen capture of F8 → F10 flow with
      Clippy reacting
- [ ] Tag: nobody. Organic-only first 24h.
- [ ] Reply within 30 min of first comments — algorithm window.
- [ ] Cross-post to X (Twitter) 6h later with shorter variant.

## Things to NOT say

- Don't claim it's "better than" WisprFlow. Just say "alternative".
  Avoids defensive responses from their users.
- Don't lead with the $1,500 number in the headline — feels braggy.
  Drop it in body as a learning anecdote.
- Don't promise Linux support yet (it builds in CI but is untested).
- Don't promise streaming transcription.
- Don't mention the Android sibling project — separate launch.
