# wispr-fox v3.5.0-nightly.3

**The sidebar is back the way you use it. Two nightly.2 regressions fixed.**

## The left pane again has your controls

nightly.2 stripped the sidebar down to three nav items and moved the engine,
polish and avatar controls into a strip under the Home header. That was the
wrong call for someone who keeps the pane open all day — it made the most
prominent surface in the app mostly empty. Reverted:

- **Quick controls** are back in the sidebar: listening engine, the Polish
  switch, the microphone picker, the avatar with its Always / While dictating /
  Hidden control.
- **Dictation keys** card is back.
- **The fox** is back at the bottom of the pane.
- The header strip is gone (nothing is duplicated).

Kept from nightly.2: the nav reads **Home · Insights · Settings**, the
**account chip** at the bottom (your initial + email, or "Sign in"), the
one-row Home header with ⌘K search, the honest filters, Clear all in
Settings › App & data › Danger zone, and Today's quota on Insights.

## Fixes

- **The avatar vanished for some users after updating.** The visibility menu
  in the (now removed) header strip rendered *behind* the day divider and the
  cards, so a click on "Always" could land on a card instead, leaving the fox
  in "While dictating" mode — which shows the fox only during a recording.
  Two changes: the strip is gone, and **"Always" is now re-asserted on every
  launch** from the saved setting, so the fox cannot stay hidden by accident.
  If yours is still missing: right-click anywhere in the app → **Show avatar**
  (new), or menu-bar fox → **Show / hide avatar**.
- **Right-click menu** in the app now always offers **Show avatar / Hide
  avatar**. The menu-bar item was renamed from "Toggle Clippy" to the same
  wording.

## Audio retention default: 3 days

Recordings were kept for 7 days by default; a heavy week can leave half a
gigabyte of WAV on disk. New installs default to **3 days**. Existing installs
keep whatever they have — change yours in Settings › App & data › Retention.
Lifetime Insights are never affected by retention.

## Coming next

The **card redesign** (nightly.4) and a **pause / resume** mode for running
commentary while reviewing documents, with crash-safe recovery of partial
recordings (design being confirmed).
