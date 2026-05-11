# Brief for the human (you) — how to drive the skin pipeline

This document is for **you, the project owner**, not for either
agent. It explains the technical model in plain language so you can
brief the animator agent effectively and know what's possible vs.
what isn't.

Read this once before your first session with the animator agent.
After that, it becomes reference material.

---

## How the system actually works

The wispr-fox app has a small **always-on-top window** (about the
size of a sticky note, 190 × 230 pixels) that floats on your
desktop. Inside it lives a "character" — currently Clippy by
default, but it can be any skin you load.

The character is **not a real-time reactive thing**. It does not
listen to your microphone, it does not see your screen, it does not
respond to mouse movement. It only knows about **five states the app
is in at any moment**:

| State | What the app is doing | What you experience |
|---|---|---|
| `idle` | Waiting for you to press a hotkey | Character is at rest |
| `listening` | You're holding F8/F9/F10 | Character is paying attention to you |
| `thinking` | Audio is being transcribed by Whisper | Character is processing |
| `writing` | LLM is cleaning your transcript | Character is polishing |
| `pasting` | Text is being inserted into your focused app | Character delivers, brief celebration |

That's it. Five states. The animator agent's job is to make a
character that reacts to those five states in a way that feels alive.

There's one extra signal: `mode` — `light` (F8) vs `advanced` (F9).
This lets a single character have two flavors of the same animation
(e.g. ordinary thinking pose for `light`, dramatic "wizard" thinking
pose for `advanced`). Optional.

## What you can and cannot ask the animator for

### You CAN ask for

- "When I'm talking to it (`listening`), I want it to cup its ear and
  lean toward me."
- "When it's thinking, I want a thought bubble with the character's
  face visible inside."
- "When it's pasting, do a little victory dance."
- "Idle state should have it bobbing gently with occasional blinks."
- "In advanced mode, give it a wizard hat during thinking."
- "Two variants per state — pick one randomly per session" (the
  runtime can cycle these on app launch, not mid-state).

### You CANNOT ask for

- "Make it react to how loud I'm speaking" — the runtime doesn't pass
  audio amplitude.
- "Make it look at my mouse cursor" — no mouse signal.
- "Make it read the transcript and react to the content" — no text
  signal.
- "Make it talk back to me" — no audio output (the host owns audio
  cues, and they're tied to fixed event sounds, not the character).
- "Make it dance to my Spotify" — see above; no audio signal.
- "Different animation depending on what app I'm dictating into" —
  the skin has no awareness of focused app.

If you find yourself wanting one of these, that's a feature request
for the **main wispr-fox agent**, not the animator. The animator
works within the existing signals.

## The contract guarantees you have

When you brief the animator, these are the **fixed-in-stone**
parameters they should design around:

1. **Window size: 190 × 230 px.** Their character lives in that box.
2. **Transparent background.** Their character floats on your desktop
   wallpaper.
3. **`thinking`, `writing`, `pasting` have a 1.4-second minimum
   duration.** No matter how fast the actual work finishes, the
   animation gets at least 1.4s to play.
4. **`listening` is open-ended.** It runs from 1 second to ~30
   seconds depending on how long you hold the hotkey. Their
   animation must loop seamlessly.
5. **`idle` is open-ended.** Same — loop seamlessly.
6. **`pasting` is roughly 1 second.** It's a brief celebration moment.
7. **No mid-state interruption.** Once `thinking` starts, it plays
   for at least 1.4s. Animator doesn't need to design "what if
   `thinking` is cut short" — it won't be.

## How animations actually get triggered

The runtime is a **state machine**, not a script:

1. App enters a state (say, `listening`).
2. Runtime sets the input value (`stateNum = 1` for Rive, or "play
   segment `listening_loop`" for Lottie).
3. The animation system smoothly transitions there.
4. App eventually leaves the state (you release the hotkey).
5. Runtime sets the input to the next state.
6. The animation transitions out.

Crucially: **the animator doesn't write code that runs at specific
times**. They build a state machine (in Rive) or named clips (in
Lottie) and the runtime drives it. This is good — it's robust and
testable. It also means: the animator cannot do "do X at exactly
2.5 seconds into listening" because listening duration is variable.

## What output formats are available

Three formats, in order of preference for new work:

| Format | Best for | Authoring tool |
|---|---|---|
| **Rive** | New characters with rich state machines | Rive editor (free, free tier sufficient) |
| **Lottie** | Characters already animated in After Effects | After Effects + Bodymovin plugin |
| **Svelte + SVG** | Very simple geometric characters | Code editor only |

If you have no preference, **default to Rive**. It's the most
expressive, the runtime is well-tested, and files are small.

## How to brief the animator agent

A good briefing has these parts:

### 1. The character

"I want a [Pikachu / cat / hamster / robot / abstract blob] as my
floater character."

Provide visual references if possible (links to images, descriptions
of the style — "looks like 90s Pokémon anime", "watercolor", "low-poly
3D feel").

### 2. State-by-state behavior

Walk through the 5 states:

- "In `idle`, I want it [behavior]."
- "In `listening`, I want it [behavior]."
- "In `thinking`, [behavior]."
- "In `writing`, [behavior]."
- "In `pasting`, [behavior]."

The animator can suggest defaults if you're undecided — just say
"surprise me on `idle` but make `listening` very engaged."

### 3. Mode variation (optional)

"In advanced mode (F9), give it [variation]."

Skip this if you don't care; not every character benefits.

### 4. Personality

A one-line vibe: "playful but not childish", "serious craftsman",
"smug genius", "anxious helper". This colors everything else.

### 5. Format preference

"Use Rive." Or "use Lottie because I have After Effects files
already." Or "use whatever's appropriate."

## What you get back

The animator drops the finished asset into a known path (see
`OUTPUT_STRUCTURE.md`). They also write a short `HANDOFF.md` with:

- What was built
- Which states are covered (all 5, hopefully)
- Any quirks or compromises
- Suggested skin name for the Settings menu

Then you tell the **main wispr-fox agent** "the new skin is in
`<path>`, integrate it" — and that agent runs through the four
mechanical integration steps from `../skins/SPEC.md`.

## A worked example

You: *"Build me a Pikachu skin. It should look like the original
yellow Pikachu from Pokémon Yellow. In idle, ear-twitch every few
seconds. In listening, the cheeks should glow faintly and ears
should be alert. In thinking, hand on chin, classic thinking pose,
maybe a thought bubble with a Poké Ball. In writing, scribbling
furiously on a notepad. In pasting, do the lightning-tail flick
celebration. In advanced mode, add a tiny wizard hat. Rive format."*

Animator agent: *Builds in Rive, drops `pikachu.riv` into
`skins-lab/experiments/pikachu/` first for preview, you approve, they
move it to the final path, write HANDOFF.md.*

You: *Tell the main agent "integrate skins-lab/output/pikachu.riv"*

Main agent: Adds Pikachu to the Settings skin picker, deploys.

You ship.

---

## When to push back on the animator

- If they propose something that requires runtime changes — they
  should flag it, not silently bake it in.
- If they ship an asset > 2 MB.
- If they ignore the 190 × 230 sizing.
- If they want to use a format not in the three supported.
- If they say "this state needs to be 0.5 seconds" — the floor is 1.4s.

When in doubt: the **`TECHNICAL_CONTRACT.md` is the source of
truth**. If the animator deviates, ask them to either come back to
contract or explain why the contract should change.
