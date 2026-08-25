// Typed wrappers around the Rust command surface (commands.rs).
// Field names are snake_case to match Rust serde defaults — keeping them
// aligned avoids a translation layer.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type FlowState =
  | "idle"
  | "recording"
  | "transcribing"
  | "denoising"
  | "cleaning"
  | "injecting";

export type ClippyMode = "light" | "advanced" | "drafting";

export type FlowPhase =
  | "idle"
  | "starting"
  | "recording"
  | "stopping"
  | "processing"
  | "succeeded"
  | "failed";

export type FlowStage = "transcribing" | "denoising" | "cleaning" | "injecting";
export type InputDisposition = "undecided" | "latched" | "hold_to_talk";
export type MicPhase = "inactive" | "waking" | "live" | "unavailable";

export interface FlowNotice {
  code: string;
  severity: "info" | "error";
  summary: string;
  detail_ref: string | null;
}

export interface FlowSnapshot {
  revision: number;
  session_id: string | null;
  phase: FlowPhase;
  stage: FlowStage | null;
  mode: ClippyMode | null;
  input: InputDisposition | null;
  mic: MicPhase;
  mic_ready_ms: number | null;
  notice: FlowNotice | null;
}

export interface MicReadyEvent {
  generation: number;
  source: "dictation" | "preview";
  ready_ms: number;
}

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
  // Serialized compatibility only; adaptive hotkeys ignore these fields.
  sticky_light: boolean;
  sticky_advanced: boolean;
  sticky_drafting: boolean;
  light_sticky_hotkey: string;
  advanced_sticky_hotkey: string;
  drafting_sticky_hotkey: string;
  auto_clean_in_light: boolean;
  auto_clean_in_advanced: boolean;
  auto_clean_in_drafting: boolean;
  auto_title: boolean;
  /** Provider + model for the auto-title call — independent of the main
   *  llm_provider/llm_model so a title stays cheap. */
  title_provider: string;
  title_model: string;
  stt_provider: string;
  stt_model: string;
  llm_provider: string;
  llm_model: string;
  draft_llm_provider: string;
  draft_llm_model: string;
  language_hint: string | null;
  /** Mic noise reduction before STT: "off" | "on" (rumble high-pass) |
   *  "aggressive" (high-pass + RNNoise). Raw WAV on disk is never modified. */
  noise_reduction: string;
  /** Microphone to record from, by device name. null / "" = system default.
   *  A saved device that isn't present when you press the hotkey falls back to
   *  the system default rather than failing the dictation. */
  input_device: string | null;
  /** Boost too-quiet audio before sending it for transcription. On by default:
   *  quiet audio doesn't fail loudly, it comes back with words silently
   *  missing. Only the uploaded copy is boosted; the saved WAV is untouched. */
  auto_gain: boolean;
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
  custom_meeting_prompt: string;
  pull_back_on_navigation: boolean;
  keep_in_clipboard: boolean;
  open_silently: boolean;
  force_clean_hotkey: string;
  force_clean_sticky_hotkey: string;
  /** Global show/hide-the-window combo. Not a dictation binding — it never
   *  starts a recording. Empty string disables it. */
  toggle_window_hotkey: string;
  adapt_to_app: boolean;
  device_name: string;
}

/** Account / sync status (accounts + cross-device sync, v3.0.0). */
/** One device signed into this account, as assembled by
 *  `src-tauri/src/sync/fleet.rs`. `stats` is null for a device that has not
 *  published a rollup yet (an older client, or one that hasn't synced since
 *  the fleet feature shipped) — render "not reporting yet", never a zero. */
export interface FleetDevice {
  id: string;
  name: string | null;
  platform: string | null;
  created_at: string | null;
  last_seen_at: string | null;
  icon: string | null;
  label: string | null;
  this_device: boolean;
  stats: StatsSummary | null;
}

export interface AuthStatus {
  /** Whether this build has a real Supabase project baked in. When false the
   *  account UI shows "Sync not configured in this build" and behaves as
   *  signed-out regardless. */
  configured: boolean;
  signed_in: boolean;
  /** A stored session is being restored right now (the launch-time token
   *  refresh is in flight). `signed_in` is still false, but the UI must show
   *  "Checking…" rather than "Not signed in" — rendering signed-out here is
   *  what made the app look like it logged itself out on every restart. */
  restoring: boolean;
  email: string | null;
  user_id: string | null;
}

