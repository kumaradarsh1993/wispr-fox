# wispr-fox Avatar SDK

> **Current implementation note (`v1.4.0-nightly.11`).** The high-fidelity
> Codex-authored skins use manifest-v2 raster state packs, not hand-coded SVG.
> Their assets live in `static/avatars/<id>/`, metadata lives in
> `src/lib/avatar-packs.ts`, and the live floater renderer is
> `src/lib/RasterAvatar.svelte`. The older SVG package contract below remains
> valid for lightweight vector avatars, but raster is now the preferred path
> for polished illustrated mascots.

> **Purpose of this document.** This is the contract for creating new
> floater avatars for wispr-fox. It's written so that another AI agent
> (Gemini CLI, a fresh Claude session, etc.) — or a human designer —
> can produce a complete, working avatar package without needing to
> read the rest of the codebase. Hand the agent this file plus the
> idea ("make a Pikachu") and you should get back a folder that drops
> into wispr-fox's avatars directory and Just Works.
>
> **Legacy SVG status.** The SVG package contract below remains a
> compatibility target for lightweight avatars. Built-in high-fidelity skins
> now use the raster state-pack path described above; user-installed package
> loading and manager UI remain future work.

---

## 1. What an avatar is

wispr-fox shows a small character in a 190×210-pixel transparent
always-on-top window (the "floater"). The character reacts to the
dictation pipeline:

| State | When | Default dwell |
|---|---|---|
| `idle` | Doing nothing. Default pose. | — |
| `listening` | Microphone is recording. | as long as user holds the hotkey |
| `thinking` | Audio captured, STT running. | ≥ 1.4 s (forced minimum) |
| `writing` | STT done, LLM polishing/drafting. | ≥ 1.4 s |
| `pasting` | LLM done, text being injected into the target app. | ≥ 1.4 s |

Plus three runtime behaviours that any avatar may opt into:

- **Eye tracking** — pupils follow the user's cursor while it's over the floater.
- **Blink** — eyes close briefly every 2–4 s.
- **Hover** — visual reaction when the cursor enters the floater window.

An avatar is a folder of static files. No JavaScript, no runtime code.
The wispr-fox renderer reads the manifest, picks the right SVG for the
current state, and sets CSS variables that the SVG / accompanying
stylesheet can use for interactivity.

---

## 2. File layout

A complete avatar package looks like this:

```
my-avatar/
├── avatar.json           ← manifest (required)
├── icon.svg              ← thumbnail for the picker (required)
├── states/               ← state SVGs (idle is required, others optional)
│   ├── idle.svg
│   ├── listening.svg
│   ├── thinking.svg
│   ├── writing.svg
│   └── pasting.svg
├── animations.css        ← keyframes + state-specific motion (optional)
├── README.md             ← human-readable description (optional)
└── LICENSE               ← asset license (optional but recommended)
```

**Folder name** = the avatar's `id` in the manifest. Use kebab-case
(`pikachu`, `pixel-cat`, `bowie-fox-2`). The id is what the renderer
uses internally; collisions are not allowed.

**File sizes.** Keep individual SVGs under 50 KB and the whole package
under 500 KB. The floater webview re-decodes assets on every state
swap, so bloated SVGs feel laggy.

---

## 3. Manifest (`avatar.json`)

A minimal manifest:

```json
{
  "manifestVersion": 1,
  "id": "pikachu",
  "name": "Pikachu",
  "description": "Lightning mouse — Pokémon-style mascot",
  "author": "anon",
  "version": "1.0.0",
  "license": "CC-BY-4.0",
  "viewBox": "-10 -10 160 180",
  "anchor": "bottom-center",
  "states": {
    "idle":      "states/idle.svg",
    "listening": "states/listening.svg",
    "thinking":  "states/thinking.svg",
    "writing":   "states/writing.svg",
    "pasting":   "states/pasting.svg"
  }
}
```

### Full manifest schema

