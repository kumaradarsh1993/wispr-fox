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
//   "cat-lab"     — retired pre-stable cat experiment; saved values migrate to "cat".
//   "wave"        — minimal Wispr-Flow-style pill with a live audio waveform;
//                   no character, no bubbles, no quips (feature: wave bar).
//
// NOTE: "off" is no longer a user-facing SKIN. Visibility ("always" / "auto" /
// "hidden") moved to its own axis in avatar-visibility.svelte.ts. The value is
// kept ONLY so we can parse legacy persisted skins and migrate them: an "off"
// skin becomes skin "fox" + visibility "hidden" (handled in the visibility
// store's one-time migration). This store also defensively rewrites any "off"
// it reads to "fox" so the picker/floater never render an "off" placeholder.
//
// Removed in v1.0.0-nightly.5: "beige" — the cream-variant paperclip.
// Removed in v1.1.0-nightly.5: "duck" — the rubber-duck design didn't
// land visually and was retired before stabilising. Saved value migrates
// to "fox" on load.
// Removed in v2.1.0-nightly.4: "duo" (Khaumani & Indy) and the already-retired
// "duo-hd" — the user judged the hand-drawn duo a bad design next to the
// raster Oru & Gujia pack, which is the same two-cat concept done right.
// Both saved values migrate to "oru-gujia".

import { emit, listen } from "@tauri-apps/api/event";

export type Skin =
  | "off"
  | "fox"
  | "codex-fox"
  | "stylized"
  | "real-clippy"
  | "cat"
  | "cat-lab"
  | "oru-gujia"
  | "spark-buddy"
  | "wave"
  | "siri"
  // Terminal pets — Codex CLI sprite-sheet companions (lib/pets.ts).
  | "pet-codex"
  | "pet-dewey"
  | "pet-fireball"
  | "pet-rocky"
  | "pet-seedy"
  | "pet-stacky"
  | "pet-bsod"
  | "pet-null-signal";

const STORAGE_KEY = "wispr.clippy.skin";
const EVENT = "wispr:skin-change";

const VALID_SKINS: readonly Skin[] = [
  "fox",
  "codex-fox",
  "stylized",
  "real-clippy",
  "cat",
  "oru-gujia",
  "spark-buddy",
  "wave",
  "siri",
  "pet-codex",
  "pet-dewey",
  "pet-fireball",
  "pet-rocky",
  "pet-seedy",
  "pet-stacky",
  "pet-bsod",
  "pet-null-signal",
] as const;

function readInitial(): Skin {
  const raw = (typeof localStorage !== "undefined"
    ? localStorage.getItem(STORAGE_KEY)
    : null) as string | null;
  // Migrate retired "off" skin → "fox". "off" is no longer a skin; its hide
  // intent is carried onto the visibility axis by avatar-visibility.svelte.ts.
  if (raw === "off") return "fox";
  // Migrate retired "beige" → "stylized" (same paperclip shape, different theme).
  if (raw === "beige") return "stylized";
  // Migrate retired "duck" → "fox" (closest "cute mascot" cousin).
  if (raw === "duck") return "fox";
  // Migrate retired "cat-lab" → "cat" (same character family, cleaner picker).
  if (raw === "cat-lab") return "cat";
  // Retire both two-cat SVG experiments → the raster Oru & Gujia pack.
  if (raw === "duo" || raw === "duo-hd") return "oru-gujia";
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
      // Migrate stale "off" / "beige" / "duck" emissions from older windows.
      if ((v as string) === "off") v = "fox";
      if ((v as string) === "beige") v = "stylized";
      if ((v as string) === "duck") v = "fox";
      if ((v as string) === "cat-lab") v = "cat";
      if ((v as string) === "duo" || (v as string) === "duo-hd") v = "oru-gujia";
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
    if ((s as string) === "off") s = "fox";
    if ((s as string) === "cat-lab") s = "cat";
    if ((s as string) === "duo" || (s as string) === "duo-hd") s = "oru-gujia";
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

/**
 * Low-level show/hide of the Clippy floater window from main-window code.
 * Prefer `applyVisibilityWindow()` in avatar-visibility.svelte.ts — that's
 * the tri-state-aware entry point. This raw helper is kept for internal use.
 */
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
