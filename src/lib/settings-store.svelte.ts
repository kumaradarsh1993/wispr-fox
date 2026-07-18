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
import { llmModelsFor, sttModelsFor } from "./provider-options";

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
  auto_title: true,
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
  noise_reduction: "off",
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
  device_name: "",
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

    // 2b) macOS hotkey migration. F8/F9 are media keys on Mac, so macOS uses a
    //     single ⌥ chord (⌥Space dictate, ⌥Enter draft, ⌘ for sticky). Two
    //     one-time steps, each gated by a marker so we never clobber a user's
    //     deliberate rebind:
    //       • macHotkeyMigrated  — moves Windows F-key defaults → Mac combos
    //         (covers installs that predate the platform split).
    //       • macHotkeyV2Migrated — moves the OLD ⌃⌥ chord defaults (D/F/C)
    //         that shipped briefly → the new ⌥Space scheme.
    const isMac =
      typeof navigator !== "undefined" && /Mac/i.test(navigator.userAgent);
    if (isMac) {
      const store = await this.getStore().catch(() => null);

      // Step 1: Windows F-key defaults → Mac (⌥Space) defaults.
      let macDone = false;
      try {
        macDone = (await store?.get<boolean>("macHotkeyMigrated")) ?? false;
      } catch {
        /* treat as not-yet-migrated */
      }
      const remap = (field: string, from: string, to: string) => {
        const bag = this.s as unknown as Record<string, unknown>;
        if (bag[field] === from) {
          bag[field] = to;
          migrated = true;
        }
      };
      if (!macDone) {
        remap("light_hotkey", "F8", "Alt+Space");
        remap("drafting_hotkey", "F9", "Alt+Enter");
        remap("light_sticky_hotkey", "Super+F8", "Super+Alt+Space");
        remap("drafting_sticky_hotkey", "Super+F9", "Super+Alt+Enter");
        remap("force_clean_hotkey", "Shift+F8", "Shift+Alt+Space");
        remap("force_clean_sticky_hotkey", "Shift+Super+F8", "Super+Shift+Alt+Space");
        try {
          await store?.set("macHotkeyMigrated", true);
          await store?.save();
        } catch {
          /* best-effort marker */
        }
      }

      // Step 2: old ⌃⌥ chord defaults → the new ⌥Space scheme.
      let macV2Done = false;
      try {
        macV2Done = (await store?.get<boolean>("macHotkeyV2Migrated")) ?? false;
      } catch {
        /* treat as not-yet-migrated */
      }
      if (!macV2Done) {
        remap("light_hotkey", "Ctrl+Alt+D", "Alt+Space");
        remap("drafting_hotkey", "Ctrl+Alt+F", "Alt+Enter");
        remap("light_sticky_hotkey", "Ctrl+Alt+Shift+D", "Super+Alt+Space");
        remap("drafting_sticky_hotkey", "Ctrl+Alt+Shift+F", "Super+Alt+Enter");
        remap("force_clean_hotkey", "Ctrl+Alt+C", "Shift+Alt+Space");
        remap("force_clean_sticky_hotkey", "Ctrl+Alt+Shift+C", "Super+Shift+Alt+Space");
        try {
          await store?.set("macHotkeyV2Migrated", true);
          await store?.save();
        } catch {
          /* best-effort marker */
        }
      }
    }

    // 2c) One-time: force LLM cleanup ON for Draft + Advanced. Those modes
    //     ONLY function with cleanup on (F9 with it off returns the raw
    //     transcript — a silent foot-gun), so the per-mode toggle was removed
    //     from the UI. Marker-gated like the hotkey migrations so a future
    //     deliberate change isn't clobbered on every launch.
    {
      const store = await this.getStore().catch(() => null);
      let draftCleanForced = false;
      try {
        draftCleanForced = (await store?.get<boolean>("draftCleanForced")) ?? false;
      } catch {
        /* treat as not-yet-migrated */
      }
      if (!draftCleanForced) {
        if (!this.s.auto_clean_in_drafting || !this.s.auto_clean_in_advanced) {
          this.s.auto_clean_in_drafting = true;
          this.s.auto_clean_in_advanced = true;
          migrated = true;
        }
        try {
          await store?.set("draftCleanForced", true);
          await store?.save();
        } catch {
          /* best-effort marker */
        }
      }
    }

    // 2d) Sanitize saved model selections against the current provider
    //     catalog (`provider-options.ts`). Providers occasionally retire a
    //     model id upstream (e.g. Groq dropping `distil-whisper-large-v3-en`,
    //     ElevenLabs retiring `scribe_v1`) — a saved id that's no longer
    //     listed can't match any <option> in the sidebar/Settings model
    //     <select>s (renders with nothing visibly selected) AND would keep
    //     getting sent straight to the provider on every call, which just
    //     4xxs. Coerce to that provider's first listed model instead, same
    //     fallback `applySttProvider`/`applyLlmProvider` already use when the
    //     user switches providers by hand.
    {
      const sttOptions = sttModelsFor(this.s.stt_provider);
      if (sttOptions.length > 0 && !sttOptions.some((m) => m.id === this.s.stt_model)) {
        console.info(
          `settings.init: saved STT model "${this.s.stt_model}" is no longer offered by ${this.s.stt_provider}; falling back to ${sttOptions[0].id}`,
        );
        this.s.stt_model = sttOptions[0].id;
        migrated = true;
      }
      const llmOptions = llmModelsFor(this.s.llm_provider);
      if (llmOptions.length > 0 && !llmOptions.some((m) => m.id === this.s.llm_model)) {
        console.info(
          `settings.init: saved LLM model "${this.s.llm_model}" is no longer offered by ${this.s.llm_provider}; falling back to ${llmOptions[0].id}`,
        );
        this.s.llm_model = llmOptions[0].id;
        migrated = true;
      }
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

  async setMany(patch: Partial<AppSettings>) {
    this.s = { ...this.s, ...patch };
    await api.setSettings(this.s).catch((e) => {
      console.error("settings.setMany: Rust push failed", e);
    });
    try {
      await this.persist();
    } catch (e) {
      console.error("settings.setMany: disk persist failed", e);
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