| Field | Type | Required | Notes |
|---|---|---|---|
| `manifestVersion` | int | yes | Always `1` for now. |
| `id` | string | yes | kebab-case, must match folder name, unique. |
| `name` | string | yes | Human-readable. Shown in the picker. Max 24 chars. |
| `description` | string | yes | One short sentence. Shown as hover tooltip. Max 120 chars. |
| `author` | string | yes | Free text. |
| `version` | string | yes | Semver. The avatar manager uses this to decide whether an update is available. |
| `license` | string | recommended | SPDX identifier (`MIT`, `CC-BY-4.0`, `Apache-2.0`, etc.) or a plain string. |
| `homepage` | string | optional | URL the manager links to. |
| `viewBox` | string | yes | The shared SVG viewBox for all state files. Recommended: `"-10 -10 160 180"` (matches built-in skins). |
| `anchor` | string | optional | Where the character sits in the window. `"bottom-center"` (default), `"bottom-left"`, `"bottom-right"`, `"center"`. |
| `scale` | number | optional | Multiplier applied via CSS transform. Default `1.0`. Use to size your art up/down without redrawing. |
| `states` | object | yes | Map of state → relative path to SVG file. Only `idle` is mandatory; missing states fall back to `idle`. |
| `bubble` | object | optional | Custom bubble colours (see §6). |
| `interactivity` | object | optional | Opt-in flags for eye tracking, blink, hover. Defaults: `eyeTracking: false`, `blink: false`, `hover: false`. |
| `animationsCss` | string | optional | Relative path to a CSS file (default `"animations.css"` if it exists). Loaded once per avatar; scoped via a wrapper class. |

### About paths

All paths in `states.*` and `animationsCss` are **relative to the
manifest's directory**. Don't use absolute paths or `..`. The loader
will reject them.

---

## 4. State SVGs — the rendering contract

### Coordinate system

All state SVGs share **one viewBox** declared in the manifest. The
recommended viewBox is `-10 -10 160 180` (matches the built-in skins,
gives a usable 140×160 drawing area with 10 px breathing room on top
and the sides). The character is anchored to the bottom of the
viewport via the `anchor` setting; in practice, draw your character
filling roughly `(20, 20) → (140, 170)`.

### Root `<svg>` rules

- Every state SVG **must** be a complete standalone SVG document.
- The root `<svg>` element **must not** declare a `viewBox` — the
  renderer wraps your SVG inside its own container that supplies the
  manifest's viewBox. Same goes for `width` / `height`.
- The renderer will strip any `xmlns` you supply and re-add a clean
  one. You don't need to add `xmlns:xlink`.
- The renderer will add `data-state="<state-name>"` and (optionally)
  `class="character avatar-<id>-skin"` as attributes on the root.

### Naming groups

Wrap each logical body part in a `<g class="…">` so animations can
target them:

```svg
<g class="body">    ... </g>
<g class="head">    ... </g>
<g class="left-eye">... </g>
<g class="right-eye">…</g>
<g class="mouth">   ... </g>
<g class="left-arm"> ... </g>
<g class="right-arm">... </g>
<g class="tail">    ... </g>   <!-- optional -->
<g class="accent">  ... </g>   <!-- thunderbolts, sparkles, glasses, etc. -->
```

These class names aren't enforced, but they're the convention CSS in
`animations.css` should target. Pick clear semantic names — your
animations will reference them directly.

### Don'ts

- **No external image references** (`<image href="https://...">` is
  forbidden — the CSP blocks it anyway).
- **No JavaScript** inside `<script>` tags.
- **No CSS `@import`** rules.
- **No raster `<image>` embeds** larger than 8 KB total per SVG. If
  you need a photo, encode it as `data:image/png;base64,...` and keep
  it tiny.
- **No fonts loaded via CSS.** Use SVG `<text>` with `font-family`
  set to safe system fallbacks (`"ui-sans-serif, system-ui, sans-serif"`)
  or convert text to paths.

---

## 5. Animations (`animations.css`)

The renderer concatenates all state SVGs into the floater DOM and
swaps them via `display: block` / `display: none` based on
`data-state`. Animations should live in `animations.css` (or be
inlined as `<style>` blocks inside each state SVG — both work, the
external file is just cleaner for editing).

### Scoping

The loader wraps your CSS so every selector you write is implicitly
scoped to `.avatar-<id>-skin`. **Write your selectors as if your
avatar is the only thing on the page** — don't prefix anything
yourself. The loader does it.

So given `id: "pikachu"` and a CSS file containing:

```css
.body { animation: idle-breathe 3.6s ease-in-out infinite; }
[data-state="listening"] .left-ear { animation: ear-perk 0.4s both; }
@keyframes idle-breathe { … }
@keyframes ear-perk { … }
```

…the runtime emits, after scoping:

```css
.avatar-pikachu-skin .body { … }
.avatar-pikachu-skin[data-state="listening"] .left-ear { … }
@keyframes pikachu__idle-breathe { … }
@keyframes pikachu__ear-perk { … }
```

