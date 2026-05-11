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
use crate::history::{History, Status};
use crate::hotkey::{Edge, HotkeyEvent};
use crate::inject;
use crate::llm::{gemini::GeminiLlm, groq::GroqLlm, ClippyMode, LlmProvider};
use crate::secrets::{self, SecretKey};
use crate::settings::{AppSettings, Mode};
use crate::stt::{groq::GroqStt, SttProvider};
use crate::usage::UsageTracker;

const MIN_DURATION_MS: i64 = 300;

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
            let key = secrets::get(SecretKey::GeminiLlm)?
                .ok_or_else(|| anyhow!("no Gemini API key — open Settings → Provider & Keys"))?;
            // If user has gemini selected but no model set, fall back to default.
            let m = if model.starts_with("gemini") {
                model
            } else {
                crate::llm::gemini::DEFAULT_MODEL.to_string()
            };
            Ok(Box::new(GeminiLlm::new(key, m)))
        }
        _ => {
            // "groq" or anything unknown -> Groq path.
            let key = secrets::get(SecretKey::GroqLlm)?
                .or_else(|| secrets::get(SecretKey::GroqStt).ok().flatten())
                .ok_or_else(|| anyhow!("no Groq LLM key — open Settings → Provider & Keys"))?;
            Ok(Box::new(GroqLlm::new(key, model)))
        }
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
}

