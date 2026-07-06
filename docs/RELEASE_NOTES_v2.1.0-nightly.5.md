# wispr-fox v2.1.0-nightly.5

Terminal pets, a calmer sidebar, and history cards that name themselves.

## New: eight terminal pets 🐣

The animated pixel companions from the Codex CLI now live in wispr-fox as
avatars: **Codex Pet, Dewey, Fireball, Rocky, Seedy, Stacky, BSOD, and
Null Signal**. They're fully state-driven, not static:

- **Idle** — blinks and breathes.
- **Listening** — waves at you while you talk.
- **Transcribing** — scratches its head, thinking.
- **Polishing** — types away at a tiny laptop.
- **Pasted** — a little celebration dance.
- **Error** — visibly sad for a moment.

They get speech bubbles, hover quips, the floor shadow, drag, scaling, and
the character enter/exit animations — everything the other avatars have.
*(Pet artwork © OpenAI, included for personal/friends-and-family use with
attribution — see `static/pets/README.md`.)*

## Sidebar: curated, wider, no more scrolling

- The avatar picker in the sidebar now shows a **hand-picked six** plus a
  "…" tile that opens the full roster in Settings → Appearance — no more
  wall of tiles. Whatever avatar you're using always appears there.
- The sidebar is **wider by default** (320px) and can stretch further
  (up to 460px).

## History: named recordings + cleaner cards

- **Recordings name themselves.** A few seconds after each dictation, a
  light Groq model writes a 3–7 word title — "what did I talk about here" —
  that appears bolded in the card header next to the time and duration.
  It's a separate parallel call that never delays your paste, and you can
  turn it off in Settings → General.
- **Raw / Cleaned / Drafted moved to the right edge**, aligned on every
  card next to Play and Copy — headers read as one clean line: time ·
  duration · title.
- **You can finally tell cards expand**: hovering a collapsed card surfaces
  a softly glimmering chevron in its bottom-right corner.

*Nightly build — try a pet, dictate something long, and watch it get named.*
