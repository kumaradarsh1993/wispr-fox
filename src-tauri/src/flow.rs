//! Serialized live-dictation coordinator plus transcription/delivery pipelines.
//!
//! Owns no `!Send` audio handles — the cpal Stream lives on a dedicated worker
//! thread inside `AudioController`, and we drive recording via async channel
//! calls. That keeps `Flow` Send + Sync, which Tauri requires for `State`.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use parking_lot::Mutex;
use tauri::{AppHandle, Emitter};

use crate::adaptive::{AdaptiveReducer, Availability, Decision};
use crate::audio::{AudioController, FinishedRecording};
use crate::clippy;
use crate::history::{AltKind, History, Status};
use crate::hotkey::{Edge, HotkeyEvent};
use crate::inject;
use crate::llm::{gemini::GeminiLlm, groq::GroqLlm, openai::OpenAiLlm, ClippyMode, LlmProvider};
use crate::secrets::{self, SecretKey};
use crate::settings::{AppSettings, Mode};
use crate::stt::{
    deepgram::DeepgramStt, elevenlabs::ElevenLabsStt, groq::GroqStt, openai::OpenAiStt, SttProvider,
};
use crate::usage::UsageTracker;

const MIN_DURATION_MS: i64 = 300;

/// Deletes the wrapped file when dropped. Used for the denoised side-WAV so
/// every pipeline exit (success, STT error, 120s timeout) cleans it up without
/// each early-return needing to remember to.
struct TempFileGuard(PathBuf);

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// One-per-run latch for the slow-mic floater warning. Affected devices are
/// slow on EVERY press — warning once is enough to send the user to the mic
/// settings without nagging them every dictation.
static SLOW_MIC_WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Head-gap past which the mic is "slow enough that it's eating your opening
/// words". Also the threshold the floater's waiting state escalates at.
pub const SLOW_MIC_MS: i64 = 2500;

/// Heuristic: does this device name look like a Bluetooth audio endpoint?
/// Windows decorates them consistently enough to be useful — "Headset
/// (DJI MIC2 Hands-Free AG Audio)", "Hands-Free AG Audio", "… Stereo" — and
/// getting this wrong only costs the user slightly-off advice, never a
/// dictation. No device-specific branching: any Bluetooth mic has the same
/// profile-negotiation delay.
fn looks_bluetooth(device: &str) -> bool {
    let d = device.to_ascii_lowercase();
    ["hands-free", "handsfree", "headset", "bluetooth", " ag audio", "wireless"]
        .iter()
        .any(|needle| d.contains(needle))
}

/// The slow-mic remediation text. Split by transport because the two causes
/// have genuinely different fixes and the wrong one sends the user hunting
/// through a settings pane that can't help them. Written to fit the floater
/// bubble's hard 2-line cap.
fn slow_mic_message(mic_ready_ms: i64, device: &str) -> String {
    let secs = (mic_ready_ms as f64) / 1000.0;
    if looks_bluetooth(device) {
        format!(
            "Bluetooth mic took {secs:.1}s to start — your first words were cut. Turn noise cancellation OFF on the mic before connecting; that's usually most of the delay."
        )
    } else {
        format!(
            "Your mic took {secs:.1}s to wake up — the first words of every recording are being cut. Usual fix: turn OFF the mic's audio enhancements and exclusive control (Sound settings → your microphone → Properties)."
        )
    }
}

/// Pretty display name for a provider id (as returned by `*.name()`).
fn pretty_provider(name: &str) -> &str {
    match name {
        "deepgram" => "Deepgram",
        "elevenlabs" => "ElevenLabs",
        "gemini" => "Gemini",
        "groq" => "Groq",
        "openai" => "OpenAI",
        other => other,
    }
}

/// Map a raw error string (from `anyhow::Error::to_string()`) to a short
/// user-readable message. We now attribute BOTH the stage (transcription vs
/// cleanup) and the provider where we can tell, so the floater toast says
/// e.g. "Transcription failed (Groq) — network issue" instead of a vague
/// "something went wrong". The stage/provider come from the `.context(...)`
/// wrappers added at each pipeline step.
fn user_friendly_error(raw: &str) -> String {
    let s = raw.to_ascii_lowercase();

    // Missing-key and local failures are stage-agnostic — handle first.
    if s.contains("no groq stt key")
        || s.contains("no groq llm key")
        || s.contains("no gemini api key")
        || s.contains("no openai")
        || s.contains("no deepgram")
        || s.contains("no elevenlabs")
    {
        return "API key missing - open Settings -> Providers & API keys.".to_string();
    }
    if s.contains("recording too short") {
        return "Recording too short — hold the hotkey longer.".to_string();
    }
    if s.contains("injection") || s.contains("clipboard") {
        return "Couldn't paste — text is on the clipboard, press Ctrl+V manually.".to_string();
    }

    // Which stage failed? Inferred from the context wrappers ("Groq Whisper
    // request" / "Whisper STT timed out" for transcription; cleanup wraps
    // mention clean/clippy/draft).
    let stage = if s.contains("whisper") || s.contains("transcrib") {
        "Transcription"
    } else if s.contains("clean") || s.contains("clippy") || s.contains("draft") {
        "Cleanup"
    } else {
        ""
    };
    // Which provider? Gemini only ever appears in cleanup; Whisper/Groq STT
    // is always Groq.
    let provider = if s.contains("gemini") {
        " (Gemini)"
    } else if s.contains("openai") {
        " (OpenAI)"
    } else if s.contains("deepgram") {
        " (Deepgram)"
    } else if s.contains("elevenlabs") {
        " (ElevenLabs)"
    } else if s.contains("groq") || s.contains("whisper") {
        " (Groq)"
    } else {
        ""
    };

    // Short reason phrase.
    let reason = if s.contains("401")
        || s.contains("unauthorized")
        || s.contains("403")
        || s.contains("forbidden")
    {
        "API key rejected — check Settings"
    } else if s.contains("429") || s.contains("rate limit") {
        "rate limit — wait a minute"
    } else if s.contains("timed out") || s.contains("timeout") {
        "took too long — check your connection"
    } else if s.contains("dns")
        || s.contains("no such host")
        || s.contains("connect")
        || s.contains("network")
    {
        "network issue — check your connection"
    } else if s.contains("500")
        || s.contains("502")
        || s.contains("503")
        || s.contains("504")
        || s.contains("upstream")
    {
        "server hiccup — retry from History"
    } else {
        return format!(
            "Something went wrong — {}",
            raw.lines().next().unwrap_or(raw)
        );
    };

    if stage.is_empty() {
        // Couldn't attribute a stage — still give the reason.
        let r = reason.to_string();
        format!("{}{}", r[..1].to_uppercase(), &r[1..])
    } else {
        format!("{stage}{provider} failed — {reason}.")
    }
}

/// Build the floater notice shown when cleanup couldn't run but we still
/// pasted the raw transcript. Non-fatal — the user got their text, they just
/// didn't get the LLM polish, and they deserve to know why.
fn cleanup_failure_message(note: &str, provider: &str) -> String {
    let p = pretty_provider(provider);
    match note {
        "clippy_auth" => format!("Cleanup skipped — {p} key rejected. Pasted raw text."),
        "clippy_rate_limited" => format!("Cleanup skipped — {p} rate limit. Pasted raw text."),
        "clippy_upstream" => format!("Cleanup skipped — {p} server hiccup. Pasted raw text."),
        "clippy_timeout" => format!("Cleanup timed out ({p}) — pasted raw text."),
        _ => format!("Cleanup failed ({p}) — pasted raw text."),
    }
}

/// Pick the user-customised system prompt for the given mode, if set.
/// Empty string means "use the baked-in default" (handled by clippy::clean).
fn custom_prompt_for(s: &AppSettings, mode: Mode) -> Option<String> {
    let raw = match mode {
        Mode::Light => &s.custom_light_prompt,
        Mode::Advanced => &s.custom_advanced_prompt,
        Mode::Drafting => &s.custom_drafting_prompt,
    };
    if raw.trim().is_empty() {
        None
    } else {
        Some(raw.clone())
    }
}

fn mode_to_str(m: Mode) -> &'static str {
    match m {
        Mode::Light => "light",
        Mode::Advanced => "advanced",
        Mode::Drafting => "drafting",
    }
}

/// Construct an LLM provider client for the user-chosen `provider_id`.
/// Falls back to Groq if the id is unrecognised (e.g. stale settings).
fn build_llm_provider(provider_id: &str, model: String) -> Result<Box<dyn LlmProvider>> {
    match provider_id {
        "gemini" => {
            let key = secrets::get(SecretKey::GeminiLlm)?.ok_or_else(|| {
                anyhow!("no Gemini API key - open Settings -> Providers & API keys")
            })?;
            // If user has gemini selected but no model set, fall back to default.
            // Also auto-migrate model ids that Google has retired so saved
            // settings from older builds don't suddenly 404 against the API.
            let m = if !model.starts_with("gemini") {
                crate::llm::gemini::DEFAULT_MODEL.to_string()
            } else if crate::llm::gemini::DEPRECATED_MODELS
                .iter()
                .any(|d| *d == model.as_str())
            {
                tracing::info!(
                    "gemini model {model:?} is deprecated; using {} instead",
                    crate::llm::gemini::DEFAULT_MODEL
                );
                crate::llm::gemini::DEFAULT_MODEL.to_string()
            } else {
                model
            };
            Ok(Box::new(GeminiLlm::new(key, m)))
        }
        "openai" => {
            let key = secrets::get(SecretKey::OpenAiLlm)?
                .or_else(|| secrets::get(SecretKey::OpenAiStt).ok().flatten())
                .ok_or_else(|| {
                    anyhow!("no OpenAI API key - open Settings -> Providers & API keys")
                })?;
            let m = if model.starts_with("gpt-") {
                model
            } else {
                crate::llm::openai::DEFAULT_MODEL.to_string()
            };
            Ok(Box::new(OpenAiLlm::new(key, m)))
        }
        _ => {
            // "groq" or anything unknown -> Groq path.
            let key = secrets::get(SecretKey::GroqLlm)?
                .or_else(|| secrets::get(SecretKey::GroqStt).ok().flatten())
                .ok_or_else(|| {
                    anyhow!("no Groq LLM key - open Settings -> Providers & API keys")
                })?;
            // Keep saved settings from calling models retired by Groq in
            // August 2026, even before the settings UI has sanitised them.
            let model = match model.as_str() {
                "llama-3.1-8b-instant" => crate::llm::groq::DEFAULT_LIGHT_MODEL.to_string(),
                "llama-3.3-70b-versatile" | "meta-llama/llama-4-scout-17b-16e-instruct" => {
                    crate::llm::groq::DEFAULT_ADVANCED_MODEL.to_string()
                }
                _ if model.trim().is_empty() => crate::llm::groq::DEFAULT_LIGHT_MODEL.to_string(),
                _ => model,
            };
            Ok(Box::new(GroqLlm::new(key, model)))
        }
    }
}

/// Model + prompt for the auto-title nicety. Deliberately a light Groq model
/// regardless of the user's main LLM pick — naming a note should be fast and
/// near-free, and Groq is the one key every install has.
const TITLE_SYSTEM: &str = "You title voice notes. Reply with ONLY a short title \
for the user's dictation: 3-7 words, plain language, no quotes, no trailing \
punctuation, written in the same language as the dictation.";

/// One-line descriptor for a finished recording. Ok(None) = transcript too
/// short to be worth an LLM call (the time/duration header is enough).
///
/// `provider`/`model` come from the Settings picker rather than the main LLM
/// pick — see `AppSettings::title_provider`. Any provider works, but the
/// default stays Groq's 8B because a title is five words and shouldn't cost
/// a frontier-model call.
async fn generate_title(text: &str, provider: &str, model: &str) -> Result<Option<String>> {
    let trimmed = text.trim();
    if trimmed.split_whitespace().count() < 8 {
        return Ok(None);
    }
    // The opening of a dictation carries its topic; don't ship a 15-minute
    // monologue to the model just to get five words back.
    let snippet: String = trimmed.chars().take(1200).collect();
    let llm = build_llm_provider(provider, model.to_string())?;
    let out = llm.complete(TITLE_SYSTEM, &snippet, 0.3).await?;
    let title = out
        .text
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if title.is_empty() {
        return Ok(None);
    }
    // Hard cap so a rambling model can't blow up the card header.
    Ok(Some(title.chars().take(80).collect()))
}

fn selected_model_or(default_model: &str, current: &str, provider_models: &[&str]) -> String {
    if provider_models.iter().any(|m| *m == current) {
        current.to_string()
    } else {
        default_model.to_string()
    }
}

