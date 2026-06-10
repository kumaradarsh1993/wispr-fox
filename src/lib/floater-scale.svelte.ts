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

// ── Floater debug overlay ───────────────────────────────────────────────
// Off by default. When on, the floater draws its window bounds (a thin
// frame around the whole webview) plus a live readout of the requested vs
// actual window size, the size-state, scale-factor and user-scale — so you
// can SEE whether the box is resizing and how tightly it hugs the avatar.
// Same localStorage + cross-window event plumbing as the scale store.
const DEBUG_KEY = "wispr.clippy.debug";
const DEBUG_EVENT = "wispr:floater-debug-change";

class FloaterDebugStore {
  current = $state<boolean>(
    typeof localStorage !== "undefined" && localStorage.getItem(DEBUG_KEY) === "1",
  );
  private subscribed = false;

  async subscribe() {
    if (this.subscribed) return;
    this.subscribed = true;
    await listen<boolean | string>(DEBUG_EVENT, (e) => {
      const v = e.payload === true || e.payload === "1" || e.payload === "true";
      this.current = v;
      try {
        localStorage.setItem(DEBUG_KEY, v ? "1" : "0");
      } catch {}
    });
  }

  async set(on: boolean) {
    this.current = on;
    try {
      localStorage.setItem(DEBUG_KEY, on ? "1" : "0");
    } catch {}
    try {
      await emit(DEBUG_EVENT, on);
    } catch {}
  }

  async toggle() {
    await this.set(!this.current);
  }
}

export const floaterDebug = new FloaterDebugStore();

// ── Floater box mode: compact (dynamic) vs full (classic) ──────────────
// Compact (default, v1.4.0): the window hugs the avatar at rest and grows
// upward only while a speech bubble is showing — reclaims the dead band
// above the head that used to cover content and eat clicks. The grow/shrink
// is masked by a brief avatar fade because the webview re-rasterizes
// asynchronously after a native resize (content briefly anchors to the
// window's top-left at the old size — the "corner glitch").
// Full (classic, v1.3.0): ONE fixed box per avatar that always reserves the
// bubble band; the window never resizes on dictation, so there is zero
// transition artifact. Surfaced in Settings → Appearance as a safety valve
// for users who find any transition at all too noticeable.
const FIXEDBOX_KEY = "wispr.clippy.fixedbox";
const FIXEDBOX_EVENT = "wispr:floater-fixedbox-change";

class FloaterFixedBoxStore {
  current = $state<boolean>(
    typeof localStorage !== "undefined" && localStorage.getItem(FIXEDBOX_KEY) === "1",
  );
  private subscribed = false;

  async subscribe() {
    if (this.subscribed) return;
    this.subscribed = true;
    await listen<boolean | string>(FIXEDBOX_EVENT, (e) => {
      const v = e.payload === true || e.payload === "1" || e.payload === "true";
      this.current = v;
      try {
        localStorage.setItem(FIXEDBOX_KEY, v ? "1" : "0");
      } catch {}
    });
  }

  async set(on: boolean) {
    this.current = on;
    try {
      localStorage.setItem(FIXEDBOX_KEY, on ? "1" : "0");
    } catch {}
    try {
      await emit(FIXEDBOX_EVENT, on);
    } catch {}
  }
}

export const floaterFixedBox = new FloaterFixedBoxStore();
