# Terminal pet sprite sheets — attribution & disclaimer

These eight sprite sheets (`codex`, `dewey`, `fireball`, `rocky`, `seedy`,
`stacky`, `bsod`, `null-signal`) are the **OpenAI Codex CLI terminal pets**,
© OpenAI, fetched from OpenAI's public asset CDN
(`https://persistent.oaistatic.com/codex/pets/v1/`). They are NOT original
wispr-fox artwork and are NOT covered by this repository's MIT license.

They are included here purely as a personal/fan-use convenience for this
hobby project, which is shared privately with friends and family. No
affiliation with or endorsement by OpenAI is implied. If you fork or
redistribute wispr-fox, remove this folder or replace the sheets with your
own art. Takedown requests: delete `static/pets/*.webp` and the pet skins
keep working with any custom sheet in the same 8×9 / 192×208 grid.

Sheet format: 1536×1872 WEBP, 8 columns × 9 rows of 192×208 frames.
Rows: idle, run-right, run-left, waving, jumping, sad, waiting, typing,
celebration. Frame counts per row: 6 / 8 / 8 / 4 / 5 / 8 / 6 / 6 / 6.

## Custom Codex v2 pet

`mochi-marmalade.webp` is the owner's Codex-authored hatch-pet, created from
their cat reference photograph. Its validated production atlas is included
unchanged: 1536×2288 WEBP, 8 columns × 11 rows, SHA-256
`5609A9398A9C7CF6CEBA7A337DCE44AB82A08170E7C93B355DE43B76BF4487C8`.
Rows 0–8 provide the standard animation contract. Rows 9–10 preserve sixteen
clockwise look directions for future pointer-aware rendering. It is generated
for this personal project and is not covered by the OpenAI terminal-pet note
above.