/// Construct an STT provider client for the user-chosen `stt_provider`.
/// Unknown/stale settings fall back to Groq to preserve existing installs.
fn build_stt_provider(settings: &AppSettings) -> Result<Box<dyn SttProvider>> {
    match settings.stt_provider.as_str() {
        "openai" => {
            let key = secrets::get(SecretKey::OpenAiStt)?
                .or_else(|| secrets::get(SecretKey::OpenAiLlm).ok().flatten())
                .ok_or_else(|| {
                    anyhow!("no OpenAI STT key - open Settings -> Providers & API keys")
                })?;
            let model = selected_model_or(
                crate::stt::openai::DEFAULT_MODEL,
                &settings.stt_model,
                &[
                    "gpt-transcribe",
                    "gpt-4o-transcribe-diarize",
                    "gpt-4o-transcribe",
                    "gpt-4o-mini-transcribe",
                    "whisper-1",
                ],
            );
            Ok(Box::new(OpenAiStt::with_model(key, model)))
        }
        "deepgram" => {
            let key = secrets::get(SecretKey::DeepgramStt)?.ok_or_else(|| {
                anyhow!("no Deepgram STT key - open Settings -> Providers & API keys")
            })?;
            let model = if settings.stt_model.starts_with("nova-") {
                settings.stt_model.clone()
            } else {
                crate::stt::deepgram::DEFAULT_MODEL.to_string()
            };
            Ok(Box::new(DeepgramStt::with_model(key, model)))
        }
        "elevenlabs" => {
            let key = secrets::get(SecretKey::ElevenLabsStt)?.ok_or_else(|| {
                anyhow!("no ElevenLabs STT key - open Settings -> Providers & API keys")
            })?;
            let model = if settings.stt_model.starts_with("scribe_") {
                settings.stt_model.clone()
            } else {
                crate::stt::elevenlabs::DEFAULT_MODEL.to_string()
            };
            Ok(Box::new(ElevenLabsStt::with_model(key, model)))
        }
        _ => {
            let key = secrets::get(SecretKey::GroqStt)?.ok_or_else(|| {
                anyhow!("no Groq STT key - open Settings -> Providers & API keys")
            })?;
            let model = selected_model_or(
                "whisper-large-v3-turbo",
                &settings.stt_model,
                &[
                    "whisper-large-v3-turbo",
                    "whisper-large-v3",
                    "distil-whisper-large-v3-en",
                ],
            );
            Ok(Box::new(GroqStt::with_model(key, model)))
        }
    }
}

/// Lightweight per-recording flight recorder. Collects timestamped one-line
/// events (`ms` = elapsed since pipeline start) plus the two headline stage
/// durations, so the History (i) inspector can explain a slow or failed run
/// after the fact — "STT 19.2s / cleanup 0.9s / total 20.4s" + a timeline.
/// Persisted once near the end of `do_pipeline` and on every error return.
struct Timeline {
    start: std::time::Instant,
    events: Vec<serde_json::Value>,
    stt_ms: Option<i64>,
    cleanup_ms: Option<i64>,
}

impl Timeline {
    fn new() -> Self {
        Self {
            start: std::time::Instant::now(),
            events: Vec::new(),
            stt_ms: None,
            cleanup_ms: None,
        }
    }

    fn mark(&mut self, msg: impl Into<String>) {
        let ms = self.start.elapsed().as_millis() as u64;
        self.events
            .push(serde_json::json!({ "ms": ms, "msg": msg.into() }));
    }

    fn total_ms(&self) -> i64 {
        self.start.elapsed().as_millis() as i64
    }

    fn json(&self) -> String {
        serde_json::to_string(&self.events).unwrap_or_else(|_| "[]".to_string())
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FlowPhase {
    Idle,
    Starting,
    Recording,
    Stopping,
    Processing,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FlowStage {
    Transcribing,
    Denoising,
    Cleaning,
    Injecting,
}

#[derive(Debug, Clone, Copy, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InputDisposition {
    Undecided,
    Latched,
    HoldToTalk,
}

#[derive(Debug, Clone, Copy, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MicPhase {
    Inactive,
    Waking,
    Live,
    Unavailable,
}

#[derive(Debug, Clone, Copy, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NoticeSeverity {
    Info,
    Error,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FlowNotice {
    pub code: String,
    pub severity: NoticeSeverity,
    pub summary: String,
    pub detail_ref: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FlowSnapshot {
    pub revision: u64,
    pub session_id: Option<String>,
    pub phase: FlowPhase,
    pub stage: Option<FlowStage>,
    pub mode: Option<String>,
    pub input: Option<InputDisposition>,
    pub mic: MicPhase,
    pub mic_ready_ms: Option<i64>,
    pub notice: Option<FlowNotice>,
}

impl Default for FlowSnapshot {
    fn default() -> Self {
        Self {
            revision: 0,
            session_id: None,
            phase: FlowPhase::Idle,
            stage: None,
            mode: None,
            input: None,
            mic: MicPhase::Inactive,
            mic_ready_ms: None,
            notice: None,
        }
    }
}

#[derive(Clone)]
struct SessionContext {
    id: String,
    mode: Mode,
    force_clean: bool,
    capture_generation: u64,
    input: InputDisposition,
    mic: MicPhase,
    mic_ready_ms: Option<i64>,
}

enum RuntimeState {
    Idle,
    Starting {
        session: SessionContext,
        stop_requested: bool,
    },
    Recording {
        session: SessionContext,
        in_flight: InFlight,
    },
    Processing {
        session: SessionContext,
        record_id: String,
    },
}

struct FlowState {
    input: AdaptiveReducer,
    runtime: RuntimeState,
    snapshot: FlowSnapshot,
}

impl Default for FlowState {
    fn default() -> Self {
        Self {
            input: AdaptiveReducer::default(),
            runtime: RuntimeState::Idle,
            snapshot: FlowSnapshot::default(),
        }
    }
}

impl FlowState {
    fn availability(&self) -> Availability {
        match &self.runtime {
            RuntimeState::Idle => Availability::Idle,
            RuntimeState::Starting {
                stop_requested: false,
                ..
            }
            | RuntimeState::Recording { .. } => Availability::Active,
            RuntimeState::Starting {
                stop_requested: true,
                ..
            }
            | RuntimeState::Processing { .. } => Availability::Busy,
        }
    }

    fn revise(
        &mut self,
        phase: FlowPhase,
        stage: Option<FlowStage>,
        notice: Option<FlowNotice>,
    ) -> FlowSnapshot {
        self.snapshot.revision = self.snapshot.revision.saturating_add(1);
        self.snapshot.phase = phase;
        self.snapshot.stage = stage;
        self.snapshot.notice = notice;
        self.snapshot.clone()
    }
}

struct InFlight {
    mode: Mode,
    record_id: String,
    audio_path: PathBuf,
    /// Foreground window + focused control at recording start. Restored
    /// just before injection to defeat focus drift during the LLM gap
    /// (F10 menu activation in Outlook, user clicking away, notification
    /// stealers, etc.). None when capture wasn't possible — e.g. when the
    /// user dictated into wispr-fox's own window.
    captured_focus: Option<inject::focus::CapturedFocus>,
    /// Set by the Shift+F8 hotkey: force the cleanup pass on (override
    /// `auto_clean_in_light` to true) for this single invocation, without
    /// persisting the setting change.
    force_clean: bool,
}

#[derive(Clone)]
pub struct Flow {
    /// Serializes state mutation with its externally visible side effects
    /// (Escape registration and snapshot/legacy events). The state mutex alone
    /// was insufficient: a Stop could publish/disarm, then an older Start
    /// handler could resume and re-arm/publish its stale revision.
    transitions: Arc<Mutex<()>>,
    state: Arc<Mutex<FlowState>>,
    /// Latest Escape-stop registration intent. See `EscapeIntent`.
    escape: Arc<Mutex<EscapeIntent>>,
    history: History,
    settings: Arc<Mutex<AppSettings>>,
    audio_dir: PathBuf,
    audio: AudioController,
    usage: UsageTracker,
}

enum FlowAction {
    Start(SessionContext),
    DisarmEscape,
    Stop {
        session_id: String,
        in_flight: InFlight,
    },
}

struct CaptureCompletion {
    snapshot: Option<FlowSnapshot>,
    action: Option<FlowAction>,
    stale: Option<InFlight>,
}

/// Desired Escape-stop registration, plus the revision that asked for it.
///
/// The registration itself CANNOT happen on the thread that decided it — see
/// `Flow::prepare_action`. Decoupling the decision from the effect reintroduces
/// the ordering hazard that putting arm/disarm inside the serialized section
/// was meant to solve (a stale Start re-arming after a newer Stop disarmed), so
/// each decision stamps a monotonic revision and the applier drops anything
/// that is no longer the latest intent.
#[derive(Default)]
struct EscapeIntent {
    revision: u64,
    armed: bool,
}

impl EscapeIntent {
    /// Stamp the newest intent, returning the revision the applier must quote.
    fn record(&mut self, armed: bool) -> u64 {
        self.revision = self.revision.saturating_add(1);
        self.armed = armed;
        self.revision
    }

    /// `Some(armed)` when `revision` is still the newest intent, `None` when a
    /// later transition has superseded it and this applier must do nothing.
    fn claim(&self, revision: u64) -> Option<bool> {
        (self.revision == revision).then_some(self.armed)
    }
}

impl Flow {
    pub fn new(
        history: History,
        settings: AppSettings,
        audio_dir: PathBuf,
        audio: AudioController,
        usage: UsageTracker,
    ) -> Self {
        Self {
            transitions: Arc::new(Mutex::new(())),
            state: Arc::new(Mutex::new(FlowState::default())),
            escape: Arc::new(Mutex::new(EscapeIntent::default())),
            history,
            settings: Arc::new(Mutex::new(settings)),
            audio_dir,
            audio,
            usage,
        }
    }

    pub fn settings(&self) -> AppSettings {
        self.settings.lock().clone()
    }

    pub fn set_settings(&self, s: AppSettings) -> AppSettings {
        std::mem::replace(&mut *self.settings.lock(), s)
    }

    pub fn get_flow_snapshot(&self) -> FlowSnapshot {
        self.state.lock().snapshot.clone()
    }

    pub fn handle_hotkey(&self, app: &AppHandle, evt: HotkeyEvent) {
        let _transition = self.transitions.lock();
        let at_ms = monotonic_ms();
        let mut action = None;
        let snapshot = {
            let mut state = self.state.lock();
            let availability = state.availability();
            let decision = match evt.edge {
                Edge::Down => state
                    .input
                    .physical_down(&evt.trigger_id, at_ms, availability),
                Edge::Up => state.input.physical_up(&evt.trigger_id, at_ms),
            };
            let start = (decision == Decision::Start).then(|| {
                self.new_session(evt.mode, evt.force_clean, false)
            });
            Self::apply_decision_locked(
                &mut state,
                decision,
                start,
                &mut action,
            )
        };
        self.prepare_action(app, action.as_ref());
        self.publish_if_some(app, snapshot);
        self.dispatch_action(app, action);
    }

    /// Explicit toggle for the floater and Touch Bar. These controls do not
    /// fabricate physical edges, so they cannot pollute the key-repeat latch.
    pub fn toggle_recording(&self, app: &AppHandle, mode: Mode, force_clean: bool) {
        let _transition = self.transitions.lock();
        let mut action = None;
        let snapshot = {
            let mut state = self.state.lock();
            let availability = state.availability();
            let decision = state.input.direct_toggle(availability);
            let start = (decision == Decision::Start).then(|| {
                self.new_session(mode, force_clean, true)
            });
            Self::apply_decision_locked(
                &mut state,
                decision,
                start,
                &mut action,
            )
        };
        self.prepare_action(app, action.as_ref());
        self.publish_if_some(app, snapshot);
        self.dispatch_action(app, action);
    }

    pub fn stop_recording(&self, app: &AppHandle) {
        let _transition = self.transitions.lock();
        let mut action = None;
        let snapshot = {
            let mut state = self.state.lock();
            let availability = state.availability();
            let decision = state.input.escape(availability);
            Self::apply_decision_locked(
                &mut state,
                decision,
                None,
                &mut action,
            )
        };
        self.prepare_action(app, action.as_ref());
        self.publish_if_some(app, snapshot);
        self.dispatch_action(app, action);
    }

    fn new_session(&self, mode: Mode, force_clean: bool, direct: bool) -> SessionContext {
        SessionContext {
            id: uuid::Uuid::new_v4().to_string(),
            mode,
            force_clean,
            capture_generation: self.audio.reserve_generation(),
            input: if direct {
                InputDisposition::Latched
            } else {
                InputDisposition::Undecided
            },
            mic: MicPhase::Waking,
            mic_ready_ms: None,
        }
    }

    fn apply_decision_locked(
        state: &mut FlowState,
        decision: Decision,
        start: Option<SessionContext>,
        action: &mut Option<FlowAction>,
    ) -> Option<FlowSnapshot> {
        match decision {
            Decision::Start => {
                let session = start.expect("Start decision requires a reserved session");
                state.snapshot.session_id = Some(session.id.clone());
                state.snapshot.mode = Some(mode_to_str(session.mode).to_owned());
                state.snapshot.input = Some(session.input);
                state.snapshot.mic = session.mic;
                state.snapshot.mic_ready_ms = None;
                state.runtime = RuntimeState::Starting {
                    session: session.clone(),
                    stop_requested: false,
                };
                *action = Some(FlowAction::Start(session));
                Some(state.revise(FlowPhase::Starting, None, None))
            }
            Decision::Stop => Self::request_stop_locked(state, action),
            Decision::Latch => {
                let updated = match &mut state.runtime {
                    RuntimeState::Starting { session, .. }
                    | RuntimeState::Recording { session, .. } => {
                        session.input = InputDisposition::Latched;
                        true
                    }
                    _ => false,
                };
                if updated {
                    state.snapshot.input = Some(InputDisposition::Latched);
                    let phase = state.snapshot.phase;
                    let stage = state.snapshot.stage;
                    Some(state.revise(phase, stage, None))
                } else {
                    None
                }
            }
            Decision::Busy => {
                let phase = state.snapshot.phase;
                let stage = state.snapshot.stage;
                Some(state.revise(
                    phase,
                    stage,
                    Some(FlowNotice {
                        code: "session_busy".to_owned(),
                        severity: NoticeSeverity::Info,
                        summary: "Still finishing the previous dictation.".to_owned(),
                        detail_ref: None,
                    }),
                ))
            }
            Decision::Ignore => None,
        }
    }

    fn request_stop_locked(
        state: &mut FlowState,
        action: &mut Option<FlowAction>,
    ) -> Option<FlowSnapshot> {
        let runtime = std::mem::replace(&mut state.runtime, RuntimeState::Idle);
        match runtime {
            RuntimeState::Starting {
                mut session,
                stop_requested: _,
            } => {
                if session.input == InputDisposition::Undecided {
                    session.input = InputDisposition::HoldToTalk;
                    state.snapshot.input = Some(InputDisposition::HoldToTalk);
                }
                state.runtime = RuntimeState::Starting {
                    session,
                    stop_requested: true,
                };
                *action = Some(FlowAction::DisarmEscape);
                Some(state.revise(FlowPhase::Stopping, None, None))
            }
            RuntimeState::Recording {
                mut session,
                in_flight,
            } => {
                if session.input == InputDisposition::Undecided {
                    session.input = InputDisposition::HoldToTalk;
                    state.snapshot.input = Some(InputDisposition::HoldToTalk);
                }
                let session_id = session.id.clone();
                let record_id = in_flight.record_id.clone();
                state.runtime = RuntimeState::Processing { session, record_id };
                state.input.session_ended();
                *action = Some(FlowAction::Stop {
                    session_id,
                    in_flight,
                });
                Some(state.revise(FlowPhase::Stopping, None, None))
            }
            other => {
                state.runtime = other;
                None
            }
        }
    }

    fn publish_if_some(&self, app: &AppHandle, snapshot: Option<FlowSnapshot>) {
        if let Some(snapshot) = snapshot {
            publish_snapshot(app, &snapshot);
        }
    }

    /// Record the Escape-stop intent for this transition and apply it OFF the
    /// deciding thread.
    ///
    /// **Never touch the global-shortcut registry from here synchronously.**
    /// `handle_hotkey` and `stop_recording` both run *inside* a global-shortcut
    /// callback, and tauri-plugin-global-shortcut invokes handlers while
    /// holding its `shortcuts: Mutex<HashMap<..>>`:
    ///
    /// ```ignore
    /// if let Some(shortcut) = shortcuts_.lock().unwrap().get(&e.id) {
    ///     handler(&app_handle, &shortcut.shortcut, e);   // lock still held
    /// ```
    ///
    /// `arm_escape_stop`/`disarm_escape_stop` call `is_registered`,
    /// `on_shortcut` and `unregister`, each of which re-locks that same
    /// non-reentrant `std::sync::Mutex` — and `register`/`unregister`
    /// additionally block on a main-thread round-trip. Calling them here wedged
    /// the hotkey event thread on the very first key-down, with the registry
    /// mutex still held, which killed every shortcut for the rest of the
    /// process (v3.3.0-nightly.2).
    fn prepare_action(&self, app: &AppHandle, action: Option<&FlowAction>) {
        let armed = match action {
            Some(FlowAction::Start(_)) => true,
            Some(FlowAction::DisarmEscape | FlowAction::Stop { .. }) => false,
            None => return,
        };

        let revision = self.escape.lock().record(armed);

        let this = self.clone();
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            // Holding the intent lock across the apply serializes concurrent
            // appliers, so an older one can never land after a newer one.
            let intent = this.escape.lock();
            match intent.claim(revision) {
                Some(true) => arm_escape_stop(&app, &this),
                Some(false) => disarm_escape_stop(&app),
                None => {}
            }
        });
    }

