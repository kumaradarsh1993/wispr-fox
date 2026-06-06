## wispr-fox v1.3.0

The "stats + a calmer floater" release.

### 📊 New: your dictation, by the numbers
A **Stats** page (and an at-a-glance card on top of History) with **time saved**
(vs typing at 40 wpm), words & sessions per day, speaking speed, a day streak,
and a 7/30/90-day chart. Totals are kept forever — they survive both the 7-day
recording cleanup and app updates, so clearing history won't reset them.

### 🍎 macOS: the Accessibility permission finally sticks
Builds are now signed with a stable identity, so once you grant Accessibility
(the auto-paste permission), it **persists across updates** instead of breaking
on each one. (No paid Apple account; first-launch right-click-to-Open is
unchanged.)

### 🗨️ Floater polish
- Roomier speech bubble with proper spacing above the avatar's head — no more
  sitting on the face, and long lines no longer clip at the top.
- The classic Clippy skin now shows the same friendly dialog bubble.
- Double-click the avatar to open the main window works reliably.

### 🎚️ Sidebar
- The S / M / L floater-size buttons stack neatly in the collapsed sidebar.

---

**Windows:** run the `.exe`; click through SmartScreen ("More info" → "Run
anyway") on first launch. **macOS (Apple Silicon):** the `.dmg` isn't notarized
— right-click the app → Open the first time (or `xattr -dr
com.apple.quarantine /Applications/wispr-fox.app`), then grant Microphone +
Accessibility when prompted.