export type SyncState = "idle" | "syncing" | "error" | "signed_out";

export interface SyncStatusEvent {
  state: SyncState;
  last_synced_at: string | null;
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
  meeting_notes_text: string | null;
  /** Versioned JSON envelope: {version:1, turns:[{speaker,text,start}]}. */
  speaker_turns: string | null;
  /** JSON object mapping placeholders to user-supplied names. */
  speaker_names: string | null;
  is_meeting: boolean;
  diarization_enabled: boolean;
  stt_provider: string | null;
  llm_provider: string | null;
  clippy_used: boolean;
  clippy_note: string | null;
  retry_count: number;
  error: string | null;
  /** LLM-generated one-line name; arrives asynchronously after the run. */
  title: string | null;
  /** Wall-clock ms the STT request took (the key debugging number). */
  stt_ms: number | null;
  /** Wall-clock ms the LLM cleanup/draft took, or null if it didn't run. */
  cleanup_ms: number | null;
  /** End-to-end turnaround ms, recording-stopped → text-delivered. */
  total_ms: number | null;
  /** JSON `[{ms,msg}]` timeline of the run; null on pre-nightly.7 rows. */
  event_log: string | null;
  /** Audio actually captured to the WAV (ms). Compare to duration_ms: if it's
   *  materially smaller, the mic dropped mid-recording and the transcript is
   *  truncated. Null on pre-nightly.8 rows. */
  audio_captured_ms: number | null;
  /** How the recording entered the app: "mic" (live dictation) or "upload"
   *  (a user-supplied audio file). Drives the "Uploaded" badge. */
  source: string;
  /** Which client made this recording: "desktop" | "web" | "mobile". Drives
   *  the per-row platform badge. NULL rows read as "desktop". */
  platform: string;
  /** Device name recorded at capture time (tooltip on the platform badge). */
  device_name: string | null;
  /** True when this row was pulled from another device via sync — no local
   *  audio exists, so playback is hidden/disabled. */
  remote: boolean;
}

/** Options for a one-off upload transcription. Null provider/model = use the
 *  current global setting; cleanup/draft add the matching version columns. */
