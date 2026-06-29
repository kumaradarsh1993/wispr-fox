# wispr-fox v1.4.0-nightly.8 - Codex settings cleanup

This is a Codex-authored nightly focused on making Settings easier to scan and
turning the sidebar model area into a real control surface.

## What changed

- Reorganized Settings into clearer sections: Providers, Modes, Dictation,
  Avatar, General, and Security.
- Moved data-retention controls into General and redirected the old Data page.
- Simplified Providers so STT and LLM service/model selection comes first, while
  API keys live behind a collapsed "Manage API keys" section.
- Added an always-visible sidebar Models panel with STT and LLM service/model
  dropdowns plus a Clean toggle for the default Transcribe behaviour.
- Kept usage anchored at the bottom, with Deepgram now showing estimated spend
  against a $200 credit when Deepgram STT is selected.
- Renamed visible floater controls to Avatar and retired the newer Khaumani &
  Indy remaster from selectable UI.
- Cleaned up Dictation settings copy and responsive settings layout so narrower
  windows are less cramped.
- Updated the macOS permission banner styling so it follows the active theme.

## Notes

Deepgram spend is an estimate based on Nova-3 multilingual pre-recorded pricing
($0.0092/min). It is not a live Deepgram billing readout.
