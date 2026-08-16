# Model matrix — what we ship, and what it costs to run

> **Researched 2026-08-16.** Free tiers change often and silently; re-check
> before trusting anything here, and re-stamp this date when you do. Every
> "free tier" claim below comes from the provider's own pricing/limits page,
> not from a blog and not from memory.
>
> **The live check is `node scripts/model-check.mjs`.** This document is the
> commercial picture; the script is the functional one. Run the script — it
> reads the model ids out of `src/lib/provider-options.ts` and puts one real
> request through each, so it cannot drift from what the app ships.

The app offers **25 model ids** across 5 providers. They are *not* equally
usable: eight of them cannot be run for free at all.

## The short version

| Provider | Free to run? | Verdict for a BYO-key app |
|---|---|---|
| **Groq** | Yes, permanent free tier | Safe default. Watch the TPM ceiling (below). |
| **Gemini** | Yes, free tier on 8 of our 9 ids | Safe, except `gemini-3.1-pro-preview`. |
| **Deepgram** | $200 signup credit — **not recurring** | Fine for a long while, then it stops. |
| **OpenAI** | **No free tier** | Every OpenAI id we ship is paid-only. |
| **ElevenLabs** | Free tier exists but is ~10 API credits/month | Effectively unusable free. |

## Groq — permanent free tier, tight token ceiling

Free plan limits ([console.groq.com/docs/rate-limits](https://console.groq.com/docs/rate-limits)):

| Model | Role | RPM | RPD | TPM | TPD |
|---|---|---|---|---|---|
| `openai/gpt-oss-20b` | LLM (default cleanup + titles) | 30 | 1,000 | 8,000 | 200,000 |
| `openai/gpt-oss-120b` | LLM (drafts, meeting notes) | 30 | 1,000 | 8,000 | 200,000 |
| `qwen/qwen3.6-27b` | LLM | 30 | 1,000 | 8,000 | 200,000 |
| `whisper-large-v3-turbo` | STT (default) | 20 | 2,000 | 7,200 audio-sec/hr | 28,800 audio-sec/day |
| `whisper-large-v3` | STT | 20 | 2,000 | 7,200 audio-sec/hr | 28,800 audio-sec/day |

**The ceiling that will actually bite is 8,000 TPM on the LLM models**, and it
is shared across the whole free plan. A long dictation is not small: a real
25-minute recording in this user's history produced a 19,370-character
transcript, roughly 5,000 tokens *before* the cleanup prompt and the response.
Two of those inside a minute exceeds the free TPM and the provider returns 429.

That is the most likely explanation for "the largest Groq model gave an error" —
a throttle on a long transcript, not a dead model id. `scripts/model-check.mjs`
distinguishes the two: it reports `LIMIT` for a throttle and `GONE` for an id
the provider no longer serves.

Audio is far more forgiving: 28,800 audio-seconds/day is 8 hours of dictation.

## Gemini — free on everything we ship except one

From [ai.google.dev/gemini-api/docs/pricing](https://ai.google.dev/gemini-api/docs/pricing):

| Model id | Free tier |
|---|---|
| `gemini-3.6-flash` (our default) | Free of charge |
| `gemini-3.5-flash` | Free of charge |
| `gemini-3.5-flash-lite` | Free of charge |
| `gemini-3.1-flash-lite` | Free of charge |
| `gemini-3-flash-preview` | Free of charge |
| `gemini-2.5-flash` | Free of charge |
| `gemini-2.5-flash-lite` | Free of charge |
| `gemini-2.5-pro` | Free of charge |
| **`gemini-3.1-pro-preview`** | **Not available** — paid tier required |

All nine ids exist and are current per
[the model list](https://ai.google.dev/gemini-api/docs/models); none are
retired. Only the Pro preview breaks the "must be usable free" rule, and its
picker label already says "billing likely required" — which is honest, but it
is the one entry that cannot work on a free key at all.

Per-model RPM/RPD numbers are **not published as a static table** — Google
serves them per-project at
[aistudio.google.com/rate-limit](https://aistudio.google.com/rate-limit). Do not
copy numbers for these from third-party blogs; they disagree with each other and
with Google.

## Deepgram — a credit, not a free tier

$200 of signup credit on Pay-As-You-Go, no card required. Nova-3 pre-recorded
runs about $0.0043–0.0092/min depending on mono/multilingual, so the credit is
worth roughly 400+ hours of transcription. It does **not** renew.

The sidebar already estimates spend against this credit, but that is an
estimate derived from local usage. `scripts/model-check.mjs` reads the **real
remaining balance** from `/v1/projects/{id}/balances` — worth doing occasionally,
because the estimate and the truth will drift.

## OpenAI — no free tier, at all

There is no permanent rate-limited free tier. New accounts get a one-time $5
trial credit that expires after three months; everything else is
pay-as-you-go. That makes **all 8 OpenAI ids we ship paid-only**:

- STT: `gpt-transcribe`, `gpt-4o-transcribe-diarize`, `gpt-4o-transcribe`,
  `gpt-4o-mini-transcribe`, `whisper-1`
- LLM: `gpt-5.4-mini`, `gpt-5.4`, `gpt-5.5`

This is not a reason to remove them — a BYO-key app should let someone spend
their own money if they want to. It *is* a reason not to let anyone land on one
by accident, and to say so in the picker.

## ElevenLabs — free in name only

The free plan grants on the order of 10 API credits/month, which will not cover
routine dictation. `scribe_v2` is the only id we ship (`scribe_v1` was retired
upstream and removed). Treat it as a paid option.

## Standing rules for this list

1. **Never add a model id from memory or from a blog.** Verify it against the
   provider's own model list, then run `scripts/model-check.mjs`. Three ids in
   this app's history were wrong — `llama-4-maverick` was never a real Groq id,
   and `distil-whisper-large-v3-en` / `scribe_v1` were retired upstream while
   still being offered in the picker.
2. **A model that cannot be run on a free key must be labelled as such** in
   `provider-options.ts`. Today that is every OpenAI id, every ElevenLabs id,
   and `gemini-3.1-pro-preview`.
3. **Re-run the check after any provider announcement**, and before promoting a
   release to stable. It exits non-zero if a shipped id is dead, so it can gate
   CI if we ever want that.
