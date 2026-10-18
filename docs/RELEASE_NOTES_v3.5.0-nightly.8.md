# wispr-fox v3.5.0-nightly.8

**Sign-in now works on networks that block Supabase, and the model lists are
current again.**

## Sign-in: "could not reach the sync service … tls handshake eof"

wispr-fox signs you in and syncs through Supabase, at an address ending in
`supabase.co`. Since 24 February 2026, several Indian internet providers (Jio,
Airtel, ACT Fibernet) have blocked those addresses under a government order.
They do it through DNS, the lookup that turns an address into a server: the
provider hands back a dead-end server, the connection starts, and the secure
handshake is dropped. The block is patchy by provider and city, which is why
sign-in can work on one network and fail on another.

The browser part of Google sign-in still worked, because browsers use their
own encrypted DNS. The app's own request at the end did not.

**Fix:** the app now looks up Supabase addresses itself through Cloudflare's
or Google's encrypted DNS (DNS-over-HTTPS), contacting them by IP address so
the lookup cannot be blocked the same way. If both fail, it falls back to the
normal lookup. The secure connection still checks Supabase's real
certificate, so a wrong answer can never be trusted. Nothing else the app
talks to (Groq, OpenAI and the rest) is affected.

## Models

Checked against each provider's own documentation on 19 October 2026.

- **Removed Qwen 3.6 27B.** Groq shut it down on 14 September 2026, so
  cleanup with it returned an error. A saved Qwen 3.6 pick now switches to
  Qwen 3.8 27B automatically.
- **Removed Gemini 3.7 Flash and Gemini 3.5 Flash.** Google deprecated both
  and silently routes them to newer models, so choosing them did not do what
  the label said.
- **Gemini's default is now Gemini 3.8 Flash**, Google's newest stable Flash
  model.

Groq Whisper, GPT-OSS, the OpenAI GPT-5.6 family, Deepgram Nova-3 and
ElevenLabs Scribe are all current and unchanged.
