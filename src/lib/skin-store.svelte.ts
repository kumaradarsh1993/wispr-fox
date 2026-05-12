// Floater (Clippy) skin selection — shared between the main window and the
// Clippy window via Tauri events. localStorage persists across restarts.
//
// Skin values:
//   "off"         — floater window hidden
//   "stylized"    — hand-built SVG paperclip (dark outline, transparent body)
//   "beige"       — light/cream-filled paperclip variant with the same
//                    animations + bigger eyes; reads better on dark
//                    backgrounds where the dark-outline stylized fades.
//   "real-clippy" — Microsoft Clippy via clippyts

import { emit, listen } from "@tauri-apps/api/event";

export type Skin = "off" | "stylized" | "beige" | "real-clippy";

const STORAGE_KEY = "wispr.clippy.skin";
const EVENT = "wispr:skin-change";

const VALID_SKINS: readonly Skin[] = ["off", "stylized", "beige", "real-clippy"] as const;

function readInitial(): Skin {
  const raw = (typeof localStorage !== "undefined"
    ? localStorage.getItem(STORAGE_KEY)
    : null) as string | null;
  if (raw && (VALID_SKINS as readonly string[]).includes(raw)) return raw as Skin;
  // Migrate legacy "chippy" (now removed) → fall back to default.
  // Default: real Microsoft Clippy.
  return "real-clippy";
}

class SkinStore {
  current = $state<Skin>(readInitial());
  private subscribed = false;

  /** Subscribe to cross-window skin updates. Both windows call this. */
  async subscribe() {
    if (this.subscribed) return;
    this.subscribed = true;
    await listen<string>(EVENT, (e) => {
      const v = e.payload as Skin;
      if (VALID_SKINS.includes(v)) {
        this.current = v;
        try {
          localStorage.setItem(STORAGE_KEY, v);
        } catch {}
      }
    });
  }

  /** Set the skin and broadcast — call from the sidebar picker. */
  async set(s: Skin) {
    this.current = s;
    try {
      localStorage.setItem(STORAGE_KEY, s);
    } catch {}
    try {
      await emit(EVENT, s);
    } catch {}
  }
}

export const skinStore = new SkinStore();

/** Show or hide the Clippy floater window from main-window code. */
export async function setClippyWindowVisible(visible: boolean) {
  try {
    const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
    const w = await WebviewWindow.getByLabel("clippy");
    if (!w) return;
    if (visible) {
      await w.show();
    } else {
      await w.hide();
    }
  } catch (e) {
    console.warn("setClippyWindowVisible failed", e);
  }
}
