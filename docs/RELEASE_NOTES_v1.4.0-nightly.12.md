# wispr-fox v1.4.0-nightly.12 - Codex raster avatar QA

This Codex-authored nightly fixes the raster avatar regressions reported after
nightly.11.

## What changed

- Raster avatars now honor the floater S/M/L scale setting. The window and the
  image scale together instead of the image staying at the original size.
- Raster avatars no longer receive the legacy SVG whole-character animations.
  This removes the extra rotate/roll motion that pushed art into the floater
  edge.
- Added an internal safe frame around raster state images so hover, bounce,
  signal waves, and drop shadows do not crop against the window border.
- Moved the speech-bubble anchor higher for Codex Fox, Oru & Gujia, and Spark
  Buddy so provider labels sit above the head instead of over the face.
- Cleaned stray edge-touching PNG fragments from affected raster states,
  including the duplicated right-edge slivers visible in the screenshots.
- Regenerated the Oru & Gujia state pack from a Codex chroma-key sheet so the
  white cat no longer inherits transparent/matte holes from white-background
  removal. The updated pack includes cleaner state PNGs and a taller art frame.

## Verification

- `npm run check` passed with 0 errors. Existing unrelated warnings remain.
- `npm run build` passed. Existing unrelated bundle/a11y warnings remain.
- A local raster QA contact sheet verified 80%, 100%, and 125% scale framing
  for Codex Fox, Oru & Gujia, and Spark Buddy.
- A component scan found no secondary edge-touching alpha islands in the raster
  state PNGs after cleanup.
- A dark-background preview verified all eight regenerated Oru & Gujia states.
