# wispr-fox v3.2.0-nightly.2 — Codex voice & avatar refinement

This Codex-authored nightly deepens the visual overhaul with a personal voice
signature, a compact-window Settings fix, and a more coherent avatar gallery.

## Your recent voice signature

- Insights now describes how your recent dictation tends to move: pace and
  consistency, session shape, sentence shape, question share, vocabulary
  breadth, recurring discourse markers, and adjacent repeated-word signals.
- The analysis is computed locally from retained raw microphone transcripts.
  Uploads, meeting speakers, and AI-cleaned or drafted output are excluded so
  the result reflects your own dictation rather than transformed text.
- A minimum sample protects against confident-looking conclusions from too
  little speech. The interface also states the limits clearly: transcript-only
  analysis cannot honestly grade accent, pronunciation, or vocal fluency.
- Lifetime totals now say “words delivered” instead of presenting mixed
  productivity data as speaking speed.

## Settings that fits the window

- The large blank band that appeared above Settings content in a half-width
  window is gone. The compact header no longer turns a desktop horizontal flex
  basis into unwanted vertical space.
- Header, horizontal navigation, and content spacing were checked at both
  1200×760 and 1000×700, while the full-window layout remains intact.

## A clearer avatar gallery

- Avatar names are now consistent everywhere: Clippo, Clippy, Blacky,
  Uru & Gujia, Mochi & Marmalade, Pikachu, Wavy, and Siri.
- “Companion” is now “Avatar” across the app, and one catalog drives the main
  shell, Settings, and the floater menu so the three surfaces cannot drift.
- Fox, Clippo, Blacky, Wavy, and Siri have more representative picker art.
  Clippy and the Codex-inherited raster packs remain untouched.
- Mochi & Marmalade, the newer Codex-created two-cat pet, joins the app using
  its validated v2 sprite atlas byte-for-byte, including all look directions.

## Validation

- Svelte diagnostics: 0 errors, 0 warnings.
- Production frontend build: passed.
- Rust desktop check: passed with only pre-existing warnings.
- Mochi & Marmalade source and packaged sprite hashes: identical.

This remains a nightly release. Stable stays on v3.1.0 until an explicit
promotion signal.
