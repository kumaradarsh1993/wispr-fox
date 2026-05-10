// Runes-backed settings store, mirroring md-reader's *-store.svelte.ts pattern.
// Reads from the Rust `get_settings` command on init, writes back via
// `set_settings` on every mutation.

import { api, type AppSettings } from "./api";

const FALLBACK: AppSettings = {
  light_hotkey: "F8",
  advanced_hotkey: "F9",
  auto_clean_in_light: true,
  clippy_light_model: "llama-3.1-8b-instant",
  clippy_advanced_model: "llama-3.3-70b-versatile",
  stt_model: "whisper-large-v3-turbo",
  language_hint: null,
  retention_days: 7,
  retention_max_mb: 500,
  autostart: false,
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
    this.ready = true;
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