    fn dispatch_action(&self, app: &AppHandle, action: Option<FlowAction>) {
        match action {
            Some(FlowAction::Start(session)) => {
                let this = self.clone();
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let session_id = session.id.clone();
                    let result = this.prepare_recording_async(&app, &session).await;
                    this.capture_started(&app, &session_id, result);
                });
            }
            Some(FlowAction::Stop {
                session_id,
                in_flight,
            }) => {
                let this = self.clone();
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let record_id = in_flight.record_id.clone();
                    let outcome = this.do_pipeline(&app, &session_id, in_flight).await;
                    this.pipeline_finished(&app, &session_id, &record_id, outcome);
                });
            }
            Some(FlowAction::DisarmEscape) => {}
            None => {}
        }
    }

    async fn prepare_recording_async(
        &self,
        app: &AppHandle,
        session: &SessionContext,
    ) -> Result<InFlight> {
        let date = Utc::now().format("%Y-%m-%d").to_string();
        let id_seed = uuid::Uuid::new_v4().to_string();
        let path = self.audio_dir.join(date).join(format!("{id_seed}.wav"));

        let captured_focus = inject::focus::capture();
        let friendly = captured_focus
            .as_ref()
            .and_then(inject::focus::friendly_app_name)
            .unwrap_or_default();
        let _ = app.emit("wispr:active_app", friendly);

        let input_device = self
            .settings
            .lock()
            .input_device
            .clone()
            .filter(|s| !s.trim().is_empty());

        // The history row is the transaction anchor. If audio startup fails,
        // compensate it immediately and remove any partial WAV.
        let device_name = self.settings.lock().device_name.clone();
        let record_id = self
            .history
            .insert_new(&path, ClippyMode::from(session.mode), &device_name)
            .context("creating recording history row")?;

        if let Err(e) = self
            .audio
            .start(path.clone(), input_device, session.capture_generation)
            .await
            .context("starting audio capture")
        {
            let raw = format!("{e:#}");
            let _ = self.history.set_error(&record_id, &raw);
            let _ = std::fs::remove_file(&path);
            return Err(e);
        }
        crate::audio::cues::play_start();

        Ok(InFlight {
            mode: session.mode,
            record_id,
            audio_path: path,
            captured_focus,
            force_clean: session.force_clean,
        })
    }

    fn capture_started(&self, app: &AppHandle, session_id: &str, result: Result<InFlight>) {
        let _transition = self.transitions.lock();
        let completion = {
            let mut state = self.state.lock();
            Self::apply_capture_completion_locked(&mut state, session_id, result)
        };

        if let Some(orphan) = completion.stale {
            let _ = self
                .history
                .set_error(&orphan.record_id, "stale capture startup completion");
            let audio = self.audio.clone();
            tauri::async_runtime::spawn(async move {
                let _ = audio.stop().await;
            });
        }
        self.prepare_action(app, completion.action.as_ref());
        self.publish_if_some(app, completion.snapshot);
        self.dispatch_action(app, completion.action);
    }

    fn apply_capture_completion_locked(
        state: &mut FlowState,
        session_id: &str,
        result: Result<InFlight>,
    ) -> CaptureCompletion {
        let runtime = std::mem::replace(&mut state.runtime, RuntimeState::Idle);
        match runtime {
            RuntimeState::Starting {
                session,
                stop_requested,
            } if session.id == session_id => match result {
                Ok(in_flight) if stop_requested => {
                    let record_id = in_flight.record_id.clone();
                    state.runtime = RuntimeState::Processing { session, record_id };
                    state.input.session_ended();
                    CaptureCompletion {
                        snapshot: Some(state.revise(FlowPhase::Stopping, None, None)),
                        action: Some(FlowAction::Stop {
                            session_id: session_id.to_owned(),
                            in_flight,
                        }),
                        stale: None,
                    }
                }
                Ok(in_flight) => {
                    state.runtime = RuntimeState::Recording { session, in_flight };
                    CaptureCompletion {
                        snapshot: Some(state.revise(FlowPhase::Recording, None, None)),
                        action: None,
                        stale: None,
                    }
                }
                Err(e) => {
                    let raw = format!("{e:#}");
                    let friendly = user_friendly_error(&raw);
                    tracing::warn!(session_id, "capture startup failed: {raw}");
                    state.input.session_ended();
                    state.snapshot.mic = MicPhase::Unavailable;
                    CaptureCompletion {
                        snapshot: Some(state.revise(
                            FlowPhase::Failed,
                            None,
                            Some(FlowNotice {
                                code: "capture_start_failed".to_owned(),
                                severity: NoticeSeverity::Error,
                                summary: friendly,
                                detail_ref: Some(session_id.to_owned()),
                            }),
                        )),
                        action: Some(FlowAction::DisarmEscape),
                        stale: None,
                    }
                }
            },
            other => {
                state.runtime = other;
                CaptureCompletion {
                    snapshot: None,
                    action: None,
                    stale: result.ok(),
                }
            }
        }
    }

    fn pipeline_finished(
        &self,
        app: &AppHandle,
        session_id: &str,
        record_id: &str,
        outcome: Result<()>,
    ) {
        let _transition = self.transitions.lock();
        let snapshot = {
            let mut state = self.state.lock();
            let matches = matches!(
                &state.runtime,
                RuntimeState::Processing {
                    session,
                    record_id: active_record,
                } if session.id == session_id && active_record == record_id
            );
            if !matches {
                return;
            }

            state.runtime = RuntimeState::Idle;
            state.input.session_ended();
            state.snapshot.mic = MicPhase::Inactive;
            match outcome {
                Ok(()) => Some(state.revise(FlowPhase::Succeeded, None, None)),
                Err(e) => {
                    let raw = format!("{e:#}");
                    tracing::warn!(record_id, session_id, "pipeline failed: {raw}");
                    let _ = self.history.set_error(record_id, &raw);
                    let friendly = user_friendly_error(&raw);
                    Some(state.revise(
                        FlowPhase::Failed,
                        None,
                        Some(FlowNotice {
                            code: "pipeline_failed".to_owned(),
                            severity: NoticeSeverity::Error,
                            summary: friendly,
                            detail_ref: Some(record_id.to_owned()),
                        }),
                    ))
                }
            }
        };
        self.publish_if_some(app, snapshot);
    }

    pub fn handle_mic_ready(&self, app: &AppHandle, generation: u64, ready_ms: i64) {
        let _transition = self.transitions.lock();
        let snapshot = {
            let mut state = self.state.lock();
            Self::apply_mic_ready_locked(&mut state, generation, ready_ms)
        };
        self.publish_if_some(app, snapshot);
    }

    fn apply_mic_ready_locked(
        state: &mut FlowState,
        generation: u64,
        ready_ms: i64,
    ) -> Option<FlowSnapshot> {
        let updated = match &mut state.runtime {
            RuntimeState::Starting { session, .. }
            | RuntimeState::Recording { session, .. }
                if session.capture_generation == generation =>
            {
                session.mic = MicPhase::Live;
                session.mic_ready_ms = Some(ready_ms);
                true
            }
            _ => false,
        };
        if updated {
            state.snapshot.mic = MicPhase::Live;
            state.snapshot.mic_ready_ms = Some(ready_ms);
            let phase = state.snapshot.phase;
            let stage = state.snapshot.stage;
            Some(state.revise(phase, stage, None))
        } else {
            None
        }
    }

    fn report_pipeline_stage(
        &self,
        app: &AppHandle,
        session_id: &str,
        stage: FlowStage,
    ) {
        let _transition = self.transitions.lock();
        let snapshot = {
            let mut state = self.state.lock();
            let matches = matches!(
                &state.runtime,
                RuntimeState::Processing { session, .. } if session.id == session_id
            );
            if matches {
                Some(state.revise(FlowPhase::Processing, Some(stage), None))
            } else {
                None
            }
        };
        self.publish_if_some(app, snapshot);
    }

    /// Start the Settings mic test on `device` (None = system default).
    /// Returns the resolved device name. Metering only — no file, no history
    /// row; see `commands::start_mic_test` for why this exists.
    pub async fn start_mic_test(&self, device: Option<String>) -> Result<String> {
        self.audio
            .start_preview(device.filter(|s| !s.trim().is_empty()))
            .await
    }

    pub async fn stop_mic_test(&self) -> Result<()> {
        self.audio.stop_preview().await
    }

    /// Write the flight-recorder timings/event-log for a recording. Best-effort:
    /// a failure here is a log line, never a pipeline error (diagnostics must
    /// not break the thing they're diagnosing).
    fn persist_timeline(&self, record_id: &str, tl: &Timeline) {
        if let Err(e) = self.history.set_timings(
            record_id,
            tl.stt_ms,
            tl.cleanup_ms,
            Some(tl.total_ms()),
            &tl.json(),
        ) {
            tracing::warn!("set_timings failed (non-fatal): {e:#}");
        }
    }

    async fn do_pipeline(
        &self,
        app: &AppHandle,
        session_id: &str,
        in_flight: InFlight,
    ) -> Result<()> {
        let InFlight {
            mode,
            record_id,
            audio_path: _,
            captured_focus,
            force_clean,
        } = in_flight;

        // Flight recorder for this run — start the clock now (recording just
        // stopped), so `total_ms` measures true end-to-end turnaround.
        let mut tl = Timeline::new();

        self.report_pipeline_stage(app, session_id, FlowStage::Transcribing);

        let FinishedRecording {
            path,
            duration_ms,
            captured_ms,
            stream_errored,
            mic_ready_ms,
            device_name: capture_device,
            device_fallback,
        } = self.audio.stop().await?;
        crate::audio::cues::play_stop();
        self.history
            .set_duration(&record_id, duration_ms, captured_ms)?;

        // Capture-integrity check. `duration_ms` is the wall-clock timer;
        // `captured_ms` is how much audio actually reached the WAV. If the mic
        // dropped mid-recording they diverge — and the transcript WILL be
        // truncated, because the audio simply isn't there. We flag this loudly
        // (timeline + a user-facing warning) instead of silently pasting a
        // fragment and calling it a success. Retrying can't recover lost audio.
        let capture_gap = stream_errored || captured_ms + 1000 < duration_ms;
        // Which mic this actually ran on. Recorded unconditionally so "why did
        // it use the laptop mic?" is answerable from the (i) inspector after
        // the fact, instead of being a guess.
        tl.mark(format!(
            "input device · {capture_device}{}",
            if device_fallback {
                " (saved device not found — fell back to system default)"
            } else {
                ""
            }
        ));
        if device_fallback {
            let _ = app.emit(
                "wispr:clippy_warning",
                format!(
                    "Your chosen mic wasn't available — recorded with {capture_device} instead. Turn the mic on (or re-pair it) before the next dictation."
                ),
            );
        }
        tl.mark(format!(
            "audio · {:.1}s recorded / {:.1}s captured{}",
            (duration_ms.max(0) as f64) / 1000.0,
            (captured_ms.max(0) as f64) / 1000.0,
            if stream_errored {
                " · mic stream ERROR"
            } else {
                ""
            }
        ));
        if capture_gap {
            let lost = ((duration_ms - captured_ms).max(0) as f64) / 1000.0;
            tl.mark(format!(
                "⚠ capture gap · ~{lost:.0}s of audio missing (mic dropped mid-recording)"
            ));
            tracing::warn!(
                record_id,
                duration_ms,
                captured_ms,
                stream_errored,
                "capture gap — transcript will be truncated"
            );
        }

        // Mic wake-up (head-gap) telemetry. `mic_ready_ms` is key-down →
        // first audio callback: anything the user said in that window never
        // reached the WAV, and the wall-clock timer can't see it either
        // (it also starts late). Emitted on EVERY recording so the
        // onboarding demo doubles as a mic health-check; the floater warning
        // fires once per run for pathological wake-ups (typically Windows
        // "audio enhancements" / exclusive-mode arbitration on the mic).
        tl.mark(format!(
            "mic wake-up · {:.2}s",
            (mic_ready_ms.max(0) as f64) / 1000.0
        ));
        #[derive(Clone, serde::Serialize)]
        struct MicDiag {
            mic_ready_ms: i64,
            duration_ms: i64,
            captured_ms: i64,
            stream_errored: bool,
        }
        let _ = app.emit(
            "wispr:mic_diag",
            MicDiag { mic_ready_ms, duration_ms, captured_ms, stream_errored },
        );
        if mic_ready_ms > SLOW_MIC_MS
            && !SLOW_MIC_WARNED.swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            // Two different mechanisms, two different fixes, and the wrong
            // advice wastes the user's time. A wired/built-in mic that is slow
            // is almost always Windows audio enhancements or exclusive-mode
            // arbitration. A Bluetooth mic that is slow is the headset profile
            // link coming up — and on a DJI transmitter specifically, the
            // dominant term is it negotiating ITSELF out of noise-cancellation
            // mode, which takes ~7-8s and is fixed by leaving NC off.
            let _ = app.emit(
                "wispr:clippy_warning",
                slow_mic_message(mic_ready_ms, &capture_device),
            );
        }

        // Trim trailing silence before sending to Whisper — prevents
        // hallucinations like "thank you" / "gracias" on silent tails.
        if let Err(e) = crate::audio::trim_trailing_silence(&path, 500, 300) {
            tracing::warn!("silence trimming failed (non-fatal): {e:#}");
        }

        if duration_ms < MIN_DURATION_MS {
            tracing::info!(record_id, duration_ms, "discarding too-short recording");
            self.history.set_error(&record_id, "recording too short")?;
            let _ = std::fs::remove_file(&path);
            return Err(anyhow!("recording too short"));
        }

        self.history
            .update_status(&record_id, Status::Transcribing)?;

        // Provider-specific key lookup happens in build_stt_provider().
        let stt_settings = self.settings();
        let stt = build_stt_provider(&stt_settings)?;
        let stt_name = stt.name();
        // Tell the floater which service is doing the transcription so its
        // bubble can read "transcribing · Groq" — and so a stall is clearly
        // attributable rather than a mystery spinner.
        let _ = app.emit("wispr:stt_provider", stt_name);

        // Optional mic noise reduction (laptop fan hum/whir). Processes the
        // recording into a SIDE file that only the STT request sees — the raw
        // WAV in history stays untouched, so playback and re-diagnosis always
        // have the original. Runs on a blocking thread (pure CPU, ~130-600x
        // realtime). Fail-open: any error falls back to the raw audio, because
        // denoising must never cost the user a dictation. The guard deletes
        // the side file on every exit path (success, STT error, timeout).
        let nr = crate::audio::denoise::NoiseReduction::parse(&stt_settings.noise_reduction);
        let (stt_input, _denoise_guard) = if nr != crate::audio::denoise::NoiseReduction::Off {
            self.report_pipeline_stage(app, session_id, FlowStage::Denoising);
            let src = path.clone();
            let result =
                tokio::task::spawn_blocking(move || {
                    crate::audio::denoise::denoise_to_side_file(&src, nr)
                })
                .await
                .map_err(anyhow::Error::new)
                .and_then(|r| r);
            self.report_pipeline_stage(app, session_id, FlowStage::Transcribing);
            match result {
                Ok(out) => {
                    tl.mark(format!(
                        "noise reduction ({}) · {}ms",
                        stt_settings.noise_reduction, out.elapsed_ms
                    ));
                    let guard = TempFileGuard(out.path.clone());
                    (out.path, Some(guard))
                }
                Err(e) => {
                    tl.mark("noise reduction FAILED · using raw audio".to_string());
                    tracing::warn!("denoise failed (non-fatal, using raw audio): {e:#}");
                    (path.clone(), None)
                }
            }
        } else {
            (path.clone(), None)
        };

        // Level check + quiet-audio rescue. This is cheap (one pass over a
        // short WAV) and it addresses a failure mode that is otherwise
        // invisible: audio around -46 dBFS transcribes to a plausible-looking
        // transcript with whole phrases silently deleted — no error, nothing in
        // the UI, the user can't tell. Measured on a real external-mic clip.
        // Boosting a copy to -3 dBFS peak recovered the dropped speech with
        // zero clipped samples.
        //
        // Fail-open throughout: a measurement problem falls back to the audio
        // we already had. The recording on disk is never modified.
        let (stt_input, _gain_guard, level_stats) = if stt_settings.auto_gain {
            let src = stt_input.clone();
            match tokio::task::spawn_blocking(move || crate::audio::level::analyze_and_rescue(&src))
                .await
                .map_err(anyhow::Error::new)
                .and_then(|r| r)
            {
                Ok(outcome) => {
                    tl.mark(format!(
                        "level · {:.1} dBFS RMS / {:.1} dBFS peak",
                        outcome.stats.rms_dbfs, outcome.stats.peak_dbfs
                    ));
                    match outcome.normalized {
                        Some(boosted) => {
                            tl.mark(format!(
                                "quiet audio · boosted {:.1} dB before transcription",
                                outcome.gain_db
                            ));
                            let guard = TempFileGuard(boosted.clone());
                            (boosted, Some(guard), Some((outcome.stats, outcome.gain_db)))
                        }
                        None => (stt_input, None, Some((outcome.stats, outcome.gain_db))),
                    }
                }
                Err(e) => {
                    tracing::warn!("level analysis failed (non-fatal): {e:#}");
                    (stt_input, None, None)
                }
            }
        } else {
            // Auto-gain off: still measure, so the flight recorder can explain
            // a bad transcript even when the user opted out of the fix.
            let src = stt_input.clone();
            let measured = tokio::task::spawn_blocking(move || {
                crate::audio::wavio::read_mono_f32(&src)
                    .map(|d| crate::audio::level::measure_samples(&d.samples))
            })
            .await
            .ok()
            .and_then(|r| r.ok());
            if let Some(stats) = measured {
                tl.mark(format!(
                    "level · {:.1} dBFS RMS / {:.1} dBFS peak (auto-gain off)",
                    stats.rms_dbfs, stats.peak_dbfs
                ));
            }
            (stt_input, None, measured.map(|s| (s, 0.0f32)))
        };

        // Warn only when the audio the provider ACTUALLY RECEIVES is still too
        // quiet — i.e. after the rescue boost, not before it.
        //
        // Warning on the raw measurement was a real defect. A mic that idles
        // near -38 dBFS RMS sits right on the -40 warn line, so about every
        // other dictation raised a red terminal-error bubble even though the
        // audio had already been boosted to a healthy -3 dBFS peak and
        // transcribed perfectly. A warning that fires on a problem the app just
        // fixed is noise, and it buries the case that genuinely matters: a
        // boost clamped by MAX_GAIN_DB that left the recording quiet anyway.
        if let Some((stats, gain_db)) = level_stats {
            let delivered = stats.with_gain(gain_db);
            if delivered.is_quiet() {
                tl.mark(format!(
                    "quiet WARNING · {:.1} dBFS RMS reaching speech-to-text",
                    delivered.rms_dbfs
                ));
                let _ = app.emit(
                    "wispr:clippy_warning",
                    crate::audio::level::quiet_warning(&stats, gain_db),
                );
            }
        }

        let wav_size = tokio::fs::metadata(&stt_input)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        tracing::info!(
            record_id,
            wav_bytes = wav_size,
            provider = stt_name,
            "sending WAV to speech-to-text provider"
        );

        tl.mark(format!(
            "STT request → {} ({})",
            pretty_provider(stt_name),
            stt_settings.stt_model
        ));

        // 120s hard cap on STT. Single-request Whisper rarely takes more
        // than ~15s for 5-min audio; the wider ceiling accommodates the
        // multi-chunk path (files > 20 MB get split and transcribed
        // sequentially, ~3-6s per chunk). Beyond 120s something is wrong
        // (DNS hang, upstream stall) and we'd rather surface an error than
        // let Clippy spin forever. reqwest itself doesn't apply a default
        // request timeout, so without this the call could hang indefinitely.
        //
        // The STT wall-time is the single most useful debugging number, so we
        // measure it explicitly and record it on the timeline for EVERY outcome
        // (ok / provider error / 120s timeout) before propagating.
        let stt_t0 = std::time::Instant::now();
        // Live dictation never diarizes — it's one person talking into their
        // own mic, and the surcharge/latency would buy nothing. Diarization is
        // an upload-path option (see `run_upload_pipeline`).
        let stt_opts = crate::stt::SttOptions {
            language: stt_settings.language_hint.clone(),
            diarize: false,
        };
        let stt_future = stt.transcribe(&stt_input, &stt_opts);
        let stt_result = tokio::time::timeout(std::time::Duration::from_secs(120), stt_future).await;
        let stt_elapsed = stt_t0.elapsed().as_millis() as i64;
        tl.stt_ms = Some(stt_elapsed);

        let transcript = match stt_result {
            Err(_) => {
                tl.mark(format!("STT TIMED OUT after {stt_elapsed}ms (120s cap)"));
                self.persist_timeline(&record_id, &tl);
                return Err(anyhow!(
                    "Whisper STT timed out after 120s — check network or try a shorter clip"
                ));
            }
            Ok(Err(e)) => {
                tl.mark(format!(
                    "STT FAILED after {stt_elapsed}ms · {}",
                    e.to_string().lines().next().unwrap_or("error")
                ));
                self.persist_timeline(&record_id, &tl);
                return Err(anyhow::Error::new(e)).with_context(|| {
                    format!(
                        "{provider} transcription request",
                        provider = pretty_provider(stt_name)
                    )
                });
            }
            Ok(Ok(t)) => {
                // Provider-reported processed duration is the third leg of the
                // triangulation: if the WAV held 180s but the provider only
                // processed 32s, the truncation happened in upload/transport
                // rather than capture. Deepgram returns this; others may not.
                let processed = t
                    .duration_seconds
                    .map(|d| format!(" · provider processed {d:.1}s"))
                    .unwrap_or_default();
                tl.mark(format!(
                    "STT ok · {stt_elapsed}ms · {} chars{processed}",
                    t.text.chars().count()
                ));
                t
            }
        };

        tracing::info!(
            record_id,
            chars = transcript.text.chars().count(),
            language = ?transcript.language,
            duration_secs = ?transcript.duration_seconds,
            stt_ms = stt_elapsed,
            provider = stt_name,
            "speech-to-text response"
        );
        self.usage.record_stt(
            stt_name,
            &stt_settings.stt_model,
            transcript
                .duration_seconds
                .unwrap_or_else(|| (duration_ms.max(0) as f64) / 1000.0),
        );

        self.history
            .set_transcript(&record_id, &transcript.text, stt_name)?;

        let clippy_settings = self.settings();
        let needs_clippy = match mode {
            // Light: cleanup either when the persistent toggle is on OR
            // when the user hit Shift+F8 (force_clean) for this single press.
            Mode::Light => clippy_settings.auto_clean_in_light || force_clean,
            Mode::Advanced => clippy_settings.auto_clean_in_advanced,
            Mode::Drafting => clippy_settings.auto_clean_in_drafting,
        };
        if force_clean {
            tracing::info!(
                record_id,
                "force-clean override active (Shift+F8 invocation)"
            );
        }

        let final_text = if needs_clippy {
            self.report_pipeline_stage(app, session_id, FlowStage::Cleaning);
            self.history.update_status(&record_id, Status::Cleaning)?;

            // Simplified: ONE LLM provider + model for all three modes.
            // The mode only changes which system prompt is sent (Light /
            // Advanced / Drafting) — not which model is called.
            let provider_id = clippy_settings.llm_provider.clone();
            let model = clippy_settings.llm_model.clone();
            let _ = mode; // mode is used downstream in clippy::clean for prompt selection

            let llm: Box<dyn LlmProvider> = build_llm_provider(&provider_id, model.clone())?;
            // Surface which LLM is doing cleanup so the floater reads
            // "polishing · Gemini" / "polishing · Groq". This is the single
            // most-requested bit of visibility: when cleanup is slow or fails,
            // the user wants to know whether it's the Groq or the Gemini call.
            let _ = app.emit("wispr:llm_provider", llm.name());
            let custom = custom_prompt_for(&clippy_settings, mode);
            // App-context hint: ONLY for Drafting (the mode that's allowed to
            // reshape register/structure). When the user opts out via the
            // adapt_to_app setting we skip the lookup entirely.
            let app_hint = if matches!(mode, Mode::Drafting) && clippy_settings.adapt_to_app {
                let kind = captured_focus
                    .as_ref()
                    .map(inject::focus::classify_app)
                    .unwrap_or(inject::focus::AppKind::Default);
                if kind != inject::focus::AppKind::Default {
                    tracing::info!(?kind, "app-context hint active for drafting");
                }
                inject::focus::context_hint(kind)
            } else {
                None
            };
            tl.mark(format!("cleanup request → {} ({})", llm.name(), model));
            let clean_t0 = std::time::Instant::now();
            let cleaned = clippy::clean(
                &transcript.text,
                ClippyMode::from(mode),
                custom.as_deref(),
                app_hint,
                llm.as_ref(),
            )
            .await;
            let clean_elapsed = clean_t0.elapsed().as_millis() as i64;
            tl.cleanup_ms = Some(clean_elapsed);
            if cleaned.used_clippy {
                tl.mark(format!("cleanup ok · {clean_elapsed}ms"));
            } else {
                tl.mark(format!(
                    "cleanup skipped · {clean_elapsed}ms · {}",
                    cleaned.note.as_deref().unwrap_or("no LLM output")
                ));
            }
            self.usage
                .record_llm(llm.name(), &model, cleaned.usage.as_ref());
            // Cleanup couldn't run (timeout, auth, rate limit, upstream) but
            // we still have the raw transcript to paste. Previously this was
            // SILENT — the user just saw a slow result and never knew the LLM
            // step failed. Now we tell them, attributed to the provider, as a
            // non-fatal floater notice (the pipeline continues + pastes raw).
            // `light_length_drift` is excluded: that's an intentional safety
            // fallback, not a failure.
            if !cleaned.used_clippy {
                if let Some(note) = cleaned.note {
                    if note != "light_length_drift" {
                        let _ = app.emit(
                            "wispr:clippy_warning",
                            cleanup_failure_message(note, llm.name()),
                        );
                    }
                }
            }
            // Drafting mode (F9) writes to `drafted_text`; everything else
            // (Light cleanup, Advanced cleanup) writes to `cleaned_text`.
            // This lets the history UI show both versions independently.
            let alt = if matches!(mode, Mode::Drafting) {
                AltKind::Drafted
            } else {
                AltKind::Cleaned
            };
            self.history.set_alt(
                &record_id,
                alt,
                &cleaned.text,
                Some(llm.name()),
                cleaned.used_clippy,
                cleaned.note,
            )?;
            cleaned.text
        } else {
            transcript.text.clone()
        };

        // Tally the lifetime daily-stats rollup once per recording, right after
        // the final text exists and before delivery (so it's counted whether we
        // paste or fall back to silent clipboard). Keyed by LOCAL calendar day
        // so "per day" lines up with the user's wall calendar, not UTC. Failure
        // here is non-fatal — stats are a nicety, never block the paste.
        {
            let words = final_text.split_whitespace().count() as i64;
            let date = chrono::Local::now().format("%Y-%m-%d").to_string();
            if let Err(e) =
                self.history
                    .record_session(&date, words, duration_ms, ClippyMode::from(mode))
            {
                tracing::warn!("daily-stats record_session failed (non-fatal): {e:#}");
            }
        }

        // Auto-title (parallel track): name the recording with a one-line LLM
        // descriptor so the history card headers read like a table of contents.
        // Fire-and-forget on a light Groq model — never blocks the paste,
        // failures are a log line, not a user-visible error.
        // Read the whole title config under one lock — the spawned task
        // outlives this scope and must not hold the settings mutex.
        let title_cfg = {
            let s = self.settings.lock();
            s.auto_title
                .then(|| (s.title_provider.clone(), s.title_model.clone()))
        };
        if let Some((title_provider, title_model)) = title_cfg {
            let history = self.history.clone();
            let app_for_title = app.clone();
            let rid = record_id.clone();
            let text_for_title = final_text.clone();
            tauri::async_runtime::spawn(async move {
                match generate_title(&text_for_title, &title_provider, &title_model).await {
                    Ok(Some(title)) => {
                        if let Err(e) = history.set_title(&rid, &title) {
                            tracing::warn!("auto-title: db write failed (non-fatal): {e:#}");
                        } else {
                            let _ = app_for_title.emit("wispr:history_changed", ());
                        }
                    }
                    Ok(None) => {} // too short to be worth naming
                    Err(e) => tracing::warn!("auto-title failed (non-fatal): {e:#}"),
                }
            });
        }

        self.report_pipeline_stage(app, session_id, FlowStage::Injecting);
        self.history.update_status(&record_id, Status::Injecting)?;

        // Decision tree:
        //   (a) Foreground HWND + focused control unchanged → SKIP restore.
        //       This is the "user stayed put" case (Teams compose, normal
        //       same-window dictation). Touching focus here actively hurts
        //       in Electron apps: SetForegroundWindow on an already-foreground
        //       window fires WM_ACTIVATE, which Electron uses to re-seat
        //       focus on its preferred element — usually NOT the user's
        //       compose box.
        //   (b) Same process but focus drifted (focused_ctrl changed, OR
        //       foreground HWND changed within same pid) → restore.
        //       This is the F10/Outlook ribbon-keytip case.
        //   (c) Different process AND pull_back_on_navigation == true →
        //       user opted in to focus-stealing; restore + inject.
        //   (d) Different process AND pull_back_on_navigation == false →
        //       respect the new app. Silent clipboard delivery + Clippy
        //       bubble. No focus theft.
        //   (e) Foreground unreadable (pid 0 after retries) → NOT (d). We have
        //       no reading, not a reading of "somewhere else"; restore the
        //       captured window and paste normally.
        let inj_settings = self.settings();
        let (current_fg, current_ctrl, current_pid) =
            inject::focus::current_foreground_state_settled();
        let cap_ref = captured_focus.as_ref();
        // A pid of 0 means the OS wouldn't tell us who is in front, even after
        // retries — not that the user moved. Case (d) below MUST NOT fire on
        // an unknown reading: doing so silently downgrades a normal paste to
        // "Copied to clipboard" while the caret is still sitting in the box
        // the user dictated into.
        let foreground_unknown = current_pid == 0;
        let same_fg = cap_ref
            .map(|c| c.foreground_hwnd() == current_fg && current_fg != 0)
            .unwrap_or(false);
        let same_ctrl = cap_ref
            .map(|c| c.focused_ctrl() == current_ctrl)
            .unwrap_or(false);
        let same_process = cap_ref
            .map(|c| c.pid() == current_pid && current_pid != 0)
            .unwrap_or(false);
        let nothing_changed = same_fg && same_ctrl;
        // On an unknown reading, restore. If `GetForegroundWindow` really did
        // return NULL then NO window owns focus at this instant, and SendInput
        // would type into the void — putting the captured window back is both
        // the safest guess and the user's stated intent.
        let should_restore_focus = !nothing_changed
            && (same_process || foreground_unknown || inj_settings.pull_back_on_navigation);

        if foreground_unknown {
            // Rare, and previously invisible — it presented to the user as a
            // random "Copied to clipboard" instead of a paste. Record it so
            // the per-recording timeline shows which path was taken.
            tl.mark("foreground unreadable · restoring captured window and pasting");
            tracing::warn!(
                captured_pid = cap_ref.map(|c| c.pid()).unwrap_or(0),
                "foreground window unreadable after retries; treating as 'user stayed put'"
            );
        }

        if let (Some(cap), true) = (cap_ref, should_restore_focus) {
            if let Err(e) = inject::focus::restore(cap) {
                tracing::warn!("focus restore failed (non-fatal): {e:#}");
            }
        } else if nothing_changed && cap_ref.is_some() {
            tracing::debug!("focus restore skipped: foreground + focused control unchanged");
        }

        if !same_process
            && !foreground_unknown
            && !inj_settings.pull_back_on_navigation
            && captured_focus.is_some()
        {
            // Cross-process silent delivery. Don't paste anywhere — that
            // would either pollute the wrong app (Chrome address bar, etc.)
            // or fight the user's current task. Just leave it on clipboard
            // and tell Clippy to show a "copied" bubble.
            tracing::info!(
                captured_pid = captured_focus.as_ref().map(|c| c.pid()).unwrap_or(0),
                current_pid,
                chars = final_text.chars().count(),
                "silent clipboard delivery (user navigated away)"
            );
            match inject::clipboard::set_only(&final_text) {
                Ok(()) => {
                    tl.mark("delivered · clipboard (navigated away)");
                    let _ = app.emit("wispr:clippy_message", "Copied to clipboard");
                    self.history.update_status(&record_id, Status::Done)?;
                    crate::sync::engine::notify_recording_done(app);
                }
                Err(e) => {
                    tl.mark(format!("clipboard delivery FAILED · {e}"));
                    tracing::warn!("silent clipboard set failed: {e:#}");
                    self.history
                        .set_error(&record_id, &format!("clipboard: {e}"))?;
                    self.persist_timeline(&record_id, &tl);
                    return Err(anyhow!("clipboard delivery failed: {e}"));
                }
            }
        } else {
            match inject::inject(&final_text, inj_settings.keep_in_clipboard) {
                Ok(channel) => {
                    tl.mark(format!("delivered · {channel:?}"));
                    tracing::info!(
                        ?channel,
                        chars = final_text.chars().count(),
                        keep_in_clipboard = inj_settings.keep_in_clipboard,
                        "injected"
                    );
                    self.history.update_status(&record_id, Status::Done)?;
                    crate::sync::engine::notify_recording_done(app);
                }
                Err(e) => {
                    tl.mark(format!("injection FAILED · {e}"));
                    tracing::warn!("injection failed: {e:#}");
                    self.history
                        .set_error(&record_id, &format!("injection: {e}"))?;
                    self.persist_timeline(&record_id, &tl);
                    return Err(anyhow!("injection failed: {e}"));
                }
            }
        }

        // If the mic dropped mid-recording, tell the user plainly. They got a
        // (partial) transcript, so this is a warning, not a hard error — but
        // they deserve to know the result is cut short and that a retry can't
        // fix it (the audio was never captured).
        if capture_gap {
            let recorded = (duration_ms.max(0) as f64) / 1000.0;
            let got = (captured_ms.max(0) as f64) / 1000.0;
            let _ = app.emit(
                "wispr:clippy_warning",
                format!(
                    "Mic dropped mid-recording — only ~{got:.0}s of ~{recorded:.0}s was captured, so the transcript is cut short. Re-record to get the rest (retry won't recover lost audio)."
                ),
            );
        }

        // Flight recorder: persist the full timeline + stage durations now that
        // delivery is done. This is the success path; error returns above have
        // already persisted their partial timeline.
        self.persist_timeline(&record_id, &tl);

        // wrapper emits idle
        Ok(())
    }

    /// Transcribe a user-supplied audio file (drag-and-drop or file picker)
    /// instead of a live mic recording. The file is copied into the same dated
    /// audio store the recorder uses — so playback, the (i) inspector, and the
    /// retention GC treat it identically — then run through STT and the optional
    /// cleanup/draft passes. It lands in History flagged `source = upload` (the
    /// "Uploaded" badge) and is NOT injected anywhere: an upload isn't aimed at
    /// a text field, it's just a new history entry.
    ///
    /// `stt_provider`/`stt_model`/`llm_provider`/`llm_model` are per-batch
    /// overrides from the upload dialog; `None` means "use the current global
    /// setting". `do_cleanup` and `do_draft` each add the corresponding version
    /// column, exactly like the Cleaned/Drafted tabs on a dictation.
    ///
    /// Returns the new recording id.
    #[allow(clippy::too_many_arguments)]
    pub async fn transcribe_file(
        &self,
        app: &AppHandle,
        src_path: &str,
        stt_provider: Option<String>,
        stt_model: Option<String>,
        llm_provider: Option<String>,
        llm_model: Option<String>,
        draft_llm_provider: Option<String>,
        draft_llm_model: Option<String>,
        do_cleanup: bool,
        do_draft: bool,
        do_diarize: bool,
        do_meeting_notes: bool,
    ) -> Result<String> {
        let src = PathBuf::from(src_path);
        if !src.is_file() {
            return Err(anyhow!("file not found: {src_path}"));
        }
        let ext = src
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        if !crate::stt::is_supported_audio_ext(&ext) {
            return Err(anyhow!(
                "unsupported audio format '.{ext}' — use wav, mp3, m4a, aac, ogg, opus, flac, or webm"
            ));
        }

        // Copy into audio_dir/YYYY-MM-DD/{uuid}.{ext}, preserving the original
        // extension so the STT providers (which sniff by container/filename)
        // and the <audio> playback both get the right format.
        let date = Utc::now().format("%Y-%m-%d").to_string();
        let id_seed = uuid::Uuid::new_v4().to_string();
        let dir = self.audio_dir.join(&date);
        std::fs::create_dir_all(&dir).with_context(|| format!("creating {dir:?}"))?;
        let dest = dir.join(format!("{id_seed}.{ext}"));
        std::fs::copy(&src, &dest).with_context(|| format!("copying upload to {dest:?}"))?;

        // Normalise exotic WAVs to 16 kHz mono 16-bit PCM at ingest.
        //
        // This is the hard blocker for meeting capture. External field
        // recorders (DJI, Zoom, Tascam) write 24-bit by default — 144 KB/s at
        // 48 kHz mono, so any recording past ~2.3 minutes crossed the 20 MB
        // chunk threshold and then failed outright, because the chunker read
        // samples as i16. An hour-long meeting is ~520 MB and never had a
        // chance. Transcoding here fixes the failure AND shrinks the upload
        // about 9x at no cost to the transcript (every provider resamples to
        // 16 kHz internally anyway).
        //
        // Best-effort: a file we can't decode is left exactly as it was and
        // handed to the provider, which may well cope with it server-side.
        if ext == "wav" {
            let dest_for_convert = dest.clone();
            match tokio::task::spawn_blocking(move || {
                crate::audio::wavio::canonicalize_in_place(&dest_for_convert)
            })
            .await
            {
                Ok(Ok(true)) => tracing::info!(?dest, "upload transcoded for transcription"),
                Ok(Ok(false)) => {}
                Ok(Err(e)) => tracing::warn!("upload transcode skipped (non-fatal): {e:#}"),
                Err(e) => tracing::warn!("upload transcode task failed (non-fatal): {e}"),
            }
        }

        let device_name = self.settings.lock().device_name.clone();
        let record_id = self
            .history
            .insert_upload(&dest, ClippyMode::Light, &device_name)?;
        if do_diarize || do_meeting_notes {
            self.history
                .set_meeting_metadata(&record_id, true, do_diarize, None)?;
        }
        // Show the pending row immediately; the pipeline fills it in.
        let _ = app.emit("wispr:history_changed", ());

        let outcome = self
            .run_upload_pipeline(
                app,
                &record_id,
                &dest,
                stt_provider,
                stt_model,
                llm_provider,
                llm_model,
                draft_llm_provider,
                draft_llm_model,
                do_cleanup,
                do_draft,
                do_diarize,
                do_meeting_notes,
            )
            .await;

        if let Err(e) = &outcome {
            let raw = format!("{e:#}");
            tracing::warn!(record_id = %record_id, "upload pipeline failed: {raw}");
            let _ = self.history.set_error(&record_id, &raw);
            let _ = app.emit(
                "wispr:flow_error",
                user_friendly_error(&raw),
            );
        }
        let _ = app.emit("wispr:history_changed", ());
        outcome.map(|_| record_id)
    }

    /// The STT (+ optional cleanup/draft) body for an uploaded file. Separate
    /// from `do_pipeline` because uploads skip everything mic-specific: no
    /// capture-gap / mic-wake-up telemetry, no silence trimming, no focus
    /// capture or injection.
    #[allow(clippy::too_many_arguments)]
    async fn run_upload_pipeline(
        &self,
        app: &AppHandle,
        record_id: &str,
        audio_path: &std::path::Path,
        stt_provider: Option<String>,
        stt_model: Option<String>,
        llm_provider: Option<String>,
        llm_model: Option<String>,
        draft_llm_provider: Option<String>,
        draft_llm_model: Option<String>,
        do_cleanup: bool,
        do_draft: bool,
        do_diarize: bool,
        do_meeting_notes: bool,
    ) -> Result<()> {
        let mut tl = Timeline::new();
        let base_settings = self.settings();

        // Groq/OpenAI cap a single request at 25 MB and only WAV can be
        // auto-chunked; reject an oversized non-WAV up front with a clear
        // message rather than a cryptic decode error inside the chunker.
        // Deepgram/ElevenLabs stream the whole file server-side, so they're fine.
        let effective_stt = stt_provider
            .clone()
            .unwrap_or_else(|| base_settings.stt_provider.clone());
        let file_bytes = std::fs::metadata(audio_path).map(|m| m.len()).unwrap_or(0);
        let is_wav = audio_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("wav"))
            .unwrap_or(false);
        let chunkable_provider = effective_stt == "deepgram" || effective_stt == "elevenlabs";
        if !is_wav && !chunkable_provider && file_bytes > 24 * 1024 * 1024 {
            return Err(anyhow!(
                "file is {:.0} MB — too large for this engine to send in one piece. Pick Deepgram for long files, or split the audio.",
                file_bytes as f64 / (1024.0 * 1024.0)
            ));
        }

        // STT provider — clone settings and apply the per-batch override so we
        // reuse the exact key-lookup + model-validation logic in build_stt_provider.
        let mut stt_settings = base_settings.clone();
        if let Some(p) = &stt_provider {
            stt_settings.stt_provider = p.clone();
        }
        if let Some(m) = &stt_model {
            stt_settings.stt_model = m.clone();
        }
        let stt = build_stt_provider(&stt_settings)?;
        let stt_name = stt.name();

        // Diarization is only honoured on providers that actually have a
        // speaker model. The dialog greys the option out for Groq/OpenAI, but
        // enforce it here too — a per-batch provider override could otherwise
        // route a diarize request to Whisper, which would return an unlabelled
        // wall of text with no indication anything was dropped.
        let diarize = do_diarize
            && crate::stt::provider_supports_diarization(stt_name, &stt_settings.stt_model);
        if do_diarize && !diarize {
            tl.mark(format!(
                "diarization skipped · {} has no speaker model",
                pretty_provider(stt_name)
            ));
        }

        tl.mark(format!(
            "upload STT → {} ({}){}",
            pretty_provider(stt_name),
            stt_settings.stt_model,
            if diarize { " · diarized" } else { "" }
        ));

        // Uploaded files can be long (a whole voice memo), so give STT a wider
        // 180s ceiling than the 120s live-dictation cap.
        let stt_t0 = std::time::Instant::now();
        let stt_opts = crate::stt::SttOptions {
            language: stt_settings.language_hint.clone(),
            diarize,
        };
        let stt_future = stt.transcribe(audio_path, &stt_opts);
        let stt_result =
            tokio::time::timeout(std::time::Duration::from_secs(180), stt_future).await;
        let stt_elapsed = stt_t0.elapsed().as_millis() as i64;
        tl.stt_ms = Some(stt_elapsed);

        let transcript = match stt_result {
            Err(_) => {
                tl.mark(format!("STT TIMED OUT after {stt_elapsed}ms (180s cap)"));
                self.persist_timeline(record_id, &tl);
                return Err(anyhow!(
                    "Transcription timed out after 180s — try a shorter file"
                ));
            }
            Ok(Err(e)) => {
                tl.mark(format!(
                    "STT FAILED after {stt_elapsed}ms · {}",
                    e.to_string().lines().next().unwrap_or("error")
                ));
                self.persist_timeline(record_id, &tl);
                return Err(anyhow::Error::new(e)).with_context(|| {
                    format!("{} transcription request", pretty_provider(stt_name))
                });
            }
            Ok(Ok(t)) => {
                let speakers = t
                    .speakers
                    .as_ref()
                    .map(|turns| {
                        let distinct = turns
                            .iter()
                            .map(|x| x.speaker.as_str())
                            .collect::<std::collections::BTreeSet<_>>()
                            .len();
                        format!(" · {distinct} speakers, {} turns", turns.len())
                    })
                    .unwrap_or_default();
                tl.mark(format!(
                    "STT ok · {stt_elapsed}ms · {} chars{speakers}",
                    t.text.chars().count()
                ));
                t
            }
        };

        // Diarization was asked for and the provider ran, but the audio had no
        // detectable speaker split (one person, or speakers too similar). The
        // transcript is still fine — say so rather than leaving the user to
        // wonder why there are no labels.
        if diarize && transcript.speakers.is_none() {
            let _ = app.emit(
                "wispr:clippy_message",
                "Transcribed, but no separate speakers were detected in this recording.",
            );
        }

        // We didn't record the file, so its length comes from the provider's
        // reported audio duration (Groq/Deepgram return it). captured == duration
        // for an upload — there's no mic to drop.
        let dur_ms = transcript
            .duration_seconds
            .map(|d| (d * 1000.0) as i64)
            .unwrap_or(0);
        self.history.set_duration(record_id, dur_ms, dur_ms)?;
        self.usage.record_stt(
            stt_name,
            &stt_settings.stt_model,
            transcript
                .duration_seconds
                .unwrap_or_else(|| (dur_ms.max(0) as f64) / 1000.0),
        );
        self.history
            .set_transcript(record_id, &transcript.text, stt_name)?;
        let turns_json = transcript.speakers.as_ref().and_then(|turns| {
            serde_json::to_string(&serde_json::json!({
                "version": 1,
                "turns": turns,
            }))
            .ok()
        });
        if do_diarize || do_meeting_notes {
            self.history.set_meeting_metadata(
                record_id,
                true,
                diarize,
                turns_json.as_deref(),
            )?;
        }

        // Optional cleanup and/or draft. Both may be requested from the dialog;
        // each writes into its own column so the history row shows both tabs.
        //
        // Meeting notes reuse the Drafting transform with a prompt override,
        // but persist in their own column. Draft and Meeting Notes may both be
        // selected and generated from the same raw transcript.
        if do_cleanup || do_draft || do_meeting_notes {
            let provider_id = llm_provider
                .clone()
                .unwrap_or_else(|| base_settings.llm_provider.clone());
            let model = llm_model
                .clone()
                .unwrap_or_else(|| base_settings.llm_model.clone());
            self.history.update_status(record_id, Status::Cleaning)?;

            if do_cleanup {
                let llm = build_llm_provider(&provider_id, model.clone())?;
                let custom = custom_prompt_for(&base_settings, Mode::Light);
                tl.mark(format!("cleanup → {} ({})", llm.name(), model));
                let t0 = std::time::Instant::now();
                let cleaned = clippy::clean(
                    &transcript.text,
                    ClippyMode::Light,
                    custom.as_deref(),
                    None,
                    llm.as_ref(),
                )
                .await;
                tl.cleanup_ms = Some(t0.elapsed().as_millis() as i64);
                self.usage
                    .record_llm(llm.name(), &model, cleaned.usage.as_ref());
                self.history.set_alt(
                    record_id,
                    AltKind::Cleaned,
                    &cleaned.text,
                    Some(llm.name()),
                    cleaned.used_clippy,
                    cleaned.note,
                )?;
            }

            if do_draft || do_meeting_notes {
                let draft_provider_id = draft_llm_provider
                    .clone()
                    .unwrap_or_else(|| base_settings.draft_llm_provider.clone());
                let draft_model = draft_llm_model
                    .clone()
                    .unwrap_or_else(|| base_settings.draft_llm_model.clone());
                let llm = build_llm_provider(&draft_provider_id, draft_model.clone())?;
                // Meeting notes override the Drafting prompt; a user's custom
                // draft prompt still wins for a plain draft.
                let custom = if do_meeting_notes {
                    Some(if base_settings.custom_meeting_prompt.trim().is_empty() {
                        crate::llm::prompts::MEETING_NOTES_SYSTEM.to_string()
                    } else {
                        base_settings.custom_meeting_prompt.clone()
                    })
                } else {
                    custom_prompt_for(&base_settings, Mode::Drafting)
                };
                tl.mark(format!(
                    "{} → {} ({})",
                    if do_meeting_notes { "meeting notes" } else { "draft" },
                    llm.name(),
                    draft_model
                ));
                let t0 = std::time::Instant::now();
                // A meeting transcript is long and the user is watching a
                // progress row, not waiting on a paste — give the model the
                // generous on-demand deadline instead of the paste-latency one.
                let drafted = clippy::clean_with_timeout(
                    &transcript.text,
                    ClippyMode::Drafting,
                    custom.as_deref(),
                    None,
                    llm.as_ref(),
                    if do_meeting_notes {
                        clippy::ON_DEMAND_TIMEOUT
                    } else {
                        llm.timeout_hint()
                    },
                )
                .await;
                let d = t0.elapsed().as_millis() as i64;
                tl.cleanup_ms = Some(tl.cleanup_ms.unwrap_or(0) + d);
                self.usage
                    .record_llm(llm.name(), &draft_model, drafted.usage.as_ref());
                self.history.set_alt(
                    record_id,
                    if do_meeting_notes { AltKind::MeetingNotes } else { AltKind::Drafted },
                    &drafted.text,
                    Some(llm.name()),
                    drafted.used_clippy,
                    drafted.note,
                )?;
            }

            // Draft and Meeting Notes are independent artifacts. The block
            // above prioritises Meeting Notes when both were selected, so run
            // the normal drafting prompt as a second pass and keep both tabs.
            if do_draft && do_meeting_notes {
                let draft_provider_id = draft_llm_provider
                    .clone()
                    .unwrap_or_else(|| base_settings.draft_llm_provider.clone());
                let draft_model = draft_llm_model
                    .clone()
                    .unwrap_or_else(|| base_settings.draft_llm_model.clone());
                let llm = build_llm_provider(&draft_provider_id, draft_model.clone())?;
                let custom = custom_prompt_for(&base_settings, Mode::Drafting);
                tl.mark(format!("draft -> {} ({})", llm.name(), draft_model));
                let t0 = std::time::Instant::now();
                let drafted = clippy::clean_with_timeout(
                    &transcript.text,
                    ClippyMode::Drafting,
                    custom.as_deref(),
                    None,
                    llm.as_ref(),
                    llm.timeout_hint(),
                )
                .await;
                let d = t0.elapsed().as_millis() as i64;
                tl.cleanup_ms = Some(tl.cleanup_ms.unwrap_or(0) + d);
                self.usage
                    .record_llm(llm.name(), &draft_model, drafted.usage.as_ref());
                self.history.set_alt(
                    record_id,
                    AltKind::Drafted,
                    &drafted.text,
                    Some(llm.name()),
                    drafted.used_clippy,
                    drafted.note,
                )?;
            }
        }

        // Count uploads toward lifetime stats too (words + audio seconds).
        {
            let words = transcript.text.split_whitespace().count() as i64;
            let date = chrono::Local::now().format("%Y-%m-%d").to_string();
            let _ = self
                .history
                .record_session(&date, words, dur_ms, ClippyMode::Light);
        }

        // Auto-title, same fire-and-forget path as a dictation.
        let title_cfg = {
            let s = self.settings.lock();
            s.auto_title
                .then(|| (s.title_provider.clone(), s.title_model.clone()))
        };
        if let Some((title_provider, title_model)) = title_cfg {
            let history = self.history.clone();
            let app_for_title = app.clone();
            let rid = record_id.to_string();
            let text_for_title = transcript.text.clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(Some(title)) =
                    generate_title(&text_for_title, &title_provider, &title_model).await
                {
                    if history.set_title(&rid, &title).is_ok() {
                        let _ = app_for_title.emit("wispr:history_changed", ());
                    }
                }
            });
        }

        self.history.update_status(record_id, Status::Done)?;
        crate::sync::engine::notify_recording_done(app);
        self.persist_timeline(record_id, &tl);
        Ok(())
    }

    /// Generate a Cleaned or Drafted version for an existing recording.
    /// The raw transcript must already exist (i.e. STT has run); we just
    /// feed it through the LLM with the appropriate prompt and persist
    /// the result to the correct column. Returns the generated text so
    /// the frontend can show it immediately without a full history refresh.
    pub async fn generate_alt_version(
        &self,
        record_id: &str,
        kind: &str,
        provider_override: Option<String>,
        model_override: Option<String>,
    ) -> Result<String> {
        let target = match kind {
            "cleaned" => AltKind::Cleaned,
            "drafted" => AltKind::Drafted,
            "meeting_notes" => AltKind::MeetingNotes,
            other => return Err(anyhow!("unknown alt-version kind '{other}'")),
        };

        let rec = self
            .history
            .get(record_id)?
            .ok_or_else(|| anyhow!("recording {record_id} not found"))?;
        let transcript = rec
            .transcript
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| anyhow!("no raw transcript yet — retry the recording first"))?;

        let settings = self.settings();
        let mode = match target {
            // Cleaned-raw uses the Light prompt (now the "cleaned raw" formatter).
            AltKind::Cleaned => Mode::Light,
            AltKind::Drafted => Mode::Drafting,
            AltKind::MeetingNotes => Mode::Drafting,
        };

        let (default_provider, default_model) = if matches!(target, AltKind::Cleaned) {
            (settings.llm_provider.clone(), settings.llm_model.clone())
        } else {
            (settings.draft_llm_provider.clone(), settings.draft_llm_model.clone())
        };
        let provider_id = provider_override.unwrap_or(default_provider);
        let model = model_override.unwrap_or(default_model);
        let llm: Box<dyn LlmProvider> = build_llm_provider(&provider_id, model.clone())?;

        let custom = if matches!(target, AltKind::MeetingNotes) {
            Some(if settings.custom_meeting_prompt.trim().is_empty() {
                crate::llm::prompts::MEETING_NOTES_SYSTEM.to_string()
            } else {
                settings.custom_meeting_prompt.clone()
            })
        } else {
            custom_prompt_for(&settings, mode)
        };
        // On-demand "generate cleaned/drafted version" from History has no
        // app-context — the original target window is long gone. Skip the
        // hint and let the LLM pick register from the brief's content.
        //
        // Generous deadline: this path is a button click with a spinner, not
        // a paste the user is waiting on, so let a slow model finish rather
        // than handing back the raw transcript.
        let cleaned = clippy::clean_with_timeout(
            transcript,
            ClippyMode::from(mode),
            custom.as_deref(),
            None,
            llm.as_ref(),
            clippy::ON_DEMAND_TIMEOUT,
        )
        .await;
        self.usage
            .record_llm(llm.name(), &model, cleaned.usage.as_ref());

        self.history.set_alt(
            record_id,
            target,
            &cleaned.text,
            Some(llm.name()),
            cleaned.used_clippy,
            cleaned.note,
        )?;
        if matches!(target, AltKind::MeetingNotes) {
            self.history.set_meeting_metadata(record_id, true, rec.diarization_enabled, rec.speaker_turns.as_deref())?;
        }
        Ok(cleaned.text)
    }

    /// Re-run transcription + cleanup on an existing recording. Used by the
    /// retry button in the History UI when the original attempt errored
    /// (rate limit, network blip, etc.). The audio file must still exist.
    pub async fn retry_recording(&self, app: &AppHandle, record_id: &str) -> Result<()> {
        self.retry_recording_with(app, record_id, None, None, false, true).await
    }

    /// Re-run STT with one-off engine choices. Used by the consolidated
    /// History Rerun dialog; choices do not mutate application defaults.
    pub async fn retry_recording_with(
        &self,
        app: &AppHandle,
        record_id: &str,
        stt_provider: Option<String>,
        stt_model: Option<String>,
        diarize: bool,
        run_default_cleanup: bool,
    ) -> Result<()> {
        let rec = self
            .history
            .get(record_id)?
            .ok_or_else(|| anyhow!("recording {record_id} not found"))?;
        if !rec.audio_path.exists() {
            return Err(anyhow!(
                "audio file gone (likely purged by retention) — cannot retry"
            ));
        }

        self.history.bump_retry(record_id)?;
        // Clear any previous error so the row stops showing as failed.
        self.history
            .update_status(record_id, Status::Transcribing)?;
        let _ = app.emit("wispr:state", "transcribing");

        // Flight recorder for the retry attempt too, so the (i) inspector shows
        // the fresh timing (and overwrites the failed original's timeline).
        let mut tl = Timeline::new();

        let mut stt_settings = self.settings();
        if let Some(provider) = stt_provider { stt_settings.stt_provider = provider; }
        if let Some(model) = stt_model { stt_settings.stt_model = model; }
        let stt = build_stt_provider(&stt_settings)?;
        let stt_name = stt.name();
        let diarize = diarize
            && crate::stt::provider_supports_diarization(stt_name, &stt_settings.stt_model);
        tl.mark(format!(
            "retry STT request → {} ({})",
            pretty_provider(stt_name),
            stt_settings.stt_model
        ));

        let stt_t0 = std::time::Instant::now();
        let retry_opts = crate::stt::SttOptions {
            language: stt_settings.language_hint.clone(),
            diarize,
        };
        let stt_res = stt.transcribe(&rec.audio_path, &retry_opts).await;
        let stt_elapsed = stt_t0.elapsed().as_millis() as i64;
        tl.stt_ms = Some(stt_elapsed);
        let transcript = match stt_res {
            Ok(t) => {
                tl.mark(format!(
                    "retry STT ok · {stt_elapsed}ms · {} chars",
                    t.text.chars().count()
                ));
                t
            }
            Err(e) => {
                tl.mark(format!(
                    "retry STT FAILED after {stt_elapsed}ms · {}",
                    e.to_string().lines().next().unwrap_or("error")
                ));
                self.persist_timeline(record_id, &tl);
                return Err(anyhow::Error::new(e)).with_context(|| {
                    format!(
                        "{provider} transcription retry",
                        provider = pretty_provider(stt_name)
                    )
                });
            }
        };
        self.usage.record_stt(
            stt_name,
            &stt_settings.stt_model,
            transcript
                .duration_seconds
                .unwrap_or_else(|| (rec.duration_ms.max(0) as f64) / 1000.0),
        );
        self.history
            .set_transcript(record_id, &transcript.text, stt_name)?;
        let turns_json = transcript.speakers.as_ref().and_then(|turns| {
            serde_json::to_string(&serde_json::json!({ "version": 1, "turns": turns })).ok()
        });
        if diarize || rec.is_meeting {
            self.history.set_meeting_metadata(
                record_id,
                diarize || rec.is_meeting,
                diarize,
                turns_json.as_deref(),
            )?;
        }

        let mode = match rec.mode {
            ClippyMode::Light => Mode::Light,
            ClippyMode::Advanced => Mode::Advanced,
            ClippyMode::Drafting => Mode::Drafting,
        };
        let needs_clippy = run_default_cleanup && match mode {
            Mode::Light => stt_settings.auto_clean_in_light,
            Mode::Advanced => stt_settings.auto_clean_in_advanced,
            Mode::Drafting => stt_settings.auto_clean_in_drafting,
        };

        let final_text = if needs_clippy {
            let _ = app.emit("wispr:state", "cleaning");
            self.history.update_status(record_id, Status::Cleaning)?;
            // Retry path: same single global provider + model.
            let provider_id = stt_settings.llm_provider.clone();
            let model = stt_settings.llm_model.clone();
            let llm: Box<dyn LlmProvider> = build_llm_provider(&provider_id, model.clone())?;
            let custom = custom_prompt_for(&stt_settings, mode);
            // Retry path has no captured focus context (it ran possibly
            // hours ago into a different app), so skip the app-context
            // hint here. The user can always trigger a fresh F9 if they
            // want app-adapted output.
            tl.mark(format!("retry cleanup request → {} ({})", llm.name(), model));
            let clean_t0 = std::time::Instant::now();
            // Same reasoning as generate_alt_version: a retry is user-initiated
            // from History, so favour finishing over failing fast.
            let cleaned = clippy::clean_with_timeout(
                &transcript.text,
                ClippyMode::from(mode),
                custom.as_deref(),
                None,
                llm.as_ref(),
                clippy::ON_DEMAND_TIMEOUT,
            )
            .await;
            let clean_elapsed = clean_t0.elapsed().as_millis() as i64;
            tl.cleanup_ms = Some(clean_elapsed);
            tl.mark(format!("retry cleanup done · {clean_elapsed}ms"));
            self.usage
                .record_llm(llm.name(), &model, cleaned.usage.as_ref());
            let alt = if matches!(mode, Mode::Drafting) {
                AltKind::Drafted
            } else {
                AltKind::Cleaned
            };
            self.history.set_alt(
                record_id,
                alt,
                &cleaned.text,
                Some(llm.name()),
                cleaned.used_clippy,
                cleaned.note,
            )?;
            cleaned.text
        } else {
            transcript.text.clone()
        };

        // Don't auto-paste on retry — the user is likely viewing history,
        // not waiting at a target field. Leave injection up to the manual
        // "copy" button.
        let _ = final_text;
        self.persist_timeline(record_id, &tl);
        self.history.update_status(record_id, Status::Done)?;
        crate::sync::engine::notify_recording_done(app);
        let _ = app.emit("wispr:state", "idle");
        Ok(())
    }
}

