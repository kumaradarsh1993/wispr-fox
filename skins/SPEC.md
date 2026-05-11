# wispr-fox Floater Skin Spec

A "skin" is the visual character that lives in the floating always-on-top window of wispr-fox. The app already supports four skins (`stylized`, `real-clippy`, `chippy`, plus `off`). This document is the contract for adding new skins — written so a freelance animator, a Rive author, or a separate Claude Code agent can produce one without touching the rest of the app.

## What a skin must respond to

Skins are driven by **5 state values** that the wispr-fox app pushes in. A skin must respond gracefully to all of them. Treat these as inputs to your state machine:

| Input | Type | Values | When it changes |
|---|---|---|---|
| `state` | enum | `idle` / `listening` / `thinking` / `writing` / `pasting` | App lifecycle. See below. |
| `mode` | enum | `light` / `advanced` | Which hotkey was pressed. Used for visual variations. |
| `phewActive` | boolean | `true` for ~700ms after listening ends | Trigger a brief relief/sigh moment. |
| `blinkOpen` | boolean | toggled by the host every 2-3s | Drives eye open/closed if your character has eyes. |
| `lookDir` | enum | `left` / `right` / `center` | Subtle eye-direction during idle. Optional. |

### State semantics — what each one means

- **`idle`**: Nothing's happening. The character is in its resting pose. Subtle ambient motion (breathing, gentle bob, occasional look-around) is welcome but optional.
- **`listening`**: User is currently holding the dictation hotkey. Character should be **clearly engaged with the user** — looking toward the camera, cupping an ear, leaning in, etc. Should be visibly different from idle.
- **`thinking`**: Audio is being transcribed by the speech-to-text API. Character should look like it's processing — eyes up, thought bubble, gears turning, etc.
  - In `advanced` mode, this state can show a more dramatic "heavy thinking" variant (brain bubble, sparkles, etc.)
- **`writing`**: Transcript is being cleaned up by the LLM. Character should look like it's polishing/refining the output — pen + paper, typewriter, etc.
  - In `advanced` mode, this can be a "magical transformation" variant (wand, sparkles, robe).
- **`pasting`**: Final text is being pasted into the user's focused window. Brief celebration moment (~1 second). Then back to idle.

### Pacing guarantees from the host

The host enforces a **minimum 1.4 second dwell** on `thinking`, `writing`, and `pasting`. So your animations for those states have at least 1.4s to play. Don't make them shorter than that or they'll feel rushed. Don't make them so long they can't loop seamlessly past 1.4s either — design for a "play once, loop tail" pattern.

`listening` is unbounded — could be 1 second or 30 seconds depending on how long the user holds the hotkey.

## Output / artifact requirements

A skin must be delivered as one of:

### Option A: Svelte component (`*.svelte`)
- File: `src/lib/skins/<name>.svelte`
- Accepts props: `{ state, mode, phewActive, blinkOpen, lookDir }`
- Renders the character (SVG / canvas / divs — anything)
- Pure CSS/SVG animations preferred; no external runtime deps
- Must fit inside a **190 × 230 px window** (drag/X-button area, transparent background, no decorations)
- Character itself should ideally be ~120-160px tall, centered horizontally, anchored bottom

### Option B: Rive file (`.riv`)
- File: `static/skins/<name>.riv`
- State machine name: `"Floater"`
- State machine inputs (must use these exact names):
  - `stateNum` (number, 0-4): 0=idle, 1=listening, 2=thinking, 3=writing, 4=pasting
  - `modeNum` (number, 0-1): 0=light, 1=advanced
  - `phew` (trigger): fired once when entering phew transient
  - `blink` (trigger): fired on each blink request
- Canvas dimensions: 190 × 230 (matches window)
- Background must be transparent
- All assets embedded in the .riv (no external sprite refs)

