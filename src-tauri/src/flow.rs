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
use crate::llm::{groq::GroqLlm, ClippyMode, LlmProvider};
use crate::secrets::{self, SecretKey};
use crate::settings::{AppSettings, Mode};
use crate::stt::{groq::GroqStt, SttProvider};

const MIN_DURATION_MS: i64 = 300;

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
}

impl Flow {
    pub fn new(
        history: History,
        settings: AppSettings,
        audio_dir: PathBuf,
        audio: AudioController,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(FlowState::default())),
            history,
            settings: Arc::new(Mutex::new(settings)),
            audio_dir,
            audio,
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
            match evt.edge {
                Edge::Down => {
                    if let Err(e) = this.start_recording_async(&app, evt.mode).await {
                        tracing::error!("start_recording failed: {e:#}");
                        let _ = app.emit("wispr:flow_error", e.to_string());
                    } else {
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
        let needs_clippy = matches!(mode, Mode::Advanced)
            || (matches!(mode, Mode::Light) && clippy_settings.auto_clean_in_light);

        let final_text = if needs_clippy {
            let _ = app.emit("wispr:state", "cleaning");
            self.history.update_status(&record_id, Status::Cleaning)?;
            let llm_key = secrets::get(SecretKey::GroqLlm)?
                .or_else(|| secrets::get(SecretKey::GroqStt).ok().flatten())
                .ok_or_else(|| anyhow!("no Groq LLM key — open Settings to add one"))?;
            let model = match mode {
                Mode::Light => clippy_settings.clippy_light_model.clone(),
                Mode::Advanced => clippy_settings.clippy_advanced_model.clone(),
            };
            let llm = GroqLlm::new(llm_key, model);
            let cleaned = clippy::clean(&transcript.text, ClippyMode::from(mode), &llm).await;
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
}
