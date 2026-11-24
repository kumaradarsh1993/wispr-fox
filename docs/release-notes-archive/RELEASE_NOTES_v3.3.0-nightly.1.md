# wispr-fox v3.3.0-nightly.1 — Codex meeting workflows

This Codex-authored nightly turns diarized uploads into a first-class meeting
workflow while keeping ordinary dictation fast and familiar.

## Meetings that remain readable

- Choosing speaker labels during upload marks the recording as a meeting and
  gives its History card a subtle meeting treatment.
- Raw and cleaned transcripts render as structured speaker turns instead of a
  wall of text, with clearer spacing and typography for longer conversations.
- A focused reading mode opens the selected Raw, Cleaned, Drafted, or Meeting
  Notes version in a calm full-screen view.

## Name speakers once, use them everywhere

- The card menu can now open a speaker editor for every detected speaker.
- Names replace generic Speaker 1 / Speaker 2 labels immediately in the card,
  reading mode, and copied text without re-transcribing the recording.
- Meeting reading mode keeps speaker naming alongside the transcript so names
  can be assigned while reviewing who said what.

## Succinct meeting notes

- Meeting Notes is now its own version rather than being hidden inside Draft.
- The default prompt aims for a concise leadership-ready summary of the main
  discussion, decisions, risks, and next actions.
- The Meeting Notes prompt and its stronger writing model are configurable in
  Settings, and notes can be generated later even if they were not requested
  during the original upload.

## One flexible rerun flow

- The separate re-transcribe, cleanup, and draft menu items are now one Rerun
  dialog.
- Transcription, cleanup, draft, and meeting notes can be selected independently
  or together; dependent AI work waits for a selected transcription rerun.
- Transcription and writing stages each expose the relevant provider and model,
  using the application's saved defaults as the starting point.

## Smarter diarization choices

- OpenAI's dedicated diarization transcription path is supported alongside
  Deepgram and ElevenLabs speaker labeling.
- If speaker labels are enabled with an incompatible Whisper model, wispr-fox
  switches to a compatible configured engine and explains the change.
- Groq and Gemini defaults have been refreshed away from retiring model IDs,
  while ordinary OpenAI transcription uses its current non-diarized model.

## Validation

- Svelte diagnostics: 0 errors, 0 warnings.
- Production frontend build: passed.
- Rust desktop check: passed with only the same pre-existing warnings.

This is a nightly for real-world testing of long meetings, speaker naming, and
the new rerun combinations before the next stable promotion.
