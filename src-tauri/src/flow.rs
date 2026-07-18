//! Top-level state machine: Idle → Recording → Transcribing → Cleaning → Injecting → Done|Error.
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
            Ok(Box::new(GroqLlm::new(key, model)))
        }
    }
}

/// Model + prompt for the auto-title nicety. Deliberately a light Groq model
/// regardless of the user's main LLM pick — naming a note should be fast and
/// near-free, and Groq is the one key every install has.
const TITLE_MODEL: &str = "llama-3.1-8b-instant";
const TITLE_SYSTEM: &str = "You title voice notes. Reply with ONLY a short title \
for the user's dictation: 3-7 words, plain language, no quotes, no trailing \
punctuation, written in the same language as the dictation.";

/// One-line descriptor for a finished recording. Ok(None) = transcript too
/// short to be worth an LLM call (the time/duration header is enough).
async fn generate_title(text: &str) -> Result<Option<String>> {
    let trimmed = text.trim();
    if trimmed.split_whitespace().count() < 8 {
        return Ok(None);
    }
    let key = secrets::get(SecretKey::GroqLlm)?
        .or_else(|| secrets::get(SecretKey::GroqStt).ok().flatten())
        .ok_or_else(|| anyhow!("no Groq key for auto-title"))?;
    // The opening of a dictation carries its topic; don't ship a 15-minute
    // monologue to the model just to get five words back.
    let snippet: String = trimmed.chars().take(1200).collect();
    let llm = GroqLlm::new(key, TITLE_MODEL.to_string());
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
                &["gpt-4o-transcribe", "gpt-4o-mini-transcribe", "whisper-1"],
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

#[derive(Default)]
struct FlowState {
    active: Option<InFlight>,
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
    state: Arc<Mutex<FlowState>>,
    history: History,
    settings: Arc<Mutex<AppSettings>>,
    audio_dir: PathBuf,
    audio: AudioController,
    usage: UsageTracker,
    /// Timestamp of the last accepted Down hotkey event. Used to debounce
    /// Windows WM_HOTKEY auto-repeat (~30/sec while a function key is
    /// held) and the spawned-task race where two concurrent Down events
    /// both pass the `state.active.is_some()` check before the first one
    /// sets it. Any Down within 150ms of the previous accepted Down is
    /// dropped — well above the auto-repeat rate, well below a human's
    /// minimum legitimate re-press cadence.
    last_down_at: Arc<Mutex<Option<std::time::Instant>>>,
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
            state: Arc::new(Mutex::new(FlowState::default())),
            history,
            settings: Arc::new(Mutex::new(settings)),
            audio_dir,
            audio,
            usage,
            last_down_at: Arc::new(Mutex::new(None)),
        }
    }

    pub fn settings(&self) -> AppSettings {
        self.settings.lock().clone()
    }

    pub fn set_settings(&self, s: AppSettings) {
        *self.settings.lock() = s;
    }

    pub fn handle_hotkey(&self, app: &AppHandle, evt: HotkeyEvent) {
        // Synchronous debounce gate — kept BEFORE the spawn so two near-
        // simultaneous Down events can't both spawn tasks and race each
        // other through `start_recording_async`. The previous v0.3.1 fix
        // checked `state.active.is_some()` AFTER the spawn, which left a
        // ~ms window where both tasks read None and both called start →
        // second one errored with "recording already in progress" →
        // flow_error event → Clippy front-end force-reset state to idle
        // (ear disappeared, red toast) even though the original recording
        // task was still happily running on the audio thread.
        //
        // Up events bypass the gate — releasing a key is always a
        // meaningful action and never auto-repeats.
        if matches!(evt.edge, Edge::Down) {
            const DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(150);
            let now = std::time::Instant::now();
            let mut last = self.last_down_at.lock();
            if let Some(prev) = *last {
                if now.duration_since(prev) < DEBOUNCE {
                    tracing::trace!(
                        elapsed_ms = now.duration_since(prev).as_millis() as u64,
                        "debouncing rapid Down event (auto-repeat or race)"
                    );
                    return;
                }
            }
            *last = Some(now);
        }

        let app = app.clone();
        let this = self.clone();
        tauri::async_runtime::spawn(async move {
            // Two routes to sticky behaviour:
            //   1. The user pressed the sticky-invoke combo (e.g. Win+F8) for
            //      this single press — `evt.sticky_invoke == true`. Forces
            //      sticky regardless of settings.
            //   2. The per-mode sticky_* setting is on — sticky is the default
            //      for this mode.
            // Both end up in sticky behaviour (ignore key-up, toggle on each
            // key-down).
            let s = this.settings();
            let mode_sticky = match evt.mode {
                Mode::Light => s.sticky_light,
                Mode::Advanced => s.sticky_advanced,
                Mode::Drafting => s.sticky_drafting,
            };
            let sticky = evt.sticky_invoke || mode_sticky;

            let mode_str = mode_to_str(evt.mode);

            if sticky {
                // Sticky: only react to Down edges; toggle recording state.
                if !matches!(evt.edge, Edge::Down) {
                    return;
                }
                let already_recording = this.state.lock().active.is_some();
                if already_recording {
                    if let Err(e) = this.finish_recording_async(&app).await {
                        tracing::error!("finish_recording (sticky stop) failed: {e:#}");
                        let _ = app.emit("wispr:flow_error", e.to_string());
                    }
                } else if let Err(e) = this
                    .start_recording_async(&app, evt.mode, evt.force_clean)
                    .await
                {
                    let raw = e.to_string();
                    if raw.contains("recording already in progress") {
                        // Race / late dispatch — another task won. Don't
                        // alarm the user; the original recording is fine.
                        tracing::debug!("sticky start: dropped duplicate ({raw})");
                    } else {
                        tracing::error!("start_recording (sticky start) failed: {e:#}");
                        let _ = app.emit("wispr:flow_error", raw);
                    }
                } else {
                    let _ = app.emit("wispr:mode", mode_str);
                    let _ = app.emit("wispr:state", "recording");
                }
                return;
            }

            // Push-to-talk: down starts, up finishes.
            //
            // Note: the synchronous debounce gate at the top of
            // `handle_hotkey` already swallows auto-repeats. This
            // additional `active.is_some()` check covers the still-
            // pathological case where a legitimate Down sneaks through
            // > 150ms after the previous accepted Down but recording is
            // still active (e.g. user spammed F8 + Win+F8 alternately).
            match evt.edge {
                Edge::Down => {
                    if this.state.lock().active.is_some() {
                        tracing::trace!("ignoring Down: recording already active");
                        return;
                    }
                    if let Err(e) = this
                        .start_recording_async(&app, evt.mode, evt.force_clean)
                        .await
                    {
                        let raw = e.to_string();
                        if raw.contains("recording already in progress") {
                            // Race-condition belt-and-suspenders: another
                            // task set state.active between our check and
                            // start_recording_async's own check. Silently
                            // ignore — emitting flow_error here would make
                            // the Clippy front-end reset state to idle
                            // even though the OTHER task's recording is
                            // still going. v0.4.0 user-reported bug.
                            tracing::debug!("dropped duplicate start ({raw})");
                        } else {
                            tracing::error!("start_recording failed: {e:#}");
                            let _ = app.emit("wispr:flow_error", raw);
                        }
                    } else {
                        let _ = app.emit("wispr:mode", mode_str);
                        let _ = app.emit("wispr:state", "recording");
                    }
                }
                Edge::Up => {
                    if let Err(e) = this.finish_recording_async(&app).await {
                        tracing::error!("finish_recording failed: {e:#}");
                        let _ = app.emit("wispr:flow_error", e.to_string());
                    }
                }
            }
        });
    }

    async fn start_recording_async(
        &self,
        app: &AppHandle,
        mode: Mode,
        force_clean: bool,
    ) -> Result<()> {
        {
            let state = self.state.lock();
            if state.active.is_some() {
                return Err(anyhow!("recording already in progress"));
            }
        }

        let date = Utc::now().format("%Y-%m-%d").to_string();
        let id_seed = uuid::Uuid::new_v4().to_string();
        let path = self.audio_dir.join(date).join(format!("{id_seed}.wav"));

        // Snapshot where the user is talking BEFORE we start the audio
        // stream. We capture now (not on hotkey-up) because F10 release can
        // itself shift focus (Outlook ribbon keytips), and we want the HWND
        // of where the user actually started speaking.
        let captured_focus = inject::focus::capture();

        // Surface a friendly app name to the floater so it can show
        // "Listening for Outlook" instead of just "Listening". Empty payload
        // = unknown / no capture; the floater falls back to the generic
        // status line. We emit before audio.start so the floater paints the
        // app label simultaneously with the recording state transition.
        let friendly = captured_focus
            .as_ref()
            .and_then(inject::focus::friendly_app_name)
            .unwrap_or_default();
        let _ = app.emit("wispr:active_app", friendly);

        self.audio
            .start(path.clone())
            .await
            .context("starting audio capture")?;
        crate::audio::cues::play_start();
        let device_name = self.settings.lock().device_name.clone();
        let record_id = self
            .history
            .insert_new(&path, ClippyMode::from(mode), &device_name)?;

        let mut state = self.state.lock();
        state.active = Some(InFlight {
            mode,
            record_id,
            audio_path: path,
            captured_focus,
            force_clean,
        });
        drop(state);

        // Arm Escape as a global stop key for the duration of this recording.
        // Dynamically registered/unregistered so we only steal Escape from
        // focused apps while a dictation is genuinely in flight.
        arm_escape_stop(app, self);
        Ok(())
    }

    async fn finish_recording_async(&self, app: &AppHandle) -> Result<()> {
        // Disarm Escape FIRST so subsequent Escape presses go back to the
        // focused app (e.g. close a dialog) instead of being swallowed.
        disarm_escape_stop(app);

        let in_flight = {
            let mut state = self.state.lock();
            state.active.take()
        };
        let Some(in_flight) = in_flight else {
            return Ok(());
        };
        let record_id = in_flight.record_id.clone();

        // Run the pipeline; whatever happens (Ok / Err / step timeout),
        // the wrapper here GUARANTEES the UI returns to idle and any
        // history row is properly closed out. Without this, an unhandled
        // error in STT/LLM leaves Clippy stuck in "thinking" forever and
        // the row stranded in `transcribing`.
        let outcome = self.do_pipeline(app, in_flight).await;

        let _ = app.emit("wispr:state", "idle");
        if let Err(e) = &outcome {
            let raw = format!("{e:#}");
            tracing::warn!(record_id = %record_id, "pipeline failed: {raw}");
            let _ = self.history.set_error(&record_id, &raw);
            let friendly = user_friendly_error(&raw);
            let _ = app.emit("wispr:flow_error", &friendly);
        }
        outcome
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

    async fn do_pipeline(&self, app: &AppHandle, in_flight: InFlight) -> Result<()> {
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

        let _ = app.emit("wispr:state", "transcribing");

        let FinishedRecording {
            path,
            duration_ms,
            captured_ms,
            stream_errored,
            mic_ready_ms,
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
        if mic_ready_ms > 2500
            && !SLOW_MIC_WARNED.swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            let _ = app.emit(
                "wispr:clippy_warning",
                format!(
                    "Your mic took {:.1}s to wake up — the first words of every recording are being cut. Usual fix: turn OFF the mic's audio enhancements and exclusive control (Sound settings → your microphone → Properties).",
                    (mic_ready_ms as f64) / 1000.0
                ),
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
            // wrapper emits idle for us; just return cleanly
            return Ok(());
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
            let _ = app.emit("wispr:state", "denoising");
            let src = path.clone();
            let result =
                tokio::task::spawn_blocking(move || {
                    crate::audio::denoise::denoise_to_side_file(&src, nr)
                })
                .await
                .map_err(anyhow::Error::new)
                .and_then(|r| r);
            let _ = app.emit("wispr:state", "transcribing");
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
        let stt_future = stt.transcribe(&stt_input, stt_settings.language_hint.as_deref());
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
            let _ = app.emit("wispr:state", "cleaning");
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
        if self.settings.lock().auto_title {
            let history = self.history.clone();
            let app_for_title = app.clone();
            let rid = record_id.clone();
            let text_for_title = final_text.clone();
            tauri::async_runtime::spawn(async move {
                match generate_title(&text_for_title).await {
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

        let _ = app.emit("wispr:state", "injecting");
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
        let inj_settings = self.settings();
        let (current_fg, current_ctrl, current_pid) = inject::focus::current_foreground_state();
        let cap_ref = captured_focus.as_ref();
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
        let should_restore_focus =
            !nothing_changed && (same_process || inj_settings.pull_back_on_navigation);

        if let (Some(cap), true) = (cap_ref, should_restore_focus) {
            if let Err(e) = inject::focus::restore(cap) {
                tracing::warn!("focus restore failed (non-fatal): {e:#}");
            }
        } else if nothing_changed && cap_ref.is_some() {
            tracing::debug!("focus restore skipped: foreground + focused control unchanged");
        }

        if !same_process && !inj_settings.pull_back_on_navigation && captured_focus.is_some() {
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
        do_cleanup: bool,
        do_draft: bool,
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

        let device_name = self.settings.lock().device_name.clone();
        let record_id = self
            .history
            .insert_upload(&dest, ClippyMode::Light, &device_name)?;
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
                do_cleanup,
                do_draft,
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
        do_cleanup: bool,
        do_draft: bool,
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
        tl.mark(format!(
            "upload STT → {} ({})",
            pretty_provider(stt_name),
            stt_settings.stt_model
        ));

        // Uploaded files can be long (a whole voice memo), so give STT a wider
        // 180s ceiling than the 120s live-dictation cap.
        let stt_t0 = std::time::Instant::now();
        let stt_future = stt.transcribe(audio_path, stt_settings.language_hint.as_deref());
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
                tl.mark(format!(
                    "STT ok · {stt_elapsed}ms · {} chars",
                    t.text.chars().count()
                ));
                t
            }
        };

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

        // Optional cleanup and/or draft. Both may be requested from the dialog;
        // each writes into its own column so the history row shows both tabs.
        if do_cleanup || do_draft {
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

            if do_draft {
                let llm = build_llm_provider(&provider_id, model.clone())?;
                let custom = custom_prompt_for(&base_settings, Mode::Drafting);
                tl.mark(format!("draft → {} ({})", llm.name(), model));
                let t0 = std::time::Instant::now();
                let drafted = clippy::clean(
                    &transcript.text,
                    ClippyMode::Drafting,
                    custom.as_deref(),
                    None,
                    llm.as_ref(),
                )
                .await;
                let d = t0.elapsed().as_millis() as i64;
                tl.cleanup_ms = Some(tl.cleanup_ms.unwrap_or(0) + d);
                self.usage
                    .record_llm(llm.name(), &model, drafted.usage.as_ref());
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
        if self.settings.lock().auto_title {
            let history = self.history.clone();
            let app_for_title = app.clone();
            let rid = record_id.to_string();
            let text_for_title = transcript.text.clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(Some(title)) = generate_title(&text_for_title).await {
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
    pub async fn generate_alt_version(&self, record_id: &str, kind: &str) -> Result<String> {
        let target = match kind {
            "cleaned" => AltKind::Cleaned,
            "drafted" => AltKind::Drafted,
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
        };

        let provider_id = settings.llm_provider.clone();
        let model = settings.llm_model.clone();
        let llm: Box<dyn LlmProvider> = build_llm_provider(&provider_id, model.clone())?;

        let custom = custom_prompt_for(&settings, mode);
        // On-demand "generate cleaned/drafted version" from History has no
        // app-context — the original target window is long gone. Skip the
        // hint and let the LLM pick register from the brief's content.
        let cleaned = clippy::clean(
            transcript,
            ClippyMode::from(mode),
            custom.as_deref(),
            None,
            llm.as_ref(),
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
        Ok(cleaned.text)
    }

    /// Re-run transcription + cleanup on an existing recording. Used by the
    /// retry button in the History UI when the original attempt errored
    /// (rate limit, network blip, etc.). The audio file must still exist.
    pub async fn retry_recording(&self, app: &AppHandle, record_id: &str) -> Result<()> {
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

        let stt_settings = self.settings();
        let stt = build_stt_provider(&stt_settings)?;
        let stt_name = stt.name();
        tl.mark(format!(
            "retry STT request → {} ({})",
            pretty_provider(stt_name),
            stt_settings.stt_model
        ));

        let stt_t0 = std::time::Instant::now();
        let stt_res = stt
            .transcribe(&rec.audio_path, stt_settings.language_hint.as_deref())
            .await;
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

        let mode = match rec.mode {
            ClippyMode::Light => Mode::Light,
            ClippyMode::Advanced => Mode::Advanced,
            ClippyMode::Drafting => Mode::Drafting,
        };
        let needs_clippy = match mode {
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
            let cleaned = clippy::clean(
                &transcript.text,
                ClippyMode::from(mode),
                custom.as_deref(),
                None,
                llm.as_ref(),
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
// While a recording is in flight, we want a single keystroke to bail out
// cleanly — for sticky-mode sessions the user has no obvious way to "release"
// the trigger key, and across both platforms Escape is the universal
// "abandon" gesture. We register Escape via the global-shortcut plugin ONLY
// during recording so we don't steal it from focused apps the rest of the
// time (closing dialogs, exiting autocomplete, leaving fullscreen, etc.).

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
            if flow_clone.state.lock().active.is_none() {
                return;
            }
            let app2 = app_clone.clone();
            let flow2 = flow_clone.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = flow2.finish_recording_async(&app2).await {
                    tracing::warn!("escape-stop: finish_recording failed: {e:#}");
                }
            });
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