// ─── Escape-stop dynamic hotkey ─────────────────────────────────────────────
//
// Escape means "stop and send" for both Starting and Recording. We register it
// as soon as a session is reserved, then remove it on stop/failure so we don't
// steal it from focused apps the rest of the
// time (closing dialogs, exiting autocomplete, leaving fullscreen, etc.).

fn monotonic_ms() -> u64 {
    static EPOCH: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    EPOCH
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_millis() as u64
}

fn publish_snapshot(app: &AppHandle, snapshot: &FlowSnapshot) {
    let _ = app.emit("wispr:flow_snapshot", snapshot);

    // Transitional compatibility for non-floater windows. The floater consumes
    // only the revisioned snapshot and never infers backend state from these.
    if let Some(mode) = snapshot.mode.as_deref() {
        let _ = app.emit("wispr:mode", mode);
    }
    let legacy_state = match snapshot.phase {
        FlowPhase::Idle | FlowPhase::Succeeded | FlowPhase::Failed => "idle",
        FlowPhase::Starting | FlowPhase::Recording => "recording",
        FlowPhase::Stopping => "transcribing",
        FlowPhase::Processing => match snapshot.stage {
            Some(FlowStage::Denoising) => "denoising",
            Some(FlowStage::Cleaning) => "cleaning",
            Some(FlowStage::Injecting) => "injecting",
            Some(FlowStage::Transcribing) | None => "transcribing",
        },
    };
    let _ = app.emit("wispr:state", legacy_state);
}

