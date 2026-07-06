# Pixel pets — ingestion plan (drafted 2026-07-06)

The user wants Codex-CLI-style animated pixel pets as wispr-fox avatars.
This doc records everything reverse-engineered about the Codex pet format
and the plan to support it. **No third-party assets live in this repo** —
see "Licensing" below.

## What Codex pets actually are (reverse-engineered 2026-07-06)

- Codex CLI (Rust TUI) renders pets in the terminal via Kitty
  graphics / Sixel / iTerm2 image protocols. Source modules (from binary
  strings): `tui/src/pets/{model,sixel,image_protocol,picker}.rs`.
- **Assets are WEBP sprite sheets** downloaded at runtime from a public CDN:
  `https://persistent.oaistatic.com/codex/pets/v1/<id>-spritesheet-v4.webp`
- **Built-in catalog (v4)**: `codex` (cloud-headed terminal robot), `dewey`
  (duck), `fireball` (flame head), `rocky` (rock), `seedy` (sprout),
  `stacky` (stack), `bsod` (blue-screen gremlin), `null-signal` (void).
- **Sheet geometry**: all sheets are 1536×1872 = **8 columns × 9 rows of
  192×208 frames** (verified by cropping). Rows are animations; short rows
  pad with empty frames.
- **Animation names** (from the binary): `running-right`, `running-left`,
  `waving`, `jumping`, `failed`, `waiting`, `running`, `wave`, `bounce`,
  `sad` — plus idle. Fireball's rows observed: idle/blink, run-right,
  run-left, waving, jumping, sad/failed, waiting/thinking, **typing at a
  laptop**, celebration.
- **Custom pet format**: a folder with `pet.json` (or `avatar.json`) +
  `spritesheet.webp`. `PetFile` fields: `displayName`, `spritesheetPath`,
  frame spec (`frame_width`, `frame_height`, `columns`, `rows`,
  `frame_count`) and `animations` (name → frame indices + fps; fps must be
  finite and bounded). Custom pets live under `~/.codex/pets/` (empty on
  this machine). Local copies of all 8 sheets: `Codex pets_ 6 Jul 2026/`
  (gitignored).

## State mapping (pet animation → wispr-fox floater state)

| wispr-fox state | pet animation |
|---|---|
| idle | idle/blink row (+ occasional `waiting`) |
| listening | `waving` on enter, then attentive idle |
| thinking | `waiting` (head-scratch row) |
| writing | typing row (the laptop one) |
| pasting (success) | celebration row (`jumping`/`bounce`) |
| error | `sad` / `failed` row |
| enter/exit | `jumping` in, `sad`-less shrink out (SDK hook classes) |

## Implementation plan (next nightly-sized batch)

1. **`SpritePet.svelte` renderer** in `src/lib/`: one `<div>` with the
   sheet as `background-image`, `background-position` stepped per frame
   from a rAF clock (or CSS `steps()` per animation). Props: manifest +
   current floater state. Display at 96–128px logical (192×208 source,
   `image-rendering: pixelated`).
2. **Manifest**: adopt the Codex `pet.json` field names verbatim so any
   Codex custom pet drops in unchanged. Extend AVATAR_SDK manifest v2 with
   `type: "sprite-pet"` alongside the existing raster state packs.
3. **Loading**: read pet folders from `%APPDATA%/com.wispr-fox.app/avatars/`
   (needs a small Rust `read_avatar_pack` command or fs-scope for that dir
   — keep the Tauri-security baseline: explicit scope, no wildcard fs).
   Picker shows any installed pet next to built-in skins.
4. **Getting the Codex pets in**: ship an **importer**, not the assets — a
   Settings → Appearance "Import pet" button that (a) picks a local folder
   (Codex custom-pet format), or (b) offers "Fetch the Codex pets" which
   downloads from the public CDN URLs above into appdata on the user's own
   machine, with an attribution note. Repo stays clean.
5. **Original pet (shippable)**: commission/generate ONE original pixel pet
   in the same 8×9 grid — an orange pixel fox ("Foxel") with the same nine
   animation rows — and bundle it as a built-in. This is the only pet that
   ships in the repo/installers.

## Licensing

The 8 Codex sheets are OpenAI-copyrighted art served from their public
CDN. Fine to download/use locally on the user's machine; **do NOT commit
them or bundle them in release artifacts.** `.gitignore` blocks
`/Codex pets_*/`. The importer approach (user-initiated download at
runtime) keeps distribution on OpenAI's side.
