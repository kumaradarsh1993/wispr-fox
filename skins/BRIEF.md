# Floater Skin Brief — wispr-fox

Looking for a character animator (Rive, Lottie, or SVG) to design a floating animated character for a Windows/Mac dictation app called wispr-fox. The character lives in a small always-on-top window on the user's desktop and visually reacts to what the app is doing.

## What the character has to do

The app drives the character through 5 states. Make sure each state is **visibly distinct at a glance** and has its own personality:

1. **Idle** — Nothing happening. Resting pose with occasional ambient movement (blinks, looks around).
2. **Listening** — User is actively speaking into the mic. Character is **clearly engaged with the user** (cups ear, leans in, attentive eyes). Should hold the "engaged" feel indefinitely while looping subtly.
3. **Thinking** — Audio is being transcribed by a speech-to-text API. Character is processing. Thought bubble, gears, scratched head, etc.
4. **Writing** — A second AI is cleaning up / formatting the transcript. Character is "polishing" — pen and paper, magic wand, etc.
5. **Pasting** — Final text is being delivered to wherever the user's cursor is. ~1 second celebration. Then back to idle.

Bonus state: a **"phew" transient** (~700ms relief beat) right after listening ends — a sigh, sweat drop, stretch, anything that feels like "okay, taking a breath now."

There are also two **modes** the app runs in:
- **Light mode** — quick cleanup
- **Advanced mode** — heavier creative transformation

The thinking/writing animations should ideally have variants between modes. E.g., light writing = pen scribble; advanced writing = wand sparkles. Mode is a separate input the animator can use.

## Spec sheet

- **Canvas**: 190 × 230 px, transparent background
- **Character size**: 120-160 px tall, centered, anchored to bottom
- **Top 50 px**: Leave clean — the app overlays a small speech bubble there
- **Format preferred**: Rive `.riv` (state machine: states + transitions + inputs)
- **Alternative formats**: Lottie JSON, or Svelte component with SVG+CSS
- **File size budget**: < 500KB
- **Performance**: 60fps on a midrange laptop
- **State machine inputs** (if Rive):
  - `stateNum` (number): 0=idle, 1=listening, 2=thinking, 3=writing, 4=pasting
  - `modeNum` (number): 0=light, 1=advanced
  - `phew` (trigger)
  - `blink` (trigger)

## What I want — character direction

(Pick the right paragraph based on the brief you got. Some examples we want:)

**Realistic Pringles** — A photo-real but slightly stylized potato chip with proper shading and dimensionality. Like the chip on the Pringles can but with a face. Variants for lounging on a sofa, wearing a tiny chef's hat for writing, etc.

**Classic Clippy improved** — Microsoft Office Assistant vibe but cleaner, more expressive, modern proportions. Bushy black eyebrows. Should feel nostalgic but not retro-pixelated.

**Pet variant** — A cute small animal (cat / dog / capybara / hamster). When listening, ears perk up. When thinking, paw to chin. When writing, on a tiny laptop. Etc.

## Deliverables

- The `.riv` (or `.json` or `.svelte`) file
- A 5-10 second preview video showing all states + the phew transient
- A brief on which animation names / state machine inputs to wire up

## Reference / current state of the app

- The app already ships 3 SVG-based skins (paperclip, Microsoft Clippy via sprite, potato chip). Source in `src/routes/clippy/+page.svelte` and `src/lib/clippyjs-vendor/`.
- The full integration spec is in `SPEC.md` next to this file.

## Pricing benchmarks I'm comparing against

- Fiverr Rive freelancer (4-5 states): $50-150
- Specialized Rive animator: $200-500
- Lottie via Fiverr: $20-100
- Open to other formats if the cost / quality story is better

## Timeline

Flexible — quality > speed. ~1-2 weeks acceptable for the realistic Pringles option since that's the most ambitious.