#[derive(Clone)]
pub struct Flow {
    state: Arc<Mutex<FlowState>>,
    history: History,
    settings: Arc<Mutex<AppSettings>>,
    audio_dir: PathBuf,
    audio: AudioController,
    usage: UsageTracker,
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
        }
    }

    pub fn settings(&self) -> AppSettings {
        self.settings.lock().clone()
    }

    pub fn set_settings(&self, s: AppSettings) {
        *self.settings.lock() = s;
    }

    pub fn handle_hotkey(&self, app: &AppHandle, evt: HotkeyEvent) {
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
                } else if let Err(e) = this.start_recording_async(&app, evt.mode).await {
                    tracing::error!("start_recording (sticky start) failed: {e:#}");
                    let _ = app.emit("wispr:flow_error", e.to_string());
                } else {
                    let _ = app.emit("wispr:mode", mode_str);
                    let _ = app.emit("wispr:state", "recording");
                }
                return;
            }

            // Push-to-talk: down starts, up finishes.
            match evt.edge {
                Edge::Down => {
                    if let Err(e) = this.start_recording_async(&app, evt.mode).await {
                        tracing::error!("start_recording failed: {e:#}");
                        let _ = app.emit("wispr:flow_error", e.to_string());
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

    async fn start_recording_async(&self, _app: &AppHandle, mode: Mode) -> Result<()> {
        {
            let state = self.state.lock();
            if state.active.is_some() {
                return Err(anyhow!("recording already in progress"));
            }
        }

        let date = Utc::now().format("%Y-%m-%d").to_string();
        let id_seed = uuid::Uuid::new_v4().to_string();
        let path = self
            .audio_dir
            .join(date)
            .join(format!("{id_seed}.wav"));

        self.audio
            .start(path.clone())
            .await
            .context("starting audio capture")?;
        crate::audio::cues::play_start();
        let record_id = self.history.insert_new(&path, ClippyMode::from(mode))?;

        let mut state = self.state.lock();
        state.active = Some(InFlight { mode, record_id, audio_path: path });
        Ok(())
    }

    async fn finish_recording_async(&self, app: &AppHandle) -> Result<()> {
        let in_flight = {
            let mut state = self.state.lock();
            state.active.take()
        };
        let Some(InFlight { mode, record_id, audio_path: _ }) = in_flight else {
            return Ok(());
        };

        let _ = app.emit("wispr:state", "transcribing");

        let FinishedRecording { path, duration_ms } = self.audio.stop().await?;
        crate::audio::cues::play_stop();
        self.history.set_duration(&record_id, duration_ms)?;

        // Trim trailing silence before sending to Whisper — prevents
        // hallucinations like "thank you" / "gracias" on silent tails.
        if let Err(e) = crate::audio::trim_trailing_silence(&path, 500, 300) {
            tracing::warn!("silence trimming failed (non-fatal): {e:#}");
        }

        if duration_ms < MIN_DURATION_MS {
            tracing::info!(record_id, duration_ms, "discarding too-short recording");
            self.history
                .set_error(&record_id, "recording too short")?;
            let _ = std::fs::remove_file(&path);
            let _ = app.emit("wispr:state", "idle");
            return Ok(());
        }

        self.history.update_status(&record_id, Status::Transcribing)?;

        let stt_key = secrets::get(SecretKey::GroqStt)?
            .ok_or_else(|| anyhow!("no Groq STT key — open Settings to add one"))?;
        let stt_settings = self.settings();
        let stt = GroqStt::with_model(stt_key, stt_settings.stt_model.clone());

        let wav_size = tokio::fs::metadata(&path).await.map(|m| m.len()).unwrap_or(0);
        tracing::info!(
            record_id,
            wav_bytes = wav_size,
            "sending WAV to Whisper"
        );

        self.usage.record_stt();
        let transcript = stt
            .transcribe(&path, stt_settings.language_hint.as_deref())
            .await
            .context("Groq Whisper request")?;

        tracing::info!(
            record_id,
            text = %transcript.text,
            language = ?transcript.language,
            duration_secs = ?transcript.duration_seconds,
            "Whisper response"
        );

        self.history
            .set_transcript(&record_id, &transcript.text, stt.name())?;

        let clippy_settings = self.settings();
        let needs_clippy = match mode {
            Mode::Light => clippy_settings.auto_clean_in_light,
            Mode::Advanced => clippy_settings.auto_clean_in_advanced,
            Mode::Drafting => clippy_settings.auto_clean_in_drafting,
        };

        let final_text = if needs_clippy {
            let _ = app.emit("wispr:state", "cleaning");
            self.history.update_status(&record_id, Status::Cleaning)?;

            let provider_id = match mode {
                Mode::Light => clippy_settings.light_provider.clone(),
                Mode::Advanced => clippy_settings.advanced_provider.clone(),
                Mode::Drafting => clippy_settings.drafting_provider.clone(),
            };
            let model = match mode {
                Mode::Light => clippy_settings.clippy_light_model.clone(),
                Mode::Advanced => clippy_settings.clippy_advanced_model.clone(),
                Mode::Drafting => clippy_settings.clippy_drafting_model.clone(),
            };

            let llm: Box<dyn LlmProvider> = build_llm_provider(&provider_id, model)?;
            self.usage.record_llm();
            let cleaned = clippy::clean(&transcript.text, ClippyMode::from(mode), llm.as_ref()).await;
            self.history.set_cleaned(
                &record_id,
                &cleaned.text,
                Some(llm.name()),
                cleaned.used_clippy,
                cleaned.note,
            )?;
            cleaned.text
        } else {
            transcript.text.clone()
        };

        let _ = app.emit("wispr:state", "injecting");
        self.history.update_status(&record_id, Status::Injecting)?;
        match inject::inject(&final_text) {
            Ok(channel) => {
                tracing::info!(?channel, chars = final_text.chars().count(), "injected");
                self.history.update_status(&record_id, Status::Done)?;
            }
            Err(e) => {
                tracing::warn!("injection failed: {e:#}");
                self.history.set_error(&record_id, &format!("injection: {e}"))?;
            }
        }

        let _ = app.emit("wispr:state", "idle");
        Ok(())
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
        self.history.update_status(record_id, Status::Transcribing)?;
        let _ = app.emit("wispr:state", "transcribing");

        let stt_settings = self.settings();
        let stt_key = secrets::get(SecretKey::GroqStt)?
            .ok_or_else(|| anyhow!("no Groq STT key — open Settings to add one"))?;
        let stt = GroqStt::with_model(stt_key, stt_settings.stt_model.clone());

        self.usage.record_stt();
        let transcript = stt
            .transcribe(&rec.audio_path, stt_settings.language_hint.as_deref())
            .await
            .context("Groq Whisper retry")?;
        self.history
            .set_transcript(record_id, &transcript.text, stt.name())?;

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
            let provider_id = match mode {
                Mode::Light => stt_settings.light_provider.clone(),
                Mode::Advanced => stt_settings.advanced_provider.clone(),
                Mode::Drafting => stt_settings.drafting_provider.clone(),
            };
            let model = match mode {
                Mode::Light => stt_settings.clippy_light_model.clone(),
                Mode::Advanced => stt_settings.clippy_advanced_model.clone(),
                Mode::Drafting => stt_settings.clippy_drafting_model.clone(),
            };
            let llm: Box<dyn LlmProvider> = build_llm_provider(&provider_id, model)?;
            self.usage.record_llm();
            let cleaned = clippy::clean(&transcript.text, ClippyMode::from(mode), llm.as_ref()).await;
            self.history.set_cleaned(
                record_id,
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
        self.history.update_status(record_id, Status::Done)?;
        let _ = app.emit("wispr:state", "idle");
        Ok(())
    }
}
