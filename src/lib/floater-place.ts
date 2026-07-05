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
//   - "wave": TOP-CENTER of the primary monitor's work area (Wispr-Flow feel).
//   - everything else ("character" class): the classic bottom-right slot.
//
// The wave window is short/wide; character windows are ~190×210 logical. We use
// per-skin logical dimensions so the physical conversion + centering is right.

import type { Skin } from "./skin-store.svelte";

/** Positioning class — wave pill vs everything else. */
export function skinClass(skin: Skin): "wave" | "character" {
  return skin === "wave" ? "wave" : "character";
}

/** Logical (design) window size used to compute the physical placement. */
function logicalWinSize(skin: Skin): { w: number; h: number } {
  // Wave: matches the ART entry {w:230,h:52} + side/top/bottom pad (~a bit
  // taller than the art). Character: the classic 190×210 footprint.
  if (skin === "wave") return { w: 246, h: 72 };
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

  let x: number;
  let y: number;
  if (skinClass(skin) === "wave") {
    // Top-center: horizontally centered, ~12px below the top edge.
    x = m.position.x + Math.round((m.size.width - winWPhys) / 2);
    y = m.position.y + Math.round(12 * sf);
  } else {
    // Bottom-right-ish, matching the existing default-placement code.
    const marginXPhys = Math.round(24 * sf);
    const marginYPhys = Math.round(60 * sf);
    x = m.position.x + m.size.width - winWPhys - marginXPhys;
    y = m.position.y + m.size.height - winHPhys - marginYPhys;
  }

  await getCurrentWindow().setPosition(new PhysicalPosition(x, y));
  return { x, y };
}

/** localStorage key for the saved floater position. Per positioning class so a
 *  wave placement and a character placement don't fight over one slot. */
export function posKeyFor(skin: Skin): string {
  return skinClass(skin) === "wave" ? "wispr.clippy.pos.wave" : "wispr.clippy.pos";
}
