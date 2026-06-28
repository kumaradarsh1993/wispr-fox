# wispr-fox v1.4.0-nightly.6 - Codex provider expansion

This is a Codex-authored nightly. The previous commit is preserved as the final
Claude Code checkpoint before this release line.

## What changed

- Added speech-to-text providers: OpenAI GPT transcription, Deepgram Nova, and
  ElevenLabs Scribe, alongside the existing Groq Whisper path.
- Added OpenAI as a cleanup/drafting provider via the Responses API.
- Expanded Provider settings so each service has its own saved key, connection
  test, provider picker, and model picker.
- Kept the new Khaumani & Indy+ animated pet and added it to the floater
  right-click avatar picker.
- Hardened privacy/security details: transcript text is no longer logged,
  stale plaintext keys migrate to the OS keyring only after verified writeback,
  Gemini key tests no longer put API keys in URLs, and unused filesystem plugin
  access was removed.
- Fixed update checks so double-digit nightlies compare correctly.
- Restored the intended macOS floater override: opaque, shadowed, and no
  macOS private API request.

## Notes

Nightly builds are pre-release builds. They are for trying the new provider
matrix before any stable promotion.
