## wispr-fox v1.3.0-nightly.10

### 📊 New: your dictation, by the numbers

A proper analytics dashboard. There's now a **Stats** page in the sidebar, plus
a slim **at-a-glance card on top of History** that you can click into.

- **Time saved** — the headline number: how much longer it would've taken to
  *type* everything you dictated (vs. a conservative 40 wpm typing speed).
- **Words & sessions per day**, your **speaking speed**, and a **day streak**.
- A **7 / 30 / 90-day chart** you can flip between words, time, and sessions.
- These totals are **kept forever** — in their own store that survives both the
  7-day recording cleanup *and* app updates. Clearing your history won't reset
  your time-saved counter.

### 🍎 macOS: the Accessibility permission finally sticks

If you're on a Mac, you've probably had to re-grant Accessibility (the
auto-paste permission) after *every* update. Root cause: unsigned builds get a
new identity each time, so macOS forgot the grant.

Builds are now **signed with a stable identity**, so once you grant
Accessibility, it **persists across future updates**. (No paid Apple account
involved; this doesn't change the one-time right-click-to-Open step on first
install.) Setup notes for the maintainer live in `docs/MACOS_SIGNING.md`.

### 🗨️ Floater polish

- More breathing room between the **speech bubble and the avatar's head** — it
  no longer sits on the face, even the instant you press to dictate.
- The bubble is **wider with more headroom**, so longer lines (like "still here
  whenever you're ready…") wrap cleanly instead of getting clipped at the top.
- The **classic Clippy** skin now gets the same friendly dialog bubble as the
  others.

### 🎚️ Sidebar

- The **S / M / L** floater-size buttons now **stack neatly** in the collapsed
  (narrow) sidebar instead of spilling out of it.

---

*Nightly build — pre-release. See the install notes at the top of the release
for first-launch steps on Windows/macOS.*