(Keyframe names are prefixed so two avatars can't collide.)

### Hooks the runtime sets

The renderer exposes a handful of CSS custom properties on the root
`.avatar-<id>-skin` element that you can read in your CSS / SVG:

| Property | Range | Description |
|---|---|---|
| `--eye-shift-x` | `-3.5` … `3.5` (SVG units) | Horizontal pupil offset for cursor tracking. Apply via `transform: translateX(var(--eye-shift-x))`. |
| `--eye-shift-y` | `-2.5` … `2.5` | Vertical pupil offset. |
| `--blink` | `0` or `1` | `1` = eyes open, `0` = closed. Drive eye height with `transform: scaleY(var(--blink))`. |
| `--hover` | `0` or `1` | `1` while the cursor is over the window. Use for "I see you" reactions. |
| `--mode` | `"light"` or `"drafting"` | Current dictation mode. Use to tint accents (drafting could be purple, light blue, etc.). Read with attribute selectors: `[data-mode="drafting"] .body { fill: purple; }`. |

These are **only set** if you opt into them via the manifest's
`interactivity` block. Otherwise they're absent and your SVG sees
nothing.

### Minimum-dwell rule

State transitions for `thinking`, `writing`, and `pasting` are forced
to stay on screen at least 1.4 seconds even if the backend finishes
sooner. Don't write animations that need longer than ~1.4 s to play
cleanly, or they'll be cut off when the next state arrives. For loops
(`idle-breathe`, `listening-tilt`), there's no upper bound — they can
run forever.

---

## 6. Bubble styling (`bubble` in manifest)

The floater shows a small speech-bubble during recording / thinking /
writing / pasting with status text ("Listening…", "Thinking…",
"Pasted ✓"). Avatars can customise its colours so the bubble feels
like part of the character:

```json
"bubble": {
  "background": "#FFF9DD",
  "color":      "#2A2200",
  "border":     "rgba(220, 180, 0, 0.35)",
  "shadow":     "0 4px 12px rgba(220, 180, 0, 0.18)",
  "eqColor":    "#FFCC00",
  "dotsColor":  "#9A8800"
}
```

| Field | Required | Notes |
|---|---|---|
| `background` | yes | Bubble fill. Any CSS colour. |
| `color` | yes | Text colour. Pick something with ≥ 4.5:1 contrast against `background`. |
| `border` | optional | Border colour (often rgba with low alpha). Defaults to a faint grey. |
| `shadow` | optional | Full CSS `box-shadow` value. Defaults to a generic soft shadow. |
| `eqColor` | optional | Colour of the equaliser bars shown while recording. Defaults to a dark grey. |
| `dotsColor` | optional | Colour of the typing-dots shown while writing/pasting. Defaults to grey. |

Skip the field entirely (or set the whole `bubble` object to `null`)
to use the default Foxy cream + orange theme.

---

## 7. Picker thumbnail (`icon.svg`)

Required. Shown in the sidebar and the Avatar tile picker in
Settings → Appearance.

- viewBox: `0 0 70 70` (square).
- Render the **head only** in most cases — the body cropped at this
  size reads as visual noise.
- Use the same palette as your full character so the picker feels
  consistent.
- Keep it under 5 KB.

If `icon.svg` is missing, the loader uses the `idle.svg` state file
scaled to fit. That's a lazy fallback; ship a proper icon for any
avatar you want to look polished.

---

## 8. Worked example: "Pikachu" minimum viable avatar

```
pikachu/
├── avatar.json
├── icon.svg
└── states/
    └── idle.svg
```

```json
// avatar.json
{
  "manifestVersion": 1,
  "id": "pikachu",
  "name": "Pikachu",
  "description": "Yellow electric mouse — Pokémon style",
  "author": "demo",
  "version": "0.1.0",
  "license": "CC-BY-NC-4.0 (fan-art)",
  "viewBox": "-10 -10 160 180",
  "anchor": "bottom-center",
  "states": {
    "idle": "states/idle.svg"
  },
  "bubble": {
    "background": "#FFF7C0",
    "color":      "#3A2A00",
    "border":     "rgba(255, 200, 0, 0.4)",
    "eqColor":    "#F4B400"
  }
}
```

```svg
<!-- states/idle.svg -->
<svg xmlns="http://www.w3.org/2000/svg">
  <g class="body">
    <ellipse cx="70" cy="110" rx="38" ry="32" fill="#FFD600" stroke="#5A4500" stroke-width="2"/>
  </g>
  <g class="head">
    <circle cx="70" cy="60" r="28" fill="#FFD600" stroke="#5A4500" stroke-width="2"/>
    <ellipse cx="62" cy="58" rx="6" ry="7" fill="#FFFFFF"/>
    <circle cx="62" cy="60" r="3" fill="#1a1a1a"/>
    <ellipse cx="78" cy="58" rx="6" ry="7" fill="#FFFFFF"/>
    <circle cx="78" cy="60" r="3" fill="#1a1a1a"/>
    <ellipse cx="50" cy="68" rx="4" ry="2.5" fill="#FF7070" opacity="0.7"/>
    <ellipse cx="90" cy="68" rx="4" ry="2.5" fill="#FF7070" opacity="0.7"/>
  </g>
  <g class="ear left">
    <path d="M 50 40 L 38 8 L 58 32 Z" fill="#FFD600" stroke="#5A4500" stroke-width="2"/>
    <path d="M 38 8 L 38 18 L 44 18 Z" fill="#1a1a1a"/>
  </g>
  <g class="ear right">
    <path d="M 90 40 L 102 8 L 82 32 Z" fill="#FFD600" stroke="#5A4500" stroke-width="2"/>
    <path d="M 102 8 L 102 18 L 96 18 Z" fill="#1a1a1a"/>
  </g>
</svg>
```

That's enough to ship. Add `listening.svg` / `thinking.svg` / etc. to
make it react to the pipeline.

---

## 9. Installation (until the manager UI ships)

Drop your `pikachu/` folder into:

| Platform | Location |
|---|---|
| Windows | `%APPDATA%\com.wispr-fox.app\avatars\pikachu\` |
| macOS | `~/Library/Application Support/com.wispr-fox.app/avatars/pikachu/` |
| Linux | `~/.local/share/com.wispr-fox.app/avatars/pikachu/` |

Restart wispr-fox. The avatar should appear in the sidebar picker
and in Settings → Appearance → Floater character.

Once the avatar manager UI ships (roadmapped for `v1.2.0-nightly.x`),
this will become drag-and-drop / "+" button in the sidebar.

---

## 10. Validation checklist

Before you ship, verify every box:

- [ ] Folder name matches `id` in manifest exactly.
- [ ] `avatar.json` parses as valid JSON.
- [ ] `manifestVersion` is `1`.
- [ ] At least `states.idle` is present and resolves to a real file.
- [ ] Every referenced state file exists.
- [ ] Every state SVG is well-formed XML with a single root `<svg>`.
- [ ] No state SVG declares its own `viewBox`, `width`, or `height`.
- [ ] No `<script>` tags, no `@import`, no external `<image>` references.
- [ ] `icon.svg` exists and is square (70×70 viewBox).
- [ ] `bubble.color` has ≥ 4.5:1 contrast against `bubble.background`.
- [ ] All animations finish within 1.4 s (non-loop states), or loop
      cleanly (idle / listening).
- [ ] Optional: `LICENSE` file or SPDX identifier in `license` field.

---

## 11. What this spec doesn't cover (yet)

Update for `v1.4.0-nightly.11`: single-frame raster state packs are now
implemented for built-in avatars through manifest version 2. Multi-frame
sprites, Lottie, Live2D, and real-time 3D models remain future work.

- **Multi-frame raster avatars** (animated sprite sheets with several frames
  per state). Single-image raster state packs are implemented; full frame
  timelines still need a separate runtime contract.
- **Lottie / Bodymovin avatars**. Same story.
- **Live2D models**. Roadmapped but a separate spec — Live2D needs
  its own runtime initialisation and licence handling.
- **Sound effects** per state. Currently the start/stop tones come
  from `~/Library/Application Support/.../sounds/`; per-avatar sound
  isn't supported.
- **Real-time 3D avatars** (`.glb`/Three.js scene packs). The current raster
  packs can use a 2.5D/3D-rendered art style, but they are still images.

If you're authoring something that needs any of those, leave a comment
on the GitHub repo (kumaradarsh1993/wispr-fox) so we can prioritise
the spec.

---

## 12. License / attribution

Avatars you author are your own work — you set the licence in
`avatar.json`. wispr-fox itself is MIT-licensed.

If you ship an avatar derived from a copyrighted character (Pikachu,
Mickey Mouse, etc.), include a fan-art note in the description and
set `license` appropriately (`"CC-BY-NC-4.0 (fan-art)"`,
`"Personal use only"`, etc.). The avatar manager will show this to
the user before installing.

---

*Document version: 1.0 — frozen on v1.1.0-nightly.5 ship-day.*
*The contract above won't break in patch / minor releases. If we ever*
*bump `manifestVersion` to 2, this doc updates with a migration guide.*
