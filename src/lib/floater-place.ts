// Default placement for the floater window, extracted so the clippy page's
// first-launch placement, the context menu's "Reset position", and any future
// caller all share ONE implementation of the monitor math (rather than three
// copies that drift apart).
//
// CRITICAL (M4 Pro invisible-floater bug): availableMonitors()/primaryMonitor()
// report PHYSICAL px; setPosition MUST be handed a PhysicalPosition or Tauri
// re-multiplies by the scale factor and shoves the window off-screen. Everything
// here stays in physical px.
//
// Skin-aware default:
//   - "wave": horizontally centred, ~10% of the screen height below the top
//     edge (Wispr-Flow feel — near the top but not jammed against it).
//   - "siri": right-of-centre, vertically centred (the little orb "pops up"
//     on the right, like Siri).
//   - everything else ("character" class): the classic bottom-right slot.

import type { Skin } from "./skin-store.svelte";

/** Positioning class — wave pill vs siri orb vs everything else. */
export function skinClass(skin: Skin): "wave" | "siri" | "character" {
  if (skin === "wave") return "wave";
  if (skin === "siri") return "siri";
  return "character";
}

/** Logical (design) window size used to compute the physical placement.
 *  Must track the ART REST box + pads in clippy/+page.svelte. Exported so the
 *  clippy page's saved-position clamping uses the same numbers. */
export function logicalWinSize(skin: Skin): { w: number; h: number } {
  // wave ART {120,32} + 8px side / 8px top+bottom pads → 136×48.
  if (skin === "wave") return { w: 136, h: 48 };
  // siri ART {58,58} + pads → 74×74.
  if (skin === "siri") return { w: 74, h: 74 };
  // character: the classic 190×210 footprint.
  return { w: 190, h: 210 };
}

/**
 * Move the floater window to its skin-appropriate default position and return
 * the physical coordinates used (so the caller can persist them). Clears
 * nothing — the caller decides whether to also drop the saved position.
 */
export async function placeFloaterDefault(skin: Skin): Promise<{ x: number; y: number } | null> {
  const { getCurrentWindow, availableMonitors, primaryMonitor, PhysicalPosition } = await import(
    "@tauri-apps/api/window"
  );
  const monitors = await availableMonitors();
  let m = monitors[0];
  try {
    const p = await primaryMonitor();
    if (p) m = p;
  } catch {
    /* fall back to monitors[0] */
  }
  if (!m) return null;

  const sf = m.scaleFactor ?? 1;
  const { w: logW, h: logH } = logicalWinSize(skin);
  const winWPhys = Math.round(logW * sf);
  const winHPhys = Math.round(logH * sf);
  const cls = skinClass(skin);

  let x: number;
  let y: number;
  if (cls === "wave") {
    // Top-centre: horizontally centred, ~10% of the screen height below the top.
    x = m.position.x + Math.round((m.size.width - winWPhys) / 2);
    y = m.position.y + Math.round(m.size.height * 0.1);
  } else if (cls === "siri") {
    // Right-of-centre, vertically centred: sits ~7% of the width in from the
    // right edge, at mid-height — the "pops up on the right" spot.
    x = m.position.x + m.size.width - winWPhys - Math.round(m.size.width * 0.07);
    y = m.position.y + Math.round((m.size.height - winHPhys) / 2);
  } else {
    // Bottom-right-ish, matching the classic default-placement code.
    const marginXPhys = Math.round(24 * sf);
    const marginYPhys = Math.round(60 * sf);
    x = m.position.x + m.size.width - winWPhys - marginXPhys;
    y = m.position.y + m.size.height - winHPhys - marginYPhys;
  }

  await getCurrentWindow().setPosition(new PhysicalPosition(x, y));
  return { x, y };
}

/** localStorage key for the saved floater position. Per positioning class so a
 *  wave, a siri, and a character placement don't fight over one slot. */
export function posKeyFor(skin: Skin): string {
  const cls = skinClass(skin);
  return cls === "character" ? "wispr.clippy.pos" : `wispr.clippy.pos.${cls}`;
}