### Option C: Lottie JSON (`.json`)
- File: `static/skins/<name>.json`
- Lottie file with named segments, one per state:
  - `idle_loop` — seamless loop
  - `listening_loop` — seamless loop
  - `thinking_loop` — seamless loop
  - `writing_loop` — seamless loop
  - `pasting_once` — one-shot, ~1s
  - `phew_once` — one-shot, ~700ms (overlay-able on others)
  - `idle_to_listening`, `listening_to_thinking`, `writing_to_pasting` — optional transition clips
- The host plays segments by name. If a segment doesn't exist, falls back to `idle_loop`.

## Visual constraints

- **Transparent background** — the window is frameless and transparent. Don't paint a background.
- **Drop-shadow OK** — adds depth, especially helpful on dark wallpapers.
- **Visibility on any background** — assume the character will be placed on white, black, photos, and gradient wallpapers. Use a soft white halo / outer glow on dark elements.
- **No mouse interaction needed** — the entire window catches drag events for moving. The character doesn't need to respond to hover/click.
- **Performance budget** — should run at 60fps on a modest laptop. SVG with reasonable number of paths is fine. Lottie/Rive will outperform SVG for complex animations.

## What the host handles for you

You don't have to:
- Position the window (the host does — user can drag it)
- Render a hide/X button (host overlays one on hover)
- Detect state changes (host pushes them in)
- Handle the speech bubble (host renders text labels above your character; leave the top ~50px of the canvas unobscured)

## Visual variation guidance (for richness)

The user has explicitly asked for the same state to **not always look identical**. To support that:

- Have 2-3 thematic variants of `listening` (e.g., ear-cup, head-tilt, hand-to-ear) and cycle them
- Have an "entry" beat when transitioning into a state — small flourish for the first 0.3s
- For `idle`, occasional micro-actions every 5-10s (look around, stretch, fidget) make a huge difference
- Each character should have **personality** — a paperclip behaves differently from a potato chip from a wizard

## Examples in this repo

- `src/routes/clippy/+page.svelte` — the current `stylized` skin (SVG + CSS, hand-authored). Has all the states implemented inline. Use as reference for what the props look like in practice.
- `src/lib/clippyjs-vendor/clippy.js` + `clippy-agent.js` — the real Microsoft Clippy via vendored clippyts. Sprite-based, very different approach.

## Themes the user has requested

In priority order:

1. **Realistic Pringles** — 3D-rendered or hand-drawn realistic potato chip, slightly cartoonish but with proper shading/highlights. Variants for lounging on a sofa, dressing up, etc.
2. **Classic Clippy improved** — keep the Office Assistant feel but freshen the proportions and add proper state animations beyond the basic sprite set
3. **Pet variant** — a small character (cat, dog, hamster) that responds attentively when listening

## Hiring path — practical notes

- **Fiverr search terms**: "Rive animator", "Lottie animator character", "SVG character animation"
- **Specify in your brief**: states list (copy from this doc), file size budget (< 500KB ideal), turnaround
- **Rive freelancers** typically $50-200 per character with 4-5 states
- **Lottie** is cheaper, $20-100 typical
- Ask for a **work-in-progress preview** at the 50% mark — easier to course-correct
- **Provide visual references** for character appearance (drawings, photos, mood boards)

## Integration: how the host loads a new skin

1. Drop the file into the right folder (Svelte → `src/lib/skins/`, Rive → `static/skins/`, Lottie → `static/skins/`)
2. Add an entry to `src/lib/skin-store.svelte.ts`'s `Skin` type (e.g., `"pringles-realistic"`)
3. Add an entry to the sidebar's `SKIN_OPTIONS` in `src/routes/+layout.svelte`
4. Add a branch in `src/routes/clippy/+page.svelte`'s skin template to render it

(Steps 2-4 are mechanical — the wispr-fox app maintainer handles them. The animator just delivers the file.)

## Questions for the animator to ask before starting

1. Should the character have a "personality moment" on first show? (e.g., entrance wave)
2. Is there a sound that should play with each state transition? (Currently no — host owns audio cues.)
3. What's the brand/personality direction? (Whimsical / professional / cute / retro?)
4. Any specific cultural references to lean into or avoid?