use std::str::FromStr;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

fn arm_escape_stop(app: &AppHandle, flow: &Flow) {
    let Ok(esc) = Shortcut::from_str("Escape") else {
        return;
    };
    // If somehow already registered (race / leftover from a previous
    // start_recording that errored out before unregister), skip — we'd just
    // get a "shortcut already registered" error and the existing handler is
    // still valid.
    if app.global_shortcut().is_registered(esc.clone()) {
        return;
    }
    let flow_clone = flow.clone();
    let app_clone = app.clone();
    let esc_match = esc.clone();
    let result = app
        .global_shortcut()
        .on_shortcut(esc, move |_a, fired, event| {
            if fired != &esc_match {
                return;
            }
            // Only react on key-down. Escape's key-up is not interesting.
            if event.state() != ShortcutState::Pressed {
                return;
            }
            // Only stop if recording is actually active. If it's not (e.g. user
            // hit Escape after recording already ended but before our unregister
            // ran), do nothing — emitting a stop would be harmless but we'd
            // rather no-op cleanly.
            flow_clone.stop_recording(&app_clone);
        });
    if let Err(e) = result {
        tracing::debug!("escape-stop register failed (non-fatal): {e:#}");
    }
}

fn disarm_escape_stop(app: &AppHandle) {
    let Ok(esc) = Shortcut::from_str("Escape") else {
        return;
    };
    if app.global_shortcut().is_registered(esc.clone()) {
        if let Err(e) = app.global_shortcut().unregister(esc) {
            tracing::debug!("escape-stop unregister failed (non-fatal): {e:#}");
        }
    }
}

