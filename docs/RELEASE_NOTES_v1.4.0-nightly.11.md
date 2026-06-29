# wispr-fox v1.4.0-nightly.11 - Codex raster avatar correction

This is a Codex-authored nightly that corrects the avatar-quality gap from
nightly.10. The generated concept art is now used directly as the live avatar
artwork instead of being approximated with hand-coded SVG.

## What changed

- Added a raster avatar-pack renderer for high-fidelity built-in mascots.
- Converted the Codex Fox, Oru & Gujia, and Spark Buddy concept sheets into
  transparent per-state PNG assets.
- Added eight poses per raster avatar: idle, listening, thinking, writing,
  pasting, error, sleeping, and excited.
- Updated Settings, sidebar, and picker previews to show the actual raster art.
- Preserved all existing avatar paths: Fox PNGs, Paperclip, Clippy, Desk Cat,
  and Khaumani & Indy remain backward-compatible.
- Updated the avatar SDK notes so future work treats raster packs as the
  preferred path for polished illustrated mascots.

## Notes

These are 2.5D/raster avatar packs, not real-time 3D models. They are designed
to look richer and more dimensional while staying lightweight inside the
existing floater window.
