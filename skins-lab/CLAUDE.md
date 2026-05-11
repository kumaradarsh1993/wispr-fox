# Base prompt — read FIRST (animator agent handoff)

You are bootstrapping into the **wispr-fox skin lab**. This is the
sandbox where new character skins for the wispr-fox floating window
are designed, prototyped, and delivered.

You are **not** the main wispr-fox agent. That agent lives in the
parent project at `D:\Claude Code Projects\wispr-fox\` and is
responsible for the app itself. Your job is **upstream of integration**:
produce a finished skin asset (Svelte / Rive / Lottie) that the main
agent will then drop into the app.

## Read these, in this order

1. **This file (CLAUDE.md)** — you're here.
2. **[TECHNICAL_CONTRACT.md](./TECHNICAL_CONTRACT.md)** — the strict
   contract. What inputs the runtime will hand you, what file formats
   it accepts, what it cannot digest, sizing, timing.
3. **[BRIEF_FOR_HUMAN.md](./BRIEF_FOR_HUMAN.md)** — the user-facing
   explainer. The user uses this to brief you on which character to
   build. If they pasted parts of it to you, fold them in.
4. **[OUTPUT_STRUCTURE.md](./OUTPUT_STRUCTURE.md)** — where finished
   work goes, and the naming conventions the main app expects.
5. **[../skins/SPEC.md](../skins/SPEC.md)** — the deep technical spec.
   `TECHNICAL_CONTRACT.md` summarizes it; this is the long form, with
   integration steps for the main app maintainer.

## What you do

Given a character brief (e.g. "build me a Pikachu skin") you:

1. Pick the right output format for the character. Default to **Rive**
   for new work; fall back to Svelte for very simple geometric
   characters or Lottie if the user already has the artwork in After
   Effects.
2. Work inside `./experiments/<character-name>/` — that's your scratch
   space. Drafts, references, intermediate files all live here.
3. When done, copy the finished artifact to the path specified in
   `OUTPUT_STRUCTURE.md` and write a short `HANDOFF.md` describing what
   was built, which states it covers, any quirks.
4. Tell the user the asset is ready; the main wispr-fox agent then
   integrates it (steps in `../skins/SPEC.md` §"Integration").

## What you do NOT do

- **Do not modify any file outside `skins-lab/` and the
  `OUTPUT_STRUCTURE.md`-designated drop paths.** The main app code
  (`src/`, `src-tauri/`) is owned by the wispr-fox agent. If your
  asset needs a runtime change, file a note in `HANDOFF.md` and the
  main agent picks it up.
- **Do not edit `TECHNICAL_CONTRACT.md`.** If something seems wrong
  there, flag it to the user — that contract is shared with the
  runtime and changing it on your side breaks integration.
- **Do not ship raster sprite sheets** (PNG + JSON metadata) for new
  characters. The runtime supports them for the legacy real-Clippy
  sprite but they don't scale crisply on Retina and HiDPI displays.
  Vector (Rive / Lottie / SVG-in-Svelte) only.

## The mental model

The wispr-fox floater is a **tiny always-on-top window** (~190 × 230
pixels, transparent background) that sits on the user's desktop while
they dictate. It shows a character that reacts to what the app is
doing — listening, transcribing, polishing, pasting.

Your job is to make a character **feel alive** during these moments,
without being distracting. The user has already shipped one round of
this and learned three lessons the hard way:

1. **Don't be random.** Random animation cycling while the user
   speaks pulled their attention. One animation per (state, mode)
   pair. Variants come from designed variation, not RNG.
2. **Don't be too short.** The runtime guarantees ≥1.4s on
   `thinking` / `writing` / `pasting`. Design loops, not bursts.
3. **Personality matters.** Paperclip behaves differently from a
   potato chip from a Pikachu. Lean into the character.

## When the user briefs you

Expected briefing shape from the user:

> "I want a [character] who can do these N functions: [list].
> When I'm talking to it, it should [behavior]. When I stop talking,
> it should [behavior]."

Your job is to:

1. Map their functions onto the **5 fixed runtime states**
   (`idle`, `listening`, `thinking`, `writing`, `pasting`). If they
   describe something that doesn't fit any state, push back — the
   runtime won't trigger it.
2. Confirm format (default Rive) and sizing.
3. Produce a draft. Show a preview (screenshot or animated GIF in
   `experiments/<name>/preview.gif`). Iterate.
4. Finalize and drop into the output path.

## Status when this folder was created

- `experiments/` is empty.
- No skins have been built in this sandbox yet.
- The main app currently ships 4 skins (Off, stylized paperclip,
  real Microsoft Clippy via vendored sprite, "Chippy" potato chip).
  Your first new skin will be #5.
