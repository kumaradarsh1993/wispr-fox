# wispr-fox v3.2.0-nightly.1

This Codex-authored nightly is a full visual and information-architecture pass for the desktop app. It keeps the watercolor fox and field atmosphere, while making everyday controls quieter, clearer, and more consistent.

## A calmer app shell

- The oversized settings-heavy sidebar is now a focused 272 px navigation rail.
- Everyday controls are grouped into one compact card: listening engine, transcript polishing, microphone, companion, and visibility.
- Provider/model detail, the full companion gallery, and advanced choices now live in Settings, where they are easier to understand and maintain.
- Sidebar width, active states, focus rings, shadows, corner radii, and motion now follow one shared visual contract.

## Settings organised around user goals

- Settings no longer opens a second navigation rail inside the first.
- The new horizontal structure is Voice, AI engines, Writing, Companion, App & data, Account, and Advanced.
- Settings opens on Voice & shortcuts, and redundant links and repeated controls have been removed.
- The layout adapts to the available content width rather than assuming a full-screen browser.

## A rebuilt first-run journey

- Onboarding now tells one fox-in-the-field story across Welcome, Voice, Try it, and Sync.
- The watercolor fox replaces the mixed pixel-pet carousel in the hero while the full companion roster remains available later.
- Progress steps are named and accessible instead of being anonymous dots.
- Skip is now remembered independently of API-key state, so choosing to explore the app cannot trap someone in an onboarding loop.
- Existing Gemini and OpenAI writing-engine keys are detected correctly, and the compact account step no longer exposes destructive account controls.

## Cleaner history and insights

- History starts with a clear page title and record count instead of repeating the analytics widget.
- Search now includes titles, raw transcripts, cleaned text, and drafted text.
- Recording rows have explicit expand controls, clearer actions, improved keyboard semantics, and compact-width reflow.
- Insights uses the same field palette, typography, surface depth, and empty-state illustration as the rest of the app.

## Fit and finish

- Warm paper surfaces, field green, fox orange, radii, shadows, focus treatment, and motion timing are shared across the redesigned surfaces.
- Reduced-motion preferences are respected globally.
- Production packaging no longer advertises an MSI artifact that the Windows build does not create.

This is a nightly so the new shell and onboarding can receive real-world feedback before the next stable promotion.
