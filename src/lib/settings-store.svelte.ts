// Runes-backed settings store, mirroring md-reader's *-store.svelte.ts pattern.
// Reads from the Rust `get_settings` command on init, writes back via
// `set_settings` on every mutation.

import { api, type AppSettings } from "./api";

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
};

class SettingsStore {
  s = $state<AppSettings>({ ...FALLBACK });
  private ready = false;

  async init() {
    if (this.ready) return;
    try {
      const fromRust = await api.getSettings();
      this.s = fromRust;
    } catch (e) {
      console.warn("settings.init: falling back to defaults", e);
    }

    // One-time migration: users upgrading from <=0.1.1 have the old F10
    // drafting / F9 advanced defaults stored. F10 is cursed on Windows
    // (menu activation in Outlook) — silently remap to the new model so
    // they don't have to relearn or hit the bug again.
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
      // F9 was advanced cleanup — now collapsed into the F8 toggle.
      this.s.advanced_hotkey = "";
      this.s.advanced_sticky_hotkey = "";
      migrated = true;
    }
    if (migrated) {
      try {
        await api.setSettings(this.s);
        console.info("settings.init: migrated v0.1.x hotkeys to new F8/F9 model");
      } catch (e) {
        console.warn("settings.init: migration save failed", e);
      }
    }

    this.ready = true;

    // Push the user's audio-cue selection into the cue worker so it picks
    // the right files on the next F8 / F9 press. Idempotent; safe to call
    // every init.
    try {
      await api.configureCues(this.s.start_sound, this.s.stop_sound, this.s.cues_enabled);
    } catch (e) {
      console.warn("settings.init: configureCues failed", e);
    }
  }

  async set<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
    this.s[key] = value;
    await api.setSettings(this.s).catch((e) => {
      console.error("settings.set failed", e);
    });
  }

  async replace(next: AppSettings) {
    this.s = { ...next };
    await api.setSettings(this.s);
  }
}

export const settings = new SettingsStore();