#[cfg(test)]
mod coordinator_tests {
    use super::*;

    /// Escape arm/disarm is applied off the deciding thread (the global-shortcut
    /// callback holds the plugin's registry mutex, so touching it there
    /// deadlocks). Ordering is preserved by revision instead: whichever
    /// transition stamped last wins, and an older applier that wakes up late
    /// must decline rather than re-arm a session that has already stopped.
    #[test]
    fn stale_escape_applier_cannot_rearm_after_a_newer_stop() {
        let mut intent = EscapeIntent::default();

        let start = intent.record(true);
        assert_eq!(intent.claim(start), Some(true));

        // A stop lands before the start's applier got to run.
        let stop = intent.record(false);
        assert_ne!(start, stop);
        assert_eq!(
            intent.claim(start),
            None,
            "the superseded start must not re-arm Escape"
        );
        assert_eq!(intent.claim(stop), Some(false));

        // Re-arming for a fresh session still works after the stale decline.
        let restart = intent.record(true);
        assert_eq!(intent.claim(restart), Some(true));
        assert_eq!(intent.claim(stop), None);
    }

    fn session(id: &str, mode: Mode, generation: u64) -> SessionContext {
        SessionContext {
            id: id.to_owned(),
            mode,
            force_clean: false,
            capture_generation: generation,
            input: InputDisposition::Undecided,
            mic: MicPhase::Waking,
            mic_ready_ms: None,
        }
    }

