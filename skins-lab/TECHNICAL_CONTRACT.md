# Technical contract — what the runtime can and cannot digest

This is the **strict contract** between the wispr-fox runtime and any
new skin. If your asset violates anything here, integration will fail.

For the long-form version with examples and integration steps, see
[../skins/SPEC.md](../skins/SPEC.md). This document is the
"can-digest / can't-digest" summary the user asked for.

---

## Inputs the runtime pushes to your skin

These are the **only** signals you get. Your animation system must
respond to them and nothing else.

| Input | Type | Values | Meaning |
|---|---|---|---|
| `state` | enum | `idle` / `listening` / `thinking` / `writing` / `pasting` | The current phase of the dictation flow. |
| `mode` | enum | `light` / `advanced` | Which hotkey class was pressed. Used for visual variation (e.g. wizard hat in `advanced`). |
| `phewActive` | boolean | `true` for ~700ms when listening ends | A relief / "got it" transient. Overlay-able. |
| `blinkOpen` | boolean | toggled every 2-3s | Drives eye open/closed if the character has eyes. |
| `lookDir` | enum | `left` / `right` / `center` | Subtle eye-direction during idle. Optional to use. |

There is **no** hover, click, mouse-position, audio-level, or
text-input signal. Your character does not react to the user moving
the mouse. The runtime does not pass amplitude data during listening
(no waveform-driven mouths — sorry).

## The 5 states — what each one means

| State | Triggered when | Duration |
|---|---|---|
| `idle` | App is doing nothing | Unbounded |
| `listening` | User is holding the dictation hotkey | Unbounded (1s to 30s typical) |
| `thinking` | Audio is being transcribed by Whisper | ≥1.4s guaranteed, typically 1-3s |
| `writing` | LLM is cleaning / drafting the transcript | ≥1.4s guaranteed, typically 1-5s |
| `pasting` | Final text is being injected into focused app | ~1s, then back to `idle` |

The **≥1.4s min-dwell** on the last three states is enforced by the
host. You do not need to debounce. Design loops, not bursts.

## File formats the runtime CAN digest

Three, in order of preference for new work:

### 1. Rive (`.riv`) — recommended for new skins

- **One file per character.**
- State machine name: **`"Floater"`** (exact string).
- State machine inputs (exact names — case-sensitive):
  - `stateNum` (number, 0–4) — `0=idle, 1=listening, 2=thinking, 3=writing, 4=pasting`
  - `modeNum` (number, 0–1) — `0=light, 1=advanced`
  - `phew` (trigger) — fired when entering phew
  - `blink` (trigger) — fired on each blink request
- Canvas: **190 × 230 px**, transparent background.
- All assets embedded — no external sprite or image refs.
- File size: ideally < 500 KB.
- Layout: character centered horizontally, anchored bottom. Top ~50px
  must be unobscured (the host paints a speech bubble there).

### 2. Lottie (`.json`)

- One JSON file. Named segments (markers):
  - `idle_loop` — seamless loop
  - `listening_loop` — seamless loop
  - `thinking_loop` — seamless loop
  - `writing_loop` — seamless loop
  - `pasting_once` — one-shot ~1s
  - `phew_once` — one-shot ~700ms (overlay-able)
  - Optional transitions: `idle_to_listening`, `listening_to_thinking`,
    `writing_to_pasting`
- Canvas: 190 × 230 px, transparent.
- The runtime plays segments by exact marker name. Missing markers
  fall back to `idle_loop`. Mis-named markers won't be found and
  the character will appear stuck.
- File size: ideally < 300 KB. Lottie balloons with too many shape
  layers — prefer keyframed transforms over many drawn paths.

### 3. Svelte component (`.svelte`)

- File: `<CharacterName>Skin.svelte`.
- Props (exact names — Svelte 5 `$props()` syntax):
  ```js
  let { state, mode, phewActive, blinkOpen, lookDir } = $props();
  ```
- Renders into a 190 × 230 box. Pure CSS/SVG animation preferred. No
  external runtime dependencies (no `npm install` adds).
- Best for very simple geometric characters where Rive would be
  overkill (e.g. a single-shape mascot).

## File formats the runtime CANNOT digest

These will be **rejected at integration** — do not deliver them:

- **PNG / JPG sprite sheets with separate metadata.** The runtime has
  legacy support for the original Microsoft Clippy sprite only;
  new characters must be vector.
- **GIF.** No state targeting, can't be driven by `stateNum`.
- **MP4 / WebM / any video.** Same problem — no state hook.
- **Three.js / WebGL scenes.** Out of scope for v1; the floater is
  not a 3D context.
- **Spine animations.** No runtime support; the user has not added
  the Spine player to the bundle and won't for one skin.
- **HTML files with embedded `<script>` tags.** The runtime sandboxes
  skins; arbitrary JS inside a skin won't run.
- **Anything > 2 MB.** Hard ceiling. Most skins should be < 500 KB.
- **Files referencing external URLs** (CDN-hosted images, Google
  Fonts, etc.). Skins must be self-contained — the app may be used
  offline.

## Sizing — non-negotiable

- **Window size:** 190 × 230 px (defined in `tauri.conf.json`,
  `clippy` window label).
- **Character footprint:** ~120–160px tall, centered horizontally,
  anchored to bottom. Leave ~50px of top space for the speech bubble.
- **Retina / HiDPI:** the window scales by device pixel ratio.
  Vector formats handle this for free; if you somehow need a raster
  fallback, ship @2x assets.

## Transparency

The floater window is `transparent: true, decorations: false,
alwaysOnTop: true`. Your skin **must not paint a background**. Use
SVG/Rive/Lottie transparency. The user will see whatever desktop
wallpaper, window, or app is behind the character.

## Performance budget

- Must hold 60fps on a modest laptop (2020-era ultrabook).
- Rive and Lottie both have GPU-accelerated renderers in our bundle —
  use them.
- Avoid: > 100 shape layers in Lottie, > 50 bones in Rive, full-canvas
  blur filters in SVG.

## What the host handles — DO NOT replicate

You do not implement, and your asset must not include:

- Window positioning / dragging (host owns it).
- Hide / X button (host overlays on hover).
- State transition logic (host pushes `state` value at you; you react).
- Speech bubble / text labels (host renders above your top 50px).
- Audio cues (host plays start/stop tones via rodio).
- Skin selection UI (host's Settings page).

## When the runtime falls back

The host falls back to the **stylized paperclip** skin if:

- Your file fails to load (404, parse error, missing state machine).
- Your skin throws during render (Svelte) or fails to initialize
  (Rive / Lottie).
- Your skin is selected but the file is missing from disk.

A failed-skin event logs a `tracing::warn` line — useful for the
main agent during integration debugging.

## Mode variations — optional, encouraged

The `mode` input lets you offer two visual flavors of the same
character:

- **`light` mode** — everyday/casual. The default look.
- **`advanced` mode** — heightened/more dramatic. Wizard hat,
  glowing aura, magnifying glass, whatever fits the character.

Mode variation is **optional**. If your skin ignores `mode` entirely,
that's fine — the runtime won't complain. But it adds richness for
free.

---

If something in this contract is unclear or appears to conflict with
`../skins/SPEC.md`, **flag it before building**. Don't guess; the
cost of rebuilding a finished asset is much higher than a 1-minute
clarification.
