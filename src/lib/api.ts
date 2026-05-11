// Typed wrappers around the Rust command surface (commands.rs).
// Field names are snake_case to match Rust serde defaults — keeping them
// aligned avoids a translation layer.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type FlowState = "idle" | "recording" | "transcribing" | "cleaning" | "injecting";

export type ClippyMode = "light" | "advanced";

export type RecordingStatus =
  | "recording"
  | "transcribing"
  | "cleaning"
  | "injecting"
  | "done"
  | "error";

export interface AppSettings {
  light_hotkey: string;
  advanced_hotkey: string;
  drafting_hotkey: string;
  sticky_light: boolean;
  sticky_advanced: boolean;
  sticky_drafting: boolean;
  light_sticky_hotkey: string;
  advanced_sticky_hotkey: string;
  drafting_sticky_hotkey: string;
  auto_clean_in_light: boolean;
  auto_clean_in_advanced: boolean;
  auto_clean_in_drafting: boolean;
  stt_provider: string;
  stt_model: string;
  llm_provider: string;
  llm_model: string;
  language_hint: string | null;
  // Legacy per-mode fields — kept for backwards compat, not used by UI.
  clippy_light_model: string;
  clippy_advanced_model: string;
  clippy_drafting_model: string;
  light_provider: string;
  advanced_provider: string;
  drafting_provider: string;
  retention_days: number;
  retention_max_mb: number;
  autostart: boolean;
  start_sound: string;
  stop_sound: string;
  cues_enabled: boolean;
  theme: string;
}

export interface Recording {
  id: string;
  created_at: string;
  audio_path: string;
  duration_ms: number;
  mode: ClippyMode;
  status: RecordingStatus;
  transcript: string | null;
  cleaned_text: string | null;
  stt_provider: string | null;
  llm_provider: string | null;
  clippy_used: boolean;
  clippy_note: string | null;
  retry_count: number;
  error: string | null;
}

export interface SecretCheck {
  stt: boolean;
  llm: boolean;
  gemini?: boolean;
}

export interface InputDeviceInfo {
  name: string;
  is_default: boolean;
}

export interface AppPaths {
  audio_dir: string;
  db_path: string;
  sounds_dir: string;
}

export type SecretKeyName = "groq_stt" | "groq_llm" | "gemini_llm";

export interface DailyUsage {
  date: string;
  stt_count: number;
  llm_count: number;
}

export interface CurrentModels {
  stt: string;
  llm_light: string;
  llm_advanced: string;
}

export const api = {
  ping: () => invoke<string>("ping"),
  checkSecrets: () => invoke<SecretCheck>("check_secrets"),
  saveSecret: (key: SecretKeyName, value: string) =>
    invoke<void>("save_secret", { key, value }),
  deleteSecret: (key: SecretKeyName) => invoke<void>("delete_secret", { key }),
  getSettings: () => invoke<AppSettings>("get_settings"),
  setSettings: (settings: AppSettings) =>
    invoke<void>("set_settings", { settings }),
  listHistory: (limit = 100) => invoke<Recording[]>("list_history", { limit }),
  deleteRecording: (id: string) => invoke<void>("delete_recording", { id }),
  retryRecording: (id: string) => invoke<void>("retry_recording", { id }),
  audioUrlFor: (id: string) => invoke<string>("audio_url_for", { id }),
  audioDataUrlFor: (id: string) => invoke<string>("audio_data_url_for", { id }),
  listInputDevices: () => invoke<InputDeviceInfo[]>("list_input_devices"),
  appPaths: () => invoke<AppPaths>("app_paths"),
  dailyUsage: () => invoke<DailyUsage>("daily_usage"),
  currentModels: () => invoke<CurrentModels>("current_models"),
  clearAllHistory: () => invoke<number>("clear_all_history"),
  listNotificationSounds: () => invoke<string[]>("list_notification_sounds"),
  addNotificationSound: (srcPath: string) =>
    invoke<string>("add_notification_sound", { srcPath }),
  testGroqKey: (key: string) => invoke<string[]>("test_groq_key", { key }),
  testGeminiKey: (key: string) => invoke<string[]>("test_gemini_key", { key }),
  testSavedGroqKey: () => invoke<string[]>("test_saved_groq_key"),
  testSavedGeminiKey: () => invoke<string[]>("test_saved_gemini_key"),
  configureCues: (start: string, stop: string, enabled: boolean) =>
    invoke<void>("configure_cues", { start, stop, enabled }),
};

/** Subscribe to flow state transitions emitted by Rust flow.rs. */
export function onFlowState(cb: (s: FlowState) => void): Promise<UnlistenFn> {
  return listen<string>("wispr:state", (e) => cb(e.payload as FlowState));
}

/** Subscribe to flow-error notifications. */
export function onFlowError(cb: (msg: string) => void): Promise<UnlistenFn> {
  return listen<string>("wispr:flow_error", (e) => cb(e.payload));
}
