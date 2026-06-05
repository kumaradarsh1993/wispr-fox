## wispr-fox v1.3.0-nightly.11

A quick follow-up to nightly.10.

### 🖱️ Double-click the avatar opens the main window again

This regressed when we removed the old drag handling. The fix: the floater now
only starts *moving* once you actually drag it a few pixels — so a plain
double-click reaches the "open the main window" action instead of being eaten
by the move gesture. Single-drag to reposition still works exactly as before.

### 🗨️ Roomier speech bubble

Widened the dialog box further so messages read as a wider box and wrap in
fewer lines. (Still no top-clipping on the long 3–4 line messages.)

---

*Nightly build — pre-release. See the install notes at the top of the release
for first-launch steps on Windows/macOS.*
