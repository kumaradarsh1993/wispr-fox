# wispr-fox v2.0.0 - Codex provider, settings, and avatar release

This stable release promotes the user-tested Codex avatar/provider line after
one final floater sizing correction. The earlier `v1.4.0` draft was canceled
before publication; `v2.0.0` is the stable release users should install.

## More speech-to-text and cleanup providers

- Added selectable speech-to-text providers beyond Groq: OpenAI GPT
  transcription, Deepgram Nova, and ElevenLabs Scribe.
- Added OpenAI as a cleanup/drafting provider via the Responses API.
- Expanded provider settings with per-service API keys, connection tests,
  service pickers, and model pickers.

## Safer key storage and clearer diagnostics

- Windows now uses OS keyring storage first, with a DPAPI-encrypted local
  fallback instead of plaintext fallback files.
- Older plaintext fallback files migrate safely into the keyring or encrypted
  fallback when possible.
- Settings -> Security now shows storage status, keyring health, fallback
  paths, and a no-secret key event log.
- Secret-handling guardrails were tightened: transcript text is no longer
  logged, Gemini key tests no longer put keys in URLs, and unused filesystem
  access was removed.

## Cleaner Settings and model controls

- Settings are reorganized into Providers, Modes, Dictation, Avatar, General,
  and Security.
- Provider selection is easier to scan, with API-key management tucked behind a
  collapsed section.
- The sidebar now has always-visible STT and LLM pickers plus a Clean toggle
  for Transcribe mode.
- The sidebar can be resized so long provider/model names have room.
- Usage tracking now records per-day, per-provider, per-model STT audio seconds
  and LLM token counts when providers return usage metadata.
- The native titlebar now follows the app theme.

## Avatar and floater polish

- Added Codex-authored high-fidelity raster avatar support for Codex Fox, Oru &
  Gujia, and Spark Buddy.
- Raster avatars now scale correctly with the floater S/M/L setting and are
  about 20% smaller than the first raster release, matching the older avatar
  footprint more closely.
- The speech/status bubble now scales independently from the avatar: it stays
  readable at the smallest size, sits above the face instead of overlapping,
  and is tighter at the largest size.
- Raster avatars are isolated from the old SVG whole-character animations, so
  they no longer roll into the window edge.
- Added an internal safe frame so subtle motion, drop shadows, and signal waves
  do not crop against the floater border.
- Regenerated the Oru & Gujia pack from a chroma-key Codex source, fixing the
  white-fur matte damage visible on dark backgrounds.
- Existing classic avatars remain backward-compatible.

## Verification

- `npm run check` passed with 0 errors. Existing unrelated warnings remain.
- `npm run build` passed. Existing unrelated bundle/a11y warnings remain.
- The v2.0.0 correction was reviewed against the reported small-scale bubble
  overlap/readability issue and the oversized raster-avatar footprint.
