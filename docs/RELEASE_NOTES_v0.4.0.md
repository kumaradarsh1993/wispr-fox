# v0.4.0 — The Foxy theme

wispr-fox finally looks like wispr-**fox**. Warm cream surfaces,
orange accent, brown text, and a fox where the empty space used to
be. Pass 1 of a two-pass redesign — structure + palette ship now;
the user-provided pastoral illustrations land in v0.4.1.

---

## 🎨 What changed

### New default palette: Foxy

The default light theme is gone. Replaced with **cream surfaces +
orange accent + brown text** — built around the wispr-fox name and
cottage-core fox mascot.

- Surfaces: warm cream (`#faf6ec` main, `#f4ead6` sidebar, `#ffffff`
  cards)
- Accent: vibrant fox orange (`#ec7c34`) with hover/pressed/soft
  variants
- Text: warm dark brown (`#2b2218` primary, `#7d6a55` secondary)
- Borders: low-contrast tan so the cream stays soft
- Semantic colours toned down to fit (soft green, soft red, no neon)

Existing themes preserved:
- **Dark** — now "fox curled up by a fireplace" — deep warm browns
  with the same orange accent (was Apple-style cold grey before)
- **Retro** — unchanged, still vintage cream

### Inline placeholder fox in the sidebar

A tiny illustrated fox now sits at the bottom of the left sidebar
where there used to be empty space. This is an **inline SVG
placeholder** — you'll get the proper pastoral illustration from
the design playbook in v0.4.1 once those assets land.

### History row redesigned

- **Filter pills** (All / Light / Advanced / Drafting / Errors) are
  now individual rounded buttons with the orange accent when active,
  not a segmented control container
- **Raw / Cleaned / Drafted** inline tabs got new styling — active
  tab is filled with the accent orange + white text; dim (= not yet
  generated) tabs are dashed outlines with italic copy
- **3-dot kebab menu** replaces the inline Retry button — opens a
  small popover with Retry + Delete. Frees up visual real estate.
  Closes on outside click or Escape.

### Empty / Loading states

- **Empty state** ("No transcripts yet"): sitting fox in flowers
  (placeholder SVG) + the hint *"Hold F8 anywhere on your computer
  to dictate"*
- **Loading state** ("Loading transcripts…"): curled-up sleeping
  fox (placeholder SVG) with a gentle breathe animation

Both placeholders will be swapped for the real illustrations next
build.

### Inter is now the preferred body font

Matching the design playbook's typography section. Falls back to
system fonts cleanly if Inter isn't installed.

---

## 📦 Asset wishlist (for v0.4.1)

If you want to generate them (one at a time or as 4-up batches):

**Tier 1 — visible everywhere:**
1. Sidebar mascot — small fox sitting in grass/flowers, ~140×180px
2. History bottom illustration — pastoral horizon, ~960×140px
3. Empty state fox — fox in flowers (different pose from sidebar), ~160×140px
4. Loading state fox — curled-up sleeping fox, ~140×100px
5. App + window + tray icon — fox head, 16/32/48/64/128/256

**Tier 2 — polish:**
6. The four illustration variants from the playbook (idle, in-tree, sleeping, with butterfly)
7. Skin variant icons for the FLOATER picker (4-5 small fox heads)

**Tier 3 — nice to have:**
8. Toast micro-illustrations (success / processing / error fox poses)
9. Corner mascot for window controls

All PNGs preferred with transparent backgrounds in the same illustration
style. Send them over and I'll drop them in.

---

## ⬇ Get it

Windows: `wispr-fox_0.4.0_x64-setup.exe` below. Installs over your
existing version — all your settings (now persisting thanks to v0.3.2),
history, and API keys carry over.
