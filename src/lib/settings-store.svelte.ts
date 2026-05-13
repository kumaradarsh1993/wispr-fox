// Runes-backed settings store.
//
// Persistence model (since v0.3.2):
//   1. On init, read from `user-prefs.json` via tauri-plugin-store (lives
//      in the app data dir — survives reinstalls). Fall back to Rust's
//      in-memory defaults if the file is missing or unreadable.
//   2. Push the loaded values to Rust via `set_settings` so the backend
//      flow has the right state (hotkeys, prompts, etc.).
//   3. On every `set()`, write through to both Rust AND the disk store.
//
// Prior to v0.3.2 the store was in-memory only on the Rust side, which
// meant theme / model picks / hotkey rebinds reset to defaults on every
// app launch. Real bug, reported by user; fixed here.

import { Store, type Store as StoreType } from "@tauri-apps/plugin-store";
import { api, type AppSettings } from "./api";

const STORE_FILE = "user-prefs.json";
const STORE_KEY = "settings";

const FALLBACK: AppSettings = {
  // Hotkey defaults: F10 retired (Windows reserves it for menu activation,
  // breaks Outlook). Drafting moved to F9. Advanced cleanup is opt-in via
  // F8's "LLM cleanup" toggle — no dedicated hotkey by default.
  light_hotkey: "F8",
  advanced_hotkey: "",
  drafting_hotkey: "F9",
  sticky_light: false,
  sticky_advanced: false,
  sticky_drafting: false,
  light_sticky_hotkey: "Super+F8",
  advanced_sticky_hotkey: "",
  drafting_sticky_hotkey: "Super+F9",
  auto_clean_in_light: false,
  auto_clean_in_advanced: true,
  auto_clean_in_drafting: true,
  stt_provider: "groq",
  llm_provider: "groq",
  llm_model: "llama-3.3-70b-versatile",
  clippy_light_model: "llama-3.3-70b-versatile",
  clippy_advanced_model: "llama-3.3-70b-versatile",
  clippy_drafting_model: "llama-3.3-70b-versatile",
  light_provider: "groq",
  advanced_provider: "groq",
  drafting_provider: "groq",
  stt_model: "whisper-large-v3-turbo",
  language_hint: null,
  retention_days: 7,
  retention_max_mb: 500,
  autostart: false,
  start_sound: "",
  stop_sound: "",
  cues_enabled: true,
  theme: "light",
  custom_light_prompt: "",
  custom_advanced_prompt: "",
  custom_drafting_prompt: "",
  pull_back_on_navigation: false,
  keep_in_clipboard: true,
  open_silently: true,
  force_clean_hotkey: "Shift+F8",
  force_clean_sticky_hotkey: "Shift+Super+F8",
  adapt_to_app: true,
};

class SettingsStore {
  s = $state<AppSettings>({ ...FALLBACK });
  private ready = false;
  private storePromise: Promise<StoreType> | null = null;

  /** Lazy-init the on-disk store. Cached so we don't re-open per call. */
  private getStore(): Promise<StoreType> {
    if (!this.storePromise) {
      this.storePromise = Store.load(STORE_FILE);
    }
    return this.storePromise;
  }

  async init() {
    if (this.ready) return;

    // 1) Read from disk if present. Missing file / parse failure → silently
    //    fall through to Rust defaults; user just sees first-run state.
    let fromDisk: AppSettings | null = null;
    try {
      const store = await this.getStore();
      const raw = await store.get<AppSettings>(STORE_KEY);
      if (raw && typeof raw === "object") {
        // Merge with FALLBACK so any new fields added in this version
        // get sensible defaults even if the saved file pre-dates them.
        fromDisk = { ...FALLBACK, ...raw };
      }
    } catch (e) {
      console.warn("settings.init: disk read failed, will use defaults", e);
    }

    if (fromDisk) {
      this.s = fromDisk;
    } else {
      // No saved file — try Rust's defaults as a starting point so e.g.
      // any future server-controlled defaults flow in correctly.
      try {
        const fromRust = await api.getSettings();
        this.s = fromRust;
      } catch (e) {
        console.warn("settings.init: Rust get_settings failed; using FALLBACK", e);
        this.s = { ...FALLBACK };
      }
    }

    // 2) One-time migration: users upgrading from <=0.1.1 have the old
    //    F10 drafting / F9 advanced defaults stored. F10 is cursed on
    //    Windows (menu activation in Outlook) — silently remap.
    let migrated = false;
    if (this.s.drafting_hotkey === "F10") {
      this.s.drafting_hotkey = "F9";
      migrated = true;
    }
    if (this.s.drafting_sticky_hotkey === "Super+F10") {
      this.s.drafting_sticky_hotkey = "Super+F9";
      migrated = true;
    }
    if (this.s.advanced_hotkey === "F9") {
      this.s.advanced_hotkey = "";
      this.s.advanced_sticky_hotkey = "";
      migrated = true;
    }

    // 3) Push to Rust so the backend flow has the right state.
    try {
      await api.setSettings(this.s);
    } catch (e) {
      console.warn("settings.init: push to Rust failed (non-fatal)", e);
    }

    // 4) If we migrated, persist the post-migration shape so the user
    //    only pays the migration cost once.
    if (migrated) {
      console.info("settings.init: migrated v0.1.x hotkeys to new F8/F9 model");
      this.persist().catch((e) => console.warn("settings.init: migration save failed", e));
    } else if (!fromDisk) {
      // First-ever run on this machine — persist the defaults so future
      // restarts hit the disk path, not the Rust-fallback path.
      this.persist().catch((e) => console.warn("settings.init: first-run save failed", e));
    }

    this.ready = true;

    // 5) Push audio-cue selection into the cue worker.
    try {
      await api.configureCues(this.s.start_sound, this.s.stop_sound, this.s.cues_enabled);
    } catch (e) {
      console.warn("settings.init: configureCues failed", e);
    }
  }

  /** Write current settings to the on-disk store and flush. */
  private async persist(): Promise<void> {
    const store = await this.getStore();
    await store.set(STORE_KEY, this.s);
    await store.save();
  }

  async set<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
    this.s[key] = value;
    // Best-effort: keep Rust + disk in sync. Failures log but don't throw —
    // the in-memory state is the source of truth for the current session.
    await api.setSettings(this.s).catch((e) => {
      console.error("settings.set: Rust push failed", e);
    });
    try {
      await this.persist();
    } catch (e) {
      console.error("settings.set: disk persist failed", e);
    }
  }

  async replace(next: AppSettings) {
    this.s = { ...next };
    await api.setSettings(this.s);
    try {
      await this.persist();
    } catch (e) {
      console.error("settings.replace: disk persist failed", e);
    }
  }
}

export const settings = new SettingsStore();
