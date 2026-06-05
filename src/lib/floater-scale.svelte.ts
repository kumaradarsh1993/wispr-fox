// Floater (Clippy) size scale — shared between the main window and the Clippy
// window via Tauri events, persisted in localStorage. Same pattern as
// skin-store.svelte.ts: both windows keep their own localStorage copy and
// stay in sync through a cross-window emit/listen pair.
//
// Why a scale knob exists: the avatar art + its window are authored at a
// "design" size that feels right on a 15" laptop. On a 13" screen the same
// floater eats a larger share of the display and leaves a big transparent
// dead-zone. This multiplier shrinks (or grows) BOTH the window and the
// avatar uniformly so small screens aren't dominated by the floater.
//
// The clippy window reads `current` to multiply its per-skin window
// dimensions and sets the `--fscale` CSS variable so the avatar art scales
// in lockstep (see src/routes/clippy/+page.svelte).

import { emit, listen } from "@tauri-apps/api/event";

const STORAGE_KEY = "wispr.clippy.scale";
const EVENT = "wispr:scale-change";

// Clamp range for the slider + any restored value. Below 0.6 the avatar gets
// too small to read; above 1.4 it starts to dominate even large screens.
export const SCALE_MIN = 0.6;
export const SCALE_MAX = 1.4;

// Quick presets surfaced as S / M / L buttons in the sidebar. Medium = 1.0 is
// the original design size (unchanged behaviour for existing users).
export const SCALE_PRESETS: { id: "s" | "m" | "l"; label: string; value: number }[] = [
  { id: "s", label: "S", value: 0.8 },
  { id: "m", label: "M", value: 1.0 },
  { id: "l", label: "L", value: 1.25 },
];

function clamp(n: number): number {
  if (Number.isNaN(n)) return 1;
  return Math.min(SCALE_MAX, Math.max(SCALE_MIN, n));
}

function readInitial(): number {
  const raw = (typeof localStorage !== "undefined"
    ? localStorage.getItem(STORAGE_KEY)
    : null) as string | null;
  if (raw == null) return 1;
  const n = Number.parseFloat(raw);
  return clamp(n);
}

class FloaterScaleStore {
  current = $state<number>(readInitial());
  private subscribed = false;

  /** Subscribe to cross-window scale updates. Both windows call this. */
  async subscribe() {
    if (this.subscribed) return;
    this.subscribed = true;
    await listen<number | string>(EVENT, (e) => {
      const v = clamp(
        typeof e.payload === "string" ? Number.parseFloat(e.payload) : (e.payload as number),
      );
      this.current = v;
      try {
        localStorage.setItem(STORAGE_KEY, String(v));
      } catch {}
    });
  }

  /** Set the scale and broadcast — call from the sidebar / settings slider. */
  async set(n: number) {
    const v = clamp(n);
    this.current = v;
    try {
      localStorage.setItem(STORAGE_KEY, String(v));
    } catch {}
    try {
      await emit(EVENT, v);
    } catch {}
  }

  /** Which preset (if any) the current value exactly matches — for highlighting. */
  activePreset(): "s" | "m" | "l" | null {
    const hit = SCALE_PRESETS.find((p) => Math.abs(p.value - this.current) < 0.001);
    return hit ? hit.id : null;
  }
}

export const floaterScale = new FloaterScaleStore();
