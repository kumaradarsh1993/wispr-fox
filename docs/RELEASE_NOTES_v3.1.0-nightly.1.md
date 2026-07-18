# wispr-fox v3.1.0-nightly.1 — Noise reduction for fan-heavy laptop mics

Built-in laptop mics pick up the machine's own fans — a low whirring bed under
everything you say that makes the transcriber mishear words. This nightly adds
an opt-in noise-reduction stage that scrubs that noise out of the audio sent
for transcription, tuned on real recordings from the affected machines.

## Noise reduction — Off / On / Aggressive

New in **Settings → Dictation → Noise reduction**:

- **Off** (default) — nothing changes; audio goes out exactly as recorded.
- **On** — a rumble filter that removes the deep fan hum below ~90 Hz. It cuts
  the loudest part of the fan signature and by design cannot touch speech —
  safe to leave on permanently.
- **Aggressive** — the rumble filter plus an AI speech denoiser (RNNoise, the
  same family of tech Discord and OBS use). On the test recordings this took
  the noise floor down by 20–45 dB and roughly doubled-to-tripled the
  signal-to-noise ratio. If a transcript ever comes out worse, just step back
  down to **On**.

## Built so it can't hurt you

- **Your saved recordings are untouched.** The cleanup happens on a temporary
  copy that only the transcription provider sees. History always keeps and
  plays the original audio, so you can compare and diagnose after the fact.
- **No noticeable wait.** Processing runs locally at roughly 130–600× realtime:
  a 10-second dictation costs ~15–80 ms, a full minute well under half a
  second — background noise compared to the transcription request itself.
- **The avatar tells you what's happening.** The floater shows a quick
  "clearing noise…" beat for exactly as long as the denoiser runs, and each
  recording's flight recorder logs the precise time it took (visible in the
  per-recording timeline).
- **Never costs a dictation.** If the denoiser hits any problem, the pipeline
  quietly falls back to the raw recording and carries on.

## Notes

- Applies to hotkey dictations. Uploaded audio files are transcribed as-is.
- Desktop only for now (Windows + macOS + Linux builds all include it).
