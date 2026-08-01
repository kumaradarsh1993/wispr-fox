# wispr-fox v3.1.0

> Stable release · 1 August 2026

## A much better microphone companion

Choose an external microphone, test it with a live level meter, and see whether
audio is truly flowing before you start speaking. WhisperFox now handles slow
Bluetooth wake-ups honestly, falls back safely when a saved mic is unavailable,
and can rescue recordings that are quiet enough to lose whole phrases.

Noise reduction is available for fan-heavy laptop microphones, including an
aggressive speech-focused option. Processing always happens on a temporary copy;
your original recording remains untouched.

## Voice memos and meetings

Uploads now accept common 16-, 24-, and 32-bit WAV formats and are normalised to
an efficient speech format before transcription. Supported providers can label
speakers, and the Draft path can turn a recording into structured meeting notes.

## More dependable every day

This release hardens sign-in restoration, cross-device key refresh, concurrent
token renewal, remote deletes, and the final paste back into the app where you
started. Hotkey changes now apply immediately and the recorder temporarily gets
out of the way while you capture a new shortcut.

## Better recovery when AI misses

Gemini cleanup and drafting now use provider-aware timeouts and model-specific
thinking controls. From History you can re-run cleanup or drafting with your
current model, and choose a separate lightweight model for automatic titles.

---

Authored across the v3.1.0 nightly cycle by Claude and promoted to stable by
Codex after the owner's explicit approval.
