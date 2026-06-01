// Floater (Clippy) skin selection — shared between the main window and the
// Clippy window via Tauri events. localStorage persists across restarts.
//
// Skin values:
//   "off"         — floater window hidden
//   "fox"         — watercolor fox PNGs (default; uses the asset pack at
//                   static/fox/*.png; state-mapped between
//                   sitting/recording/curious/success/error)
//   "stylized"    — hand-built SVG paperclip (dark outline, transparent body)
//   "real-clippy" — Microsoft Clippy via clippyts
//   "cat"         — SVG charcoal desk cat (green eyes, slit pupils)
//   "cat-lab"     — experimental refined charcoal cat (thin edge highlights,
//                   lighter belly/paws, defined neck + mouth + tail). Lives
//                   side-by-side with "cat" while we iterate on legibility
//                   over dark wallpapers. Same animations + bubble theme.
//
// Removed in v1.0.0-nightly.5: "beige" — the cream-variant paperclip.
// Removed in v1.1.0-nightly.5: "duck" — the rubber-duck design didn't
// land visually and was retired before stabilising. Saved value migrates
// to "fox" on load.

import { emit, listen } from "@tauri-apps/api/event";

export type Skin = "off" | "fox" | "stylized" | "real-clippy" | "cat" | "cat-lab";

const STORAGE_KEY = "wispr.clippy.skin";
const EVENT = "wispr:skin-change";

const VALID_SKINS: readonly Skin[] = ["off", "fox", "stylized", "real-clippy", "cat", "cat-lab"] as const;

function readInitial(): Skin {
  const raw = (typeof localStorage !== "undefined"
    ? localStorage.getItem(STORAGE_KEY)
    : null) as string | null;
  // Migrate retired "beige" → "stylized" (same paperclip shape, different theme).
  if (raw === "beige") return "stylized";
  // Migrate retired "duck" → "fox" (closest "cute mascot" cousin).
  if (raw === "duck") return "fox";
  if (raw && (VALID_SKINS as readonly string[]).includes(raw)) return raw as Skin;
  // Default: the watercolor fox — wispr-fox's own mascot, matches the
  // design playbook. Previously defaulted to real Clippy; new users now
  // land on the fox.
  return "fox";
}

class SkinStore {
  current = $state<Skin>(readInitial());
  private subscribed = false;

  /** Subscribe to cross-window skin updates. Both windows call this. */
  async subscribe() {
    if (this.subscribed) return;
    this.subscribed = true;
    await listen<string>(EVENT, (e) => {
      let v = e.payload as Skin;
      // Migrate stale "beige" / "duck" emissions from older windows still running.
      if ((v as string) === "beige") v = "stylized";
      if ((v as string) === "duck") v = "fox";
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
