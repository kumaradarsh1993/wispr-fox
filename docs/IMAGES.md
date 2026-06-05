# Adding images to the README

A short guide so the README stays visually consistent. The README references
two hero images today, both of which are placeholders waiting on real
screenshots:

| File | Where it's used | Recommended dimensions | What it should show |
|---|---|---|---|
| `docs/images/hero.png` | Top of README, big | **1200 × 700** (or 1440 × 840 for Retina) | wispr-fox in action — the main window OR the Clippy floater + a target app, captured mid-dictation. Sets the tone in one frame. |
| `docs/images/onboarding.png` | "Setup in 3 steps" section | **900 × 540** | One of the three onboarding screens (Welcome / Setup / Demo). Welcome is probably the best — it has the three mode cards which are instantly readable. |

You can add more images for the avatar gallery, the history page, etc. —
just drop them into `docs/images/` and reference them inline:

```markdown
<p align="center">
  <img src="docs/images/your-screenshot.png" alt="Description" width="640" />
</p>
```

## Capturing the screenshots

### Windows — built-in tool
- **Snipping Tool** (`Win+Shift+S`) → "Window snip" → click the window you
  want. Saves to clipboard; paste into Paint or save directly.
- For the whole app screenshot, capture the main window at **1200 × 800**
  (default size — drag a corner if it opened smaller).

### macOS — built-in tool
- **`Cmd+Shift+5`** → "Capture Selected Window" → click the window.
  Saves to Desktop by default with a soft shadow that looks great.
- For a borderless capture (no shadow), hold **Option** while clicking the
  window — useful if you want to layer the screenshot over a coloured
  background later.

### Hiding sensitive UI bits before capturing
- API keys → blur or set Settings → Providers to "***" by mousing out
- Personal recordings in History → switch to a fresh install if you have
  one, or use the "Clear all history" button in Settings → Data

## Optimizing the files

Bigger is not better — keep the README fast to load. Two-step pipeline:

### 1. Resize
- macOS: open in **Preview** → Tools → Adjust Size → set the longer
  dimension to 1440 (Retina) or 1200 (regular). Keep proportional.
- Windows: open in **Photos** → "..." → Resize → set max dimension.
- Cross-platform CLI: `magick convert input.png -resize 1440x output.png`

### 2. Crush
- **TinyPNG.com** (drag-drop): handles PNGs and JPEGs, ~70% size cut at
  imperceptible quality loss. Free for up to 20 images / session.
- **Squoosh.app** (Google's tool): more controls if you want JPG / WebP.
- CLI alternative: `pngquant --quality=65-80 output.png --output final.png`

Target file sizes:
- Hero image: **≤ 250 KB**
- Section images: **≤ 150 KB**
- Inline icons: **≤ 30 KB**

If you're over those numbers, consider JPEG instead of PNG for photos /
heavy-colour screenshots — JPEG handles gradients much more efficiently
than PNG.

## Where to put them

```
docs/
  images/
    hero.png            ← the main README hero
    onboarding.png      ← README setup-in-3-steps section
    floater-skins.png   ← (optional) avatar gallery 4-up
    history.png         ← (optional) history page
    settings.png        ← (optional) settings page
    macos-permissions.png  ← (optional) the two macOS prompts side by side
```

Once placed, reference them in markdown with relative paths:

```markdown
<img src="docs/images/hero.png" alt="…" width="720" />
```

GitHub renders these correctly on the repo page; relative paths work both
on github.com and when the repo is mirrored / cloned.

## Style notes — keeping it visually consistent

- **Hero image**: lean toward a real-context screenshot (Clippy floating
  over a Slack window mid-dictation) rather than just the app's own
  window. Makes the value prop instantly obvious.
- **App screenshots**: capture in **light theme** for primary shots — the
  cream + fox-orange palette is the app's identity. Use dark / retro
  shots for a "themes" section if you add one.
- **Borders & shadows**: macOS's built-in shadow looks polished. On
  Windows, the Snipping Tool doesn't add one — you can render a soft
  drop-shadow in HTML/CSS later if needed (or leave it borderless for a
  modern flat look).
- **Aspect ratio**: keep hero around 16:9 or 5:3 — wider feels more
  modern, taller feels cramped.
- **People in screenshots**: keep them out unless they're stock or you
  have permission.

## When you add a new image

1. Drop the file in `docs/images/`
2. Reference it in README.md (or whichever doc)
3. Verify it renders by previewing the markdown on GitHub (or with
   `code --preview README.md` in VS Code)
4. Commit both the image and the markdown change in the same commit so a
   clone never sees a broken link

That's it.
