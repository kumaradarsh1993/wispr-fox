# wispr-fox v1.4.0-nightly.9 - Codex settings polish

This is a Codex-authored nightly that tightens the Settings cleanup from
nightly.8 and adds more useful model-usage tracking.

## What changed

- Collapsed Settings -> Security's key event log by default, so diagnostics are
  still available without dominating the page.
- Removed the retired Cat lab avatar from selectable Avatar surfaces; saved
  values migrate back to Desk Cat.
- Made the left sidebar resizable and widened the saved-width floor, so STT and
  LLM model pickers have enough room.
- Kept the bottom usage area compact by moving decorative sidebar extras back
  into the scrollable sidebar content.
- Added per-day, per-provider, per-model usage buckets for STT and LLM calls.
  STT tracks successful audio seconds; LLM tracks input/output/total tokens when
  the provider response includes usage metadata.
- Updated the sidebar usage readout to show current model audio/tokens instead
  of only coarse global counters.
- Synced the native Tauri window titlebar theme with the app theme instead of
  forcing the main window to stay light.

## Notes

Deepgram credit remains an estimate based on Nova-3 multilingual pre-recorded
pricing. LLM cost is intentionally not estimated in-app yet because provider
pricing changes too often; token usage is now recorded so a pricing layer can be
added later without losing history.