    fn flight(id: &str, mode: Mode) -> InFlight {
        InFlight {
            mode,
            record_id: id.to_owned(),
            audio_path: PathBuf::from(format!("{id}.wav")),
            captured_focus: None,
            force_clean: false,
        }
    }

    fn down(
        state: &mut FlowState,
        trigger: &str,
        at_ms: u64,
        start: Option<SessionContext>,
    ) -> (Option<FlowSnapshot>, Option<FlowAction>) {
        let availability = state.availability();
        let decision = state
            .input
            .physical_down(trigger, at_ms, availability);
        let mut action = None;
        let snapshot = Flow::apply_decision_locked(state, decision, start, &mut action);
        (snapshot, action)
    }

    fn up(
        state: &mut FlowState,
        trigger: &str,
        at_ms: u64,
    ) -> (Option<FlowSnapshot>, Option<FlowAction>) {
        let decision = state.input.physical_up(trigger, at_ms);
        let mut action = None;
        let snapshot = Flow::apply_decision_locked(state, decision, None, &mut action);
        (snapshot, action)
    }

    #[test]
    fn queued_sub_700_up_latches_then_startup_enters_recording() {
        let mut state = FlowState::default();
        let (_, action) = down(
            &mut state,
            "F8",
            1_000,
            Some(session("session-a", Mode::Light, 11)),
        );
        assert!(matches!(action, Some(FlowAction::Start(_))));

        let (snapshot, action) = up(&mut state, "F8", 1_699);
        assert!(action.is_none());
        assert_eq!(snapshot.unwrap().input, Some(InputDisposition::Latched));

        let completion = Flow::apply_capture_completion_locked(
            &mut state,
            "session-a",
            Ok(flight("record-a", Mode::Light)),
        );
        assert!(completion.action.is_none());
        assert_eq!(completion.snapshot.unwrap().phase, FlowPhase::Recording);
        assert!(matches!(
            &state.runtime,
            RuntimeState::Recording {
                session: SessionContext {
                    input: InputDisposition::Latched,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn exact_700_up_dispatches_one_stop_after_startup() {
        let mut state = FlowState::default();
        let _ = down(
            &mut state,
            "F8",
            5_000,
            Some(session("session-a", Mode::Light, 12)),
        );

        let (_, action) = up(&mut state, "F8", 5_700);
        assert!(matches!(action, Some(FlowAction::DisarmEscape)));
        let (_, duplicate_up) = up(&mut state, "F8", 5_701);
        assert!(duplicate_up.is_none());

        let completion = Flow::apply_capture_completion_locked(
            &mut state,
            "session-a",
            Ok(flight("record-a", Mode::Light)),
        );
        assert!(matches!(completion.action, Some(FlowAction::Stop { .. })));
        assert!(matches!(&state.runtime, RuntimeState::Processing { .. }));

        let duplicate = Flow::apply_capture_completion_locked(
            &mut state,
            "session-a",
            Ok(flight("duplicate", Mode::Light)),
        );
        assert!(duplicate.action.is_none());
        assert!(duplicate.snapshot.is_none());
        assert!(duplicate.stale.is_some());
        assert!(matches!(&state.runtime, RuntimeState::Processing { .. }));
    }

    #[test]
    fn second_key_and_escape_stop_the_original_session_mode() {
        let mut state = FlowState::default();
        let mut original = session("advanced", Mode::Advanced, 20);
        original.force_clean = true;
        let _ = down(&mut state, "F8", 0, Some(original));
        let _ = Flow::apply_capture_completion_locked(
            &mut state,
            "advanced",
            Ok(flight("record-advanced", Mode::Advanced)),
        );

        let (_, action) = down(&mut state, "F9", 100, None);
        match action {
            Some(FlowAction::Stop {
                session_id,
                in_flight,
            }) => {
                assert_eq!(session_id, "advanced");
                assert!(matches!(in_flight.mode, Mode::Advanced));
            }
            _ => panic!("second dictation Down must stop the original session"),
        }

        let mut state = FlowState::default();
        let _ = down(
            &mut state,
            "F8",
            0,
            Some(session("escape-owned", Mode::Drafting, 21)),
        );
        let _ = Flow::apply_capture_completion_locked(
            &mut state,
            "escape-owned",
            Ok(flight("record-draft", Mode::Drafting)),
        );
        let availability = state.availability();
        let decision = state.input.escape(availability);
        let mut action = None;
        let _ = Flow::apply_decision_locked(&mut state, decision, None, &mut action);
        assert!(matches!(
            action,
            Some(FlowAction::Stop {
                session_id,
                in_flight: InFlight {
                    mode: Mode::Drafting,
                    ..
                },
            }) if session_id == "escape-owned"
        ));
    }

    #[test]
    fn processing_down_is_busy_and_never_starts() {
        let mut state = FlowState::default();
        state.runtime = RuntimeState::Processing {
            session: session("busy", Mode::Light, 30),
            record_id: "record-busy".to_owned(),
        };
        state.snapshot.session_id = Some("busy".to_owned());
        state.snapshot.phase = FlowPhase::Processing;

        let (snapshot, action) = down(&mut state, "F9", 100, None);
        assert!(action.is_none());
        let snapshot = snapshot.expect("busy notice revision");
        assert_eq!(snapshot.phase, FlowPhase::Processing);
        assert_eq!(snapshot.notice.unwrap().code, "session_busy");
        assert!(matches!(
            &state.runtime,
            RuntimeState::Processing { session, .. } if session.id == "busy"
        ));
    }

    #[test]
    fn stale_capture_completion_cannot_replace_current_start() {
        let mut state = FlowState::default();
        let _ = down(
            &mut state,
            "F9",
            0,
            Some(session("current", Mode::Drafting, 40)),
        );
        let revision = state.snapshot.revision;

        let stale = Flow::apply_capture_completion_locked(
            &mut state,
            "old-session",
            Ok(flight("old-record", Mode::Light)),
        );
        assert!(stale.snapshot.is_none());
        assert!(stale.action.is_none());
        assert_eq!(stale.stale.unwrap().record_id, "old-record");
        assert_eq!(state.snapshot.revision, revision);
        assert!(matches!(
            &state.runtime,
            RuntimeState::Starting { session, .. } if session.id == "current"
        ));
    }

    #[test]
    fn stale_mic_generation_is_ignored_without_revision() {
        let mut state = FlowState::default();
        let _ = down(
            &mut state,
            "F8",
            0,
            Some(session("mic", Mode::Light, 50)),
        );
        let revision = state.snapshot.revision;

        assert!(Flow::apply_mic_ready_locked(&mut state, 49, 123).is_none());
        assert_eq!(state.snapshot.revision, revision);
        assert_eq!(state.snapshot.mic, MicPhase::Waking);

        let snapshot = Flow::apply_mic_ready_locked(&mut state, 50, 124)
            .expect("matching generation becomes live");
        assert_eq!(snapshot.revision, revision + 1);
        assert_eq!(snapshot.mic, MicPhase::Live);
        assert_eq!(snapshot.mic_ready_ms, Some(124));
    }
}
