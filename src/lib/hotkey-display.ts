// Cross-platform hotkey display helpers.
//
// Settings store the canonical combo strings (e.g. "Ctrl+Alt+D", "F8",
// "Shift+Super+F8"). The UI shows them differently on each platform:
//
//   Windows:   "Ctrl+Alt+D"     "F8"     "Win+F8"      "Shift+F8"
//   macOS:     "⌃⌥D"            "F8"     "⌘F8"         "⇧F8"
//
// All over-the-app touchpoints (sidebar reminder, onboarding instructions,
// settings labels, history empty state, floater bubble tooltips) should
// route through `prettyHotkey()` so the messaging matches whatever combo
// the user actually has bound on their platform — never hardcoded "F8".

let _isMac: boolean | null = null;
export function isMac(): boolean {
  if (_isMac !== null) return _isMac;
  if (typeof navigator === "undefined") {
    _isMac = false;
    return false;
  }
  // userAgent is the reliable shared signal across Electron/Tauri webviews.
  // userAgentData.platform is nicer but not in WKWebView yet.
  _isMac = /Mac|iPhone|iPad/i.test(navigator.userAgent);
  return _isMac;
}

/**
 * Convert a canonical combo string into the platform-appropriate display
 * form. Idempotent; safe to call on already-pretty input.
 *
 *   prettyHotkey("Ctrl+Alt+D")     // Win → "Ctrl+Alt+D"
 *                                  // Mac → "⌃⌥D"
 *   prettyHotkey("Super+F8")       // Win → "Win+F8"
 *                                  // Mac → "⌘F8"
 *   prettyHotkey("Shift+Super+F8") // Win → "Shift+Win+F8"
 *                                  // Mac → "⇧⌘F8"
 *   prettyHotkey("F8")             // both → "F8"
 *   prettyHotkey("")               // → "(unbound)"
 */
export function prettyHotkey(combo: string | null | undefined): string {
  if (!combo || !combo.trim()) return "(unbound)";
  // Normalise: split on "+", uppercase the key tokens we know.
  const parts = combo.split("+").map((p) => p.trim()).filter(Boolean);
  if (isMac()) {
    // Mac convention: modifier symbols are joined to the key with no
    // delimiter — "⌃⌥D" not "⌃+⌥+D".
    const mods: string[] = [];
    let key = "";
    for (const p of parts) {
      const norm = p.toLowerCase();
      if (norm === "ctrl" || norm === "control") mods.push("⌃");
      else if (norm === "alt" || norm === "option") mods.push("⌥");
      else if (norm === "shift") mods.push("⇧");
      else if (norm === "super" || norm === "cmd" || norm === "command" || norm === "meta") mods.push("⌘");
      else key = p; // last non-modifier wins
    }
    // Order: ⌃ ⌥ ⇧ ⌘ (Apple HIG convention)
    const order = ["⌃", "⌥", "⇧", "⌘"];
    mods.sort((a, b) => order.indexOf(a) - order.indexOf(b));
    return mods.join("") + key;
  }
  // Windows / Linux: keep "+"-separated, replace "Super" with "Win".
  return parts
    .map((p) => {
      const norm = p.toLowerCase();
      if (norm === "super" || norm === "meta" || norm === "cmd" || norm === "command") return "Win";
      if (norm === "control") return "Ctrl";
      if (norm === "option") return "Alt";
      return p;
    })
    .join("+");
}

/**
 * Just the "hold this to dictate" string for the active mode — what the
 * onboarding tip line and the sidebar reminder both read out. Use over
 * raw `prettyHotkey()` when the text reads better as a verb phrase, e.g.
 *   "Hold {holdToDictate(s.light_hotkey)} anywhere to dictate"
 */
export function holdToDictate(combo: string | null | undefined): string {
  return prettyHotkey(combo);
}

/**
 * Tap/click affordance label. For sticky variants users tap once start,
 * tap again stop. We expose the same prettyHotkey() output — kept as a
 * named function so future divergence (e.g. adding "(toggle)" annotation)
 * lands in one place.
 */
export function tapToToggle(combo: string | null | undefined): string {
  return prettyHotkey(combo);
}
