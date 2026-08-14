# wispr-fox v3.3.0-nightly.2 — Adaptive tap-or-hold dictation

This focused Codex-authored nightly replaces separate push-to-talk and sticky
bindings with one predictable shortcut per dictation mode.

## One key, two natural gestures

- Tap and release a dictation key in less than 700 ms to keep recording.
- Hold it for 700 ms or longer to use hold-to-talk; release to stop and send.
- While a tap-latched recording is active, press any configured dictation key
  or Escape to stop and send.
- The separate sticky bindings and sticky-default controls are gone from
  Settings. Existing legacy values remain readable for compatibility but are
  not registered.

## The fast/slow-computer race is closed

- A session is reserved synchronously before microphone startup begins, so a
  fast key release cannot disappear while a slow audio device is waking up.
- Key timing uses the physical edge timestamps and a strict 700 ms boundary,
  independent of database or microphone latency.
- Key repeat is latched until the matching physical release, so Windows repeat
  events cannot look like a second command.
- Starting, recording, stopping, and processing are serialized and
  session-owned. A second run cannot start while the previous transcript is
  still being produced or delivered.
- Persisted custom bindings now become live during startup instead of waiting
  for the hotkey editor to be opened.

## Honest microphone and error status

- Microphone readiness is tagged with its capture generation and source. A
  late preview or previous-session signal cannot clear the current waiting
  state, and event ordering cannot leave “mic taking a while” stuck forever.
- The floater consumes one revisioned lifecycle snapshot, rejects stale
  revisions, and rehydrates after focus or reload without inventing an idle
  transition.
- Terminal errors use a bounded, focusable scroll area with the full message,
  assertive alert semantics, a longer reading window, and an explicit dismiss
  control. Wave and Siri receive the same safe error sizing as other skins.

## Validation

- Svelte diagnostics: 0 errors, 0 warnings.
- Production frontend build: passed.
- Rust application and test targets: passed compilation with only the existing
  dead-code warnings.
- Pure adaptive reducer: 7/7 tests passed.
- Coordinator: six focused race, ownership, Busy, stale-completion, and
  microphone-generation scenarios compile locally and execute in release CI.
- Release CI now validates the tag, package/Cargo/Tauri versions, exact release
  notes, frontend checks, Rust library tests, and all Rust targets before it can
  create a draft release.

This remains a nightly for physical testing on both fast and slow Windows
machines, especially with cold or Bluetooth microphones, before stable
promotion.
