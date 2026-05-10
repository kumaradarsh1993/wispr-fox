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
  auto_clean_in_light: boolean;
  clippy_light_model: string;
  clippy_advanced_model: string;
  stt_model: string;
  language_hint: string | null;
  retention_days: number;
  retention_max_mb: number;
  autostart: boolean;
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
}

export interface InputDeviceInfo {
  name: string;
  is_default: boolean;
}

export interface AppPaths {
  audio_dir: string;
  db_path: string;
}

export type SecretKeyName = "groq_stt" | "groq_llm";

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
  listInputDevices: () => invoke<InputDeviceInfo[]>("list_input_devices"),
  appPaths: () => invoke<AppPaths>("app_paths"),
};

/** Subscribe to flow state transitions emitted by Rust flow.rs. */
export function onFlowState(cb: (s: FlowState) => void): Promise<UnlistenFn> {
  return listen<string>("wispr:state", (e) => cb(e.payload as FlowState));
}

/** Subscribe to flow-error notifications. */
export function onFlowError(cb: (msg: string) => void): Promise<UnlistenFn> {
  return listen<string>("wispr:flow_error", (e) => cb(e.payload));
}
