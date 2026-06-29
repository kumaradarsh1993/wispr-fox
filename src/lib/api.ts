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
  custom_light_prompt: string;
  custom_advanced_prompt: string;
  custom_drafting_prompt: string;
  pull_back_on_navigation: boolean;
  keep_in_clipboard: boolean;
  open_silently: boolean;
  force_clean_hotkey: string;
  force_clean_sticky_hotkey: string;
  adapt_to_app: boolean;
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
  drafted_text: string | null;
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
  openai_stt: boolean;
  openai_llm: boolean;
  deepgram_stt: boolean;
  elevenlabs_stt: boolean;
  any_stt: boolean;
}

export type SecretLocation = "keyring" | "encrypted_file" | "file" | "legacy_file" | "none";

export interface SecretsDiagnostic {
  stt: SecretLocation;
  llm: SecretLocation;
  gemini: SecretLocation;
  openai_stt: SecretLocation;
  openai_llm: SecretLocation;
  deepgram_stt: SecretLocation;
  elevenlabs_stt: SecretLocation;
  keyring_works: boolean;
  fallback_path: string;
  fallback_exists: boolean;
  encrypted_fallback_path: string;
  encrypted_fallback_exists: boolean;
  legacy_fallback_path: string;
  legacy_fallback_exists: boolean;
  audit_log_path: string;
}

export interface SecretAuditEvent {
  ts: string;
  key: string;
  label: string;
  action: string;
  storage: string;
  outcome: string;
  detail: string;
}

export interface UpdateInfo {
  current: string;
  latest: string;
  newer: boolean;
  html_url: string;
  prerelease: boolean;
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

export type SecretKeyName =
  | "groq_stt"
  | "groq_llm"
  | "gemini_llm"
  | "openai_stt"
  | "openai_llm"
  | "deepgram_stt"
  | "elevenlabs_stt";

export interface DailyUsage {
  date: string;
  stt_count: number;
  llm_count: number;
  deepgram_audio_seconds: number;
  deepgram_estimated_usd: number;
  deepgram_free_credit_usd: number;
  deepgram_rate_usd_per_min: number;
  model_usage: ModelUsage[];
  recent_days: UsageDay[];
}

export interface ModelUsage {
  stage: "stt" | "llm" | string;
  provider: string;
  model: string;
  calls: number;
  audio_seconds: number;
  input_tokens: number;
  output_tokens: number;
  total_tokens: number;
  estimated_usd: number;
}

export interface UsageDay {
  date: string;
  model_usage: ModelUsage[];
}

export interface DailyStat {
  date: string;
  sessions: number;
  words: number;
  dictation_ms: number;
  light_count: number;
  draft_count: number;
}

export interface StatsSummary {
  days: DailyStat[];
  total_sessions: number;
  total_words: number;
  total_dictation_ms: number;
  first_day: string | null;
}

export interface CurrentModels {
  stt: string;
  llm_light: string;
  llm_advanced: string;
}

export interface DefaultPrompts {
  light: string;
  advanced: string;
  drafting: string;
}

export const api = {
  ping: () => invoke<string>("ping"),
  checkSecrets: () => invoke<SecretCheck>("check_secrets"),
  secretsDiagnostic: () => invoke<SecretsDiagnostic>("secrets_diagnostic"),
  secretAuditLog: (limit = 100) => invoke<SecretAuditEvent[]>("secret_audit_log", { limit }),
  floaterTrigger: (mode: "light" | "advanced" | "drafting") =>
    invoke<void>("floater_trigger", { mode }),
  setClickthrough: (ignore: boolean) =>
    invoke<void>("set_clickthrough", { ignore }),
  revealFolder: (kind: "audio" | "sounds" | "avatars" | "data") =>
    invoke<void>("reveal_folder", { kind }),
  checkForUpdates: () => invoke<UpdateInfo>("check_for_updates"),
  saveSecret: (key: SecretKeyName, value: string) =>
    invoke<void>("save_secret", { key, value }),
  deleteSecret: (key: SecretKeyName) => invoke<void>("delete_secret", { key }),
  getSettings: () => invoke<AppSettings>("get_settings"),
  setSettings: (settings: AppSettings) =>
    invoke<void>("set_settings", { settings }),
  listHistory: (limit = 100) => invoke<Recording[]>("list_history", { limit }),
  deleteRecording: (id: string) => invoke<void>("delete_recording", { id }),
  retryRecording: (id: string) => invoke<void>("retry_recording", { id }),
  generateAltVersion: (id: string, kind: "cleaned" | "drafted") =>
    invoke<string>("generate_alt_version", { id, kind }),
  audioUrlFor: (id: string) => invoke<string>("audio_url_for", { id }),
  audioDataUrlFor: (id: string) => invoke<string>("audio_data_url_for", { id }),
  listInputDevices: () => invoke<InputDeviceInfo[]>("list_input_devices"),
  appPaths: () => invoke<AppPaths>("app_paths"),
  dailyUsage: () => invoke<DailyUsage>("daily_usage"),
  statsSummary: () => invoke<StatsSummary>("stats_summary"),
  currentModels: () => invoke<CurrentModels>("current_models"),
  getDefaultPrompts: () => invoke<DefaultPrompts>("get_default_prompts"),
  clearAllHistory: () => invoke<number>("clear_all_history"),
  listNotificationSounds: () => invoke<string[]>("list_notification_sounds"),
  addNotificationSound: (srcPath: string) =>
    invoke<string>("add_notification_sound", { srcPath }),
  testGroqKey: (key: string) => invoke<string[]>("test_groq_key", { key }),
  testGeminiKey: (key: string) => invoke<string[]>("test_gemini_key", { key }),
  testOpenAiKey: (key: string) => invoke<string[]>("test_openai_key", { key }),
  testDeepgramKey: (key: string) => invoke<string[]>("test_deepgram_key", { key }),
  testElevenLabsKey: (key: string) => invoke<string[]>("test_elevenlabs_key", { key }),
  testSavedGroqKey: () => invoke<string[]>("test_saved_groq_key"),
  testSavedGeminiKey: () => invoke<string[]>("test_saved_gemini_key"),
  testSavedOpenAiKey: () => invoke<string[]>("test_saved_openai_key"),
  testSavedDeepgramKey: () => invoke<string[]>("test_saved_deepgram_key"),
  testSavedElevenLabsKey: () => invoke<string[]>("test_saved_elevenlabs_key"),
  configureCues: (start: string, stop: string, enabled: boolean) =>
    invoke<void>("configure_cues", { start, stop, enabled }),
  // macOS auto-paste permission (Accessibility). Always true off-macOS.
  accessibilityOk: () => invoke<boolean>("accessibility_ok"),
  openAccessibilitySettings: () =>
    invoke<void>("open_accessibility_settings"),
};

/** Subscribe to flow state transitions emitted by Rust flow.rs. */
export function onFlowState(cb: (s: FlowState) => void): Promise<UnlistenFn> {
  return listen<string>("wispr:state", (e) => cb(e.payload as FlowState));
}

/** Subscribe to flow-error notifications. */
export function onFlowError(cb: (msg: string) => void): Promise<UnlistenFn> {
  return listen<string>("wispr:flow_error", (e) => cb(e.payload));
}