export interface UploadOptions {
  sttProvider?: string | null;
  sttModel?: string | null;
  llmProvider?: string | null;
  llmModel?: string | null;
  draftLlmProvider?: string | null;
  draftLlmModel?: string | null;
  cleanup: boolean;
  draft: boolean;
  /** Ask the provider to label who is speaking. Only Deepgram and ElevenLabs
   *  can — Whisper (Groq/OpenAI) has no speaker model. Enforced in Rust too. */
  diarize: boolean;
  /** Summarise the transcript into meeting notes (summary / decisions /
   *  action items with owners). Writes to the Drafted column. */
  meetingNotes: boolean;
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

/** One release as the About screen shows it. */
export interface ReleaseInfo {
  tag: string;
  version: string;
  html_url: string;
  published_at: string | null;
  prerelease: boolean;
  /** Newer than the running build. */
  newer: boolean;
  /** Installer for THIS platform. Null means the release shipped nothing we
   *  can install here — offer the release page, not an Install button. */
  asset: { name: string; url: string; size: number } | null;
  summary: string | null;
}

export interface UpdateStatus {
  current: string;
  current_is_nightly: boolean;
  stable: ReleaseInfo | null;
  /** Only set when a pre-release is NEWER than the newest stable. */
  nightly: ReleaseInfo | null;
  /** Whether this platform can install in place (Windows). */
  can_self_install: boolean;
  checked_at: string;
}

export interface UpdateProgress {
  phase: "starting" | "downloading" | "launching";
  downloaded: number;
  total: number;
  tag: string;
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
  meeting: string;
}

export const api = {
  ping: () => invoke<string>("ping"),
  getFlowSnapshot: () => invoke<FlowSnapshot>("get_flow_snapshot"),
  checkSecrets: () => invoke<SecretCheck>("check_secrets"),
  secretsDiagnostic: () => invoke<SecretsDiagnostic>("secrets_diagnostic"),
  secretAuditLog: (limit = 100) => invoke<SecretAuditEvent[]>("secret_audit_log", { limit }),
  floaterTrigger: (mode: "light" | "advanced" | "drafting") =>
    invoke<void>("floater_trigger", { mode }),
  setClickthrough: (ignore: boolean) =>
    invoke<void>("set_clickthrough", { ignore }),
  revealFolder: (kind: "audio" | "sounds" | "avatars" | "data") =>
    invoke<void>("reveal_folder", { kind }),
  saveSecret: (key: SecretKeyName, value: string) =>
    invoke<void>("save_secret", { key, value }),
  deleteSecret: (key: SecretKeyName) => invoke<void>("delete_secret", { key }),
  getSettings: () => invoke<AppSettings>("get_settings"),
  setSettings: (settings: AppSettings) =>
    invoke<void>("set_settings", { settings }),
  listHistory: (limit = 100) => invoke<Recording[]>("list_history", { limit }),
  deleteRecording: (id: string) => invoke<void>("delete_recording", { id }),
  /** Ownership-scoped delete: removes only rows this device originated
   *  (transcript + audio together), tombstoning the cloud copy so other
   *  devices drop it too. `ids` omitted = all of this device's recordings. */
  deleteRecordings: (ids?: string[] | null) =>
    invoke<number>("delete_recordings", { ids: ids ?? null }),
  /** Account-wide purge — resets the entire account history on every device,
   *  including orphaned rows. Deliberate, irreversible; signed-in only. */
  purgeAccount: () => invoke<void>("purge_account"),
  retryRecording: (id: string) => invoke<void>("retry_recording", { id }),
  // ── Accounts + cross-device sync ──────────────────────────────────────
  authStatus: () => invoke<AuthStatus>("auth_status"),
  signInEmail: (email: string, password: string) =>
    invoke<AuthStatus>("sign_in_email", { email, password }),
  signUpEmail: (email: string, password: string) =>
    invoke<AuthStatus>("sign_up_email", { email, password }),
  signInGoogle: () => invoke<AuthStatus>("sign_in_google"),
  cancelGoogleSignIn: () => invoke<void>("cancel_google_sign_in"),
  signOut: () => invoke<AuthStatus>("sign_out"),
  syncNow: () => invoke<void>("sync_now"),
  setDeviceName: (name: string) => invoke<void>("set_device_name", { name }),
  /** Transcribe an on-disk audio file. Returns the new recording id. */
  transcribeUpload: (path: string, opts: UploadOptions) =>
    invoke<string>("transcribe_upload", {
      path,
      sttProvider: opts.sttProvider ?? null,
      sttModel: opts.sttModel ?? null,
      llmProvider: opts.llmProvider ?? null,
      llmModel: opts.llmModel ?? null,
      draftLlmProvider: opts.draftLlmProvider ?? null,
      draftLlmModel: opts.draftLlmModel ?? null,
      cleanup: opts.cleanup,
      draft: opts.draft,
      diarize: opts.diarize,
      meetingNotes: opts.meetingNotes,
    }),
  generateAltVersion: (
    id: string,
    kind: "cleaned" | "drafted" | "meeting_notes",
    opts?: { provider?: string; model?: string },
  ) => invoke<string>("generate_alt_version", { id, kind, provider: opts?.provider ?? null, model: opts?.model ?? null }),
  rerunTranscription: (
    id: string,
    sttProvider: string,
    sttModel: string,
    diarize: boolean,
  ) => invoke<void>("rerun_transcription", { id, sttProvider, sttModel, diarize }),
  setSpeakerNames: (id: string, names: Record<string, string>) =>
    invoke<void>("set_speaker_names", { id, namesJson: JSON.stringify(names) }),
  audioUrlFor: (id: string) => invoke<string>("audio_url_for", { id }),
  audioDataUrlFor: (id: string) => invoke<string>("audio_data_url_for", { id }),
  listInputDevices: () => invoke<InputDeviceInfo[]>("list_input_devices"),
  /** Open a metering-only capture stream so the user can verify their mic.
   *  Returns the RESOLVED device name (may differ from what was asked for if
   *  the saved device is gone). `null` device = system default. */
  startMicTest: (device: string | null) =>
    invoke<string>("start_mic_test", { device }),
  stopMicTest: () => invoke<void>("stop_mic_test"),
  /** Tear down every dictation hotkey — call before capturing a new binding,
   *  or the global shortcut swallows the keypress and fires a recording
   *  instead. ALWAYS pair with applyHotkeys(), including on cancel/unmount. */
  suspendHotkeys: () => invoke<void>("suspend_hotkeys"),
  /** Re-register hotkeys from the saved settings. Resumes after a capture AND
   *  makes a newly-saved binding live without restarting the app. */
  applyHotkeys: () => invoke<void>("apply_hotkeys"),
  hotkeysActive: () => invoke<boolean>("hotkeys_active"),
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
  /** Every device on this account. Hits the network; falls back to the local
   *  cache when offline. */
  listDevices: () => invoke<FleetDevice[]>("list_devices"),
  /** Cached fleet, no network — paints the UI before listDevices resolves. */
  listDevicesCached: () => invoke<FleetDevice[]>("list_devices_cached"),
  /** Assign an icon and/or display label to ANY device on the account.
   *  Returns the refreshed fleet. Pass null to clear a field. */
  setDeviceMeta: (deviceId: string, icon: string | null, label: string | null) =>
    invoke<FleetDevice[]>("set_device_meta", { deviceId, icon, label }),
  /** Both release channels at once, each compared against this build. */
  updateStatus: () => invoke<UpdateStatus>("update_status"),
  /** Download the installer for `tag` and hand it to the OS. On Windows the
   *  app quits so the installer can replace locked files. */
  downloadAndInstall: (tag: string) => invoke<string>("download_and_install", { tag }),
  accessibilityOk: () => invoke<boolean>("accessibility_ok"),
  openAccessibilitySettings: () =>
    invoke<void>("open_accessibility_settings"),
};

/** Download progress while an update is being fetched. */
export function onUpdateProgress(cb: (p: UpdateProgress) => void): Promise<UnlistenFn> {
  return listen<UpdateProgress>("wispr:update_progress", (e) => cb(e.payload));
}

/** Subscribe to flow state transitions emitted by Rust flow.rs. */
export function onFlowState(cb: (s: FlowState) => void): Promise<UnlistenFn> {
  return listen<string>("wispr:state", (e) => cb(e.payload as FlowState));
}

/** Authoritative, revisioned live-dictation lifecycle. */
export function onFlowSnapshot(cb: (s: FlowSnapshot) => void): Promise<UnlistenFn> {
  return listen<FlowSnapshot>("wispr:flow_snapshot", (e) => cb(e.payload));
}

/** Subscribe to flow-error notifications. */
export function onFlowError(cb: (msg: string) => void): Promise<UnlistenFn> {
  return listen<string>("wispr:flow_error", (e) => cb(e.payload));
}

/** Subscribe to sync-status changes emitted by the Rust sync engine. */
export function onSyncStatus(cb: (s: SyncStatusEvent) => void): Promise<UnlistenFn> {
  return listen<SyncStatusEvent>("wispr:sync_status", (e) => cb(e.payload));
}

/** Subscribe to auth-status changes. Emitted after the launch-time session
 *  restore settles and on every sign-in / sign-out, so a window that mounted
 *  mid-restore (or a sidebar that wasn't the one the user signed in from)
 *  corrects itself without needing a remount. */
export function onAuthStatus(cb: (s: AuthStatus) => void): Promise<UnlistenFn> {
  return listen<AuthStatus>("wispr:auth_status", (e) => cb(e.payload));
}

/** Subscribe to "the stored API keys changed underneath you" — currently
 *  fired when a sync cycle adopts keys pushed from another device. Anything
 *  gating UI on `checkSecrets()` must re-read it here, or a freshly-signed-in
 *  device shows every model greyed out until it is restarted. */
export function onSecretsChanged(cb: () => void): Promise<UnlistenFn> {
  return listen("wispr:secrets_changed", () => cb());
}
