# wispr-fox v1.4.0-nightly.10 - Codex avatar pack

This is a Codex-authored nightly focused on richer avatar options and the
first packaged AI-generated avatar concept assets.

## What changed

- Added three selectable avatars:
  - Codex Fox: a higher-fidelity fox companion with Codex-blue glow accents.
  - Oru & Gujia: a richer two-cat duo based on the user's orange tabby Oru and
    white cat Gujia.
  - Spark Buddy: an original electric companion with lightning ears and teal
    glow, deliberately not a franchise character.
- Added state-aware SVG animation for the new avatars: idle breathing, listening
  bounce/waves, thinking effects, writing/tapping, paste celebration, hover
  reactions, and the existing phew transition.
- Added avatar-specific hover quips and matching bubble themes.
- Wired the new skins into Settings -> Avatar, the sidebar picker, and the
  floater right-click menu.
- Tightened the expanded sidebar avatar picker at short window heights so all
  9 avatar buttons sit above the anchored usage block.
- Copied the generated concept sheets into `static/avatar-concepts/` so the art
  direction travels with the repository.
- Updated the avatar SDK note to mention the new `RichAvatar.svelte` built-in
  renderer location.

## Notes

Cat lab remains retired from selectable UI. Old saved `cat-lab` values still
migrate to Desk Cat for safety.
