// Avatar (floater) visibility tri-state — shared between the main window and
// the Clippy window via Tauri events, persisted in localStorage. Same pattern
// as skin-store.svelte.ts / floater-scale.svelte.ts: both windows keep their
// own localStorage copy and stay in sync through a cross-window emit/listen
// pair.
//
// This DECOUPLES "is the avatar shown" from "which skin". Historically the
// skin value "off" doubled as the hide switch, which meant you couldn't pick
// a character AND keep it hidden, and hiding forgot your skin. Now visibility
// is its own axis:
//
//   "always"  — the floater is always on screen (classic behaviour).
//   "auto"    — the floater shows itself only while a dictation is in flight
//               (state leaves idle → enter animation + show; back to idle →
//               grace, exit animation, hide). Managed BY the clippy webview,
//               which keeps running its JS even while its window is hidden.
//   "hidden"  — the floater is never shown.
//
// Ownership of the actual window show/hide:
//   - "always"/"hidden": the MAIN window drives it (show / hide) — see
//     applyVisibilityWindow() below, called from the sidebar / settings /
//     context menu whenever the value changes.
//   - "auto": the CLIPPY webview owns it (main window leaves the window in
//     whatever state it's in; the clippy page shows/hides on flow state).

import { emit, listen } from "@tauri-apps/api/event";
import type { Skin } from "./skin-store.svelte";

export type AvatarVisibility = "always" | "auto" | "hidden";

const STORAGE_KEY = "wispr.avatar.visibility";
const EVENT = "wispr:avatar-visibility-change";

const VALID: readonly AvatarVisibility[] = ["always", "auto", "hidden"] as const;

// One-time migration marker: if the persisted skin was "off", we move that
// intent onto the visibility axis (hidden) and reset the skin to the default
// fox. Idempotent — guarded by its own marker so a user who legitimately
// re-hides never gets their skin stomped on a later launch.
const MIGRATED_KEY = "wispr.avatar.visibility.migratedFromOffSkin";
const SKIN_STORAGE_KEY = "wispr.clippy.skin";

function readRaw(): string | null {
  if (typeof localStorage === "undefined") return null;
  return localStorage.getItem(STORAGE_KEY);
}

/**
 * Run the one-time "off skin → hidden" migration. Called from readInitial so
 * the very first store read already reflects the migrated value. Safe to call
 * repeatedly (marker-gated + only acts when the skin is literally "off").
 */
function migrateOffSkin(): AvatarVisibility | null {
  if (typeof localStorage === "undefined") return null;
  if (localStorage.getItem(MIGRATED_KEY) === "1") return null;
  const skin = localStorage.getItem(SKIN_STORAGE_KEY);
  if (skin === "off") {
    try {
      localStorage.setItem(STORAGE_KEY, "hidden");
      // Reset the skin to the default so switching back to "shown" reveals a
      // real character rather than the retired "off" placeholder.
      localStorage.setItem(SKIN_STORAGE_KEY, "fox" satisfies Skin);
      localStorage.setItem(MIGRATED_KEY, "1");
    } catch {}
    return "hidden";
  }
  // Nothing to migrate, but stamp the marker so we never re-scan on every boot.
  try {
    localStorage.setItem(MIGRATED_KEY, "1");
  } catch {}
  return null;
}

function readInitial(): AvatarVisibility {
  const migrated = migrateOffSkin();
  if (migrated) return migrated;
  const raw = readRaw();
  if (raw && (VALID as readonly string[]).includes(raw)) return raw as AvatarVisibility;
  return "always";
}

class AvatarVisibilityStore {
  current = $state<AvatarVisibility>(readInitial());
  private subscribed = false;

  /** Subscribe to cross-window visibility updates. Both windows call this. */
  async subscribe() {
    if (this.subscribed) return;
    this.subscribed = true;
    await listen<string>(EVENT, (e) => {
      const v = e.payload as AvatarVisibility;
      if ((VALID as readonly string[]).includes(v)) {
        this.current = v;
        try {
          localStorage.setItem(STORAGE_KEY, v);
        } catch {}
      }
    });
  }

  /** Set the visibility and broadcast — call from the sidebar / settings / menu. */
  async set(v: AvatarVisibility) {
    this.current = v;
    try {
      localStorage.setItem(STORAGE_KEY, v);
    } catch {}
    try {
      await emit(EVENT, v);
    } catch {}
  }
}

export const avatarVisibility = new AvatarVisibilityStore();

/**
 * Apply the window show/hide that the MAIN window owns for the two static
 * modes. Call this from any main-window surface right after `set()`:
 *   - "always" → show the clippy window.
 *   - "hidden" → hide it.
 *   - "auto"   → do NOT touch the window here; the clippy webview manages it
 *                from flow state. (We still leave it as-is so an in-flight
 *                dictation isn't yanked away.)
 */
export async function applyVisibilityWindow(v: AvatarVisibility) {
  if (v === "auto") return;
  try {
    const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
    const w = await WebviewWindow.getByLabel("clippy");
    if (!w) return;
    if (v === "always") {
      await w.show();
    } else {
      await w.hide();
    }
  } catch (e) {
    console.warn("applyVisibilityWindow failed", e);
  }
}
