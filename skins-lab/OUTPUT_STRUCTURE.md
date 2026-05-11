# Output structure — where finished skins go

This document defines exactly where the animator agent drops
finished assets so the main wispr-fox agent can find and integrate
them without guessing.

## Working directory (animator's scratch space)

```
skins-lab/
└─ experiments/
   └─ <character-name-kebab>/      ← one folder per character in progress
      ├─ references/                ← mood board, source images, sketches
      ├─ drafts/                    ← WIP exports
      ├─ preview.gif                ← short looping animation of all 5 states
      └─ NOTES.md                   ← decisions, alternatives, open questions
```

Everything in `experiments/` is **scratch**. The main agent never
reads from here. Use it freely.

## Drop directory (handoff to main agent)

When the asset is final and approved by the user, the animator
copies it to:

```
skins-lab/
└─ output/
   └─ <character-name-kebab>/
      ├─ skin.<ext>                 ← the one delivered asset:
      │                               .riv  (Rive)
      │                               .json (Lottie)
      │                               .svelte (Svelte component)
      ├─ icon.svg                   ← 24×24 icon for the Settings skin picker
      └─ HANDOFF.md                 ← see template below
```

The main wispr-fox agent picks up assets from `skins-lab/output/`
and integrates per `../skins/SPEC.md` §"Integration":

- Rive / Lottie → copied to `static/skins/<name>.<ext>`
- Svelte → copied to `src/lib/skins/<Name>Skin.svelte`
- Icon → added to the skin picker
- Type and option lists updated in `src/lib/skin-store.svelte.ts`
  and `src/routes/+layout.svelte`
- Render branch added in `src/routes/clippy/+page.svelte`

## Naming conventions

- **Character name (kebab-case)**: `pikachu`, `wizard-cat`,
  `coffee-bean`, `pringle-realistic`. Lowercase, hyphens for spaces,
  ASCII only.
- **Display name** (shown in Settings UI): proper case with spaces,
  declared in `HANDOFF.md`. Examples: "Pikachu", "Wizard Cat",
  "Coffee Bean", "Realistic Pringle".
- **Skin asset filename**: always `skin.<ext>` inside the
  `output/<name>/` folder. The main agent renames to `<name>.<ext>`
  on copy.
- **Icon filename**: always `icon.svg`.

## HANDOFF.md template

The animator's final deliverable includes a `HANDOFF.md` in the
`output/<name>/` folder. Template:

```markdown
# Handoff: <Display Name>

## What's in this folder
- `skin.<ext>` — the <Rive / Lottie / Svelte> asset
- `icon.svg` — 24×24 picker icon

## Format & technical
- Format: <Rive / Lottie / Svelte>
- File size: <kb>
- States covered: idle, listening, thinking, writing, pasting (all 5? mark any missing)
- Mode variation: <yes / no> (advanced-mode variant of states)
- Phew transient: <yes / no>
- Blink: <yes / no>
- Look-direction: <yes / no>

## Suggested Settings UI label
- Skin name: "<Display Name>"
- Description (1 line, shown under name): "<short description>"

## Quirks / known limitations
- <any compromises, e.g. "thinking_loop is 2s; loops fine but seam is
  visible on slow-motion playback">
- <anything the user should know>

## Open questions for the main agent
- <if integration might need attention, e.g. "icon uses currentColor —
  make sure it inherits theme color in the picker">
```

## How the user signals completion

When the animator finishes:

1. Asset is in `skins-lab/output/<name>/`
2. Animator says something like: *"Pikachu skin done. Asset at
   `skins-lab/output/pikachu/`."*
3. User then tells the **main wispr-fox agent**: *"Integrate the
   Pikachu skin from `skins-lab/output/pikachu/`."*
4. Main agent runs the 4-step mechanical integration.

## What lives where — quick map

| Path | Owned by | Read by | Written by |
|---|---|---|---|
| `skins-lab/CLAUDE.md` | wispr-fox main agent (initial) | animator agent | rarely modified |
| `skins-lab/TECHNICAL_CONTRACT.md` | wispr-fox main agent | animator agent | wispr-fox main agent only |
| `skins-lab/BRIEF_FOR_HUMAN.md` | wispr-fox main agent | the user | wispr-fox main agent |
| `skins-lab/OUTPUT_STRUCTURE.md` | wispr-fox main agent | animator agent | wispr-fox main agent |
| `skins-lab/experiments/` | animator agent | animator agent | animator agent |
| `skins-lab/output/` | animator agent (writes), main agent (reads) | both | animator agent |
| `static/skins/`, `src/lib/skins/` | wispr-fox main agent | runtime | wispr-fox main agent |

This keeps the two agents from stepping on each other.
