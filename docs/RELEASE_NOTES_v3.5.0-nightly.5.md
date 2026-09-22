# wispr-fox v3.5.0-nightly.5

**The fox stops drifting up and to the left, and a recording interrupted
mid-sentence is no longer lost.**

## The floater crept across the screen, once per session

This was a real bug, not a mis-drag. The fox you see sits at the **bottom
centre** of a transparent window that is larger than the fox — and that window
**grows** in two situations: when a speech bubble appears (roughly 132x132 to
238x187), and when you right-click the fox to open its menu (192x316). It grows
upward and outward, so the fox itself stays visually still. That part was
correct.

The bug was in what got **saved**. The app stored the window's **top-left
corner**, on any mouse-release inside the floater — which in practice happened
while the box was grown. On the next launch the box was back at its small
resting size, but the app placed that small box's top-left at the saved
coordinate. Because the grown box's top-left had been higher and further left,
the fox reappeared up and to the left of where you left it. That new position
was then saved in turn, so it compounded silently:

| What you did | How far the fox moved next launch |
|---|---|
| Right-clicked the fox | ~184 px up, ~30 px left |
| Left a bubble up when you clicked | ~55 px up, ~53 px left |

Which is exactly the reported behaviour — never while speaking, always across
sessions, always up and to the left, eventually off the top-left of the screen.

**The fix saves the fox, not the box.** The app now persists the **midpoint of
the window's bottom edge** — the point the fox actually stands on. That point
does not move when the box grows, so it no longer matters whether a bubble or
the right-click menu was open when the position was saved. This removes the
whole class of bug rather than patching the two known cases.

Three things come with it:

- **A position can no longer be off-screen.** On restore the window is clamped
  onto a monitor that really exists, and on macOS kept clear of the menu bar. A
  position inherited from an unplugged external display now lands in the
  equivalent spot on the remaining screen instead of vanishing.
- **Self-rescue on display changes.** The floater already pings the backend
  every 10 seconds; every third ping it now checks whether less than half the
  window is on any monitor and pulls it back. That covers unplugging an
  external monitor mid-session. It deliberately tolerates a fox you have parked
  half off the edge on purpose.
- **A one-time migration.** Your existing saved position is a drifted top-left.
  On the first launch after this update it is converted into a proper anchor and
  clamped, so the fox comes back on screen and then holds. If it lands somewhere
  you dislike, right-click → **Reset position**.

Also corrected while in there: the default-placement code still assumed a
190x210 fox box when the real resting box is about 132x132, so a freshly placed
fox sat ~58 px further from the bottom-right corner than intended. **Reset
position** is now anchor-based too, which fixes it landing wrong when the
right-click menu had the window grown at the moment you clicked it.

## A crash-truncated recording repairs itself

A WAV states its own length in two places — the `RIFF` chunk size near byte 4,
and the `data` chunk size just before the audio — and both can only be written
when recording **stops**. If the app is killed mid-recording (a crash, a forced
quit, or an update that replaces the running app), those fields stay zero while
the real audio sits right behind them. Every decoder trusts the header, so the
file comes back as **"contains no audio"** and the recording looks destroyed
when in fact only 8 bytes are wrong. That is how a 13.5-minute session was
nearly lost on 16 September.

wispr-fox now rewrites those 8 bytes from the file's true size on disk the first
time it touches such a file. Nothing else in the file changes and no samples are
re-encoded. The repair sits inside `read_mono_f32`, which every path funnels
through — playback, chunked upload, the 16 kHz shrink before transcription, and
a Rerun from History — so an affected recording heals wherever you first open
it, with no separate recovery step.

It fails open by design: a header that looks fine, a read-only file, or an
exotic layout is left exactly as it was.
