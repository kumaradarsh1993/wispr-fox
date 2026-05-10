//! Microphone capture via cpal, streaming a WAV file to disk.
//!
//! cpal's `Stream` is `!Send` on Windows (WASAPI/COM thread affinity), so we
//! park it on a dedicated audio worker thread and expose a Send/Sync handle
//! (`AudioController`) that the Flow layer calls into.
//!
//! **Always-hot mic:** The cpal stream starts at app launch and runs
//! continuously. Audio samples are discarded unless recording is active.
//! This eliminates WASAPI stream startup latency (3–6s on some Windows
//! setups) so recording begins on the very first syllable after F8.

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, SupportedStreamConfig};
use hound::{SampleFormat as WavSampleFormat, WavSpec, WavWriter};
use parking_lot::Mutex;
use tokio::sync::oneshot;

pub mod devices;

type SharedWriter = Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>;

#[derive(Debug)]
pub struct FinishedRecording {
    pub path: PathBuf,
    pub duration_ms: i64,
}

enum AudioCmd {
    Start {
        path: PathBuf,
        reply: oneshot::Sender<Result<()>>,
    },
    Stop {
        reply: oneshot::Sender<Result<FinishedRecording>>,
    },
}

#[derive(Clone)]
pub struct AudioController {
    tx: mpsc::Sender<AudioCmd>,
}

impl AudioController {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel::<AudioCmd>();
        std::thread::Builder::new()
            .name("wispr-audio".into())
            .spawn(move || worker_loop(rx))
            .expect("spawn audio thread");
        Self { tx }
    }

    pub async fn start(&self, path: PathBuf) -> Result<()> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(AudioCmd::Start { path, reply: reply_tx })
            .map_err(|_| anyhow!("audio worker thread is gone"))?;
        reply_rx
            .await
            .map_err(|_| anyhow!("audio worker dropped reply"))?
    }

    pub async fn stop(&self) -> Result<FinishedRecording> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(AudioCmd::Stop { reply: reply_tx })
            .map_err(|_| anyhow!("audio worker thread is gone"))?;
        reply_rx
            .await
            .map_err(|_| anyhow!("audio worker dropped reply"))?
    }
}

/// State shared between the always-running cpal callback and the worker loop.
/// When `writer` holds `Some(wav)`, samples are written. When `None`, they're
/// discarded — the mic stays hot but nothing hits disk.
struct RecordingGate {
    writer: SharedWriter,
    channels: u16,
}

struct ActiveRecording {
    path: PathBuf,
    started_at: Instant,
}

fn worker_loop(rx: mpsc::Receiver<AudioCmd>) {
    // ── Initialise audio device + always-hot stream ────────────────────────
    let t0 = Instant::now();
    let host = cpal::default_host();
    let device = match host.default_input_device() {
        Some(d) => d,
        None => {
            tracing::error!("no default input device — check Windows mic privacy settings");
            drain_errors(rx, "no input device");
            return;
        }
    };
    let config: SupportedStreamConfig = match device.default_input_config() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("querying input config: {e}");
            drain_errors(rx, "input config failed");
            return;
        }
    };

    let sample_rate = config.sample_rate().0;
    let channels = config.channels();

    // Shared writer gate — starts as None (not recording).
    let writer: SharedWriter = Arc::new(Mutex::new(None));
    let gate = RecordingGate { writer: writer.clone(), channels };

    let err_fn = |e: cpal::StreamError| {
        tracing::error!("audio stream error: {e}");
    };

    let _stream: Stream = match config.sample_format() {
        SampleFormat::F32 => build_stream::<f32>(&device, &config.into(), &gate, err_fn),
        SampleFormat::I16 => build_stream::<i16>(&device, &config.into(), &gate, err_fn),
        SampleFormat::U16 => build_stream::<u16>(&device, &config.into(), &gate, err_fn),
        other => {
            tracing::error!("unsupported sample format: {other:?}");
            drain_errors(rx, "unsupported format");
            return;
        }
    }.expect("building always-hot input stream");

    _stream.play().expect("starting always-hot stream");

    let init_ms = t0.elapsed().as_millis();
    tracing::info!(sample_rate, channels, init_ms, "audio device + hot stream ready");

    // WAV spec used for every recording file.
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: WavSampleFormat::Int,
    };

    // ── Command loop ───────────────────────────────────────────────────────
    let mut active: Option<ActiveRecording> = None;

    while let Ok(cmd) = rx.recv() {
        match cmd {
            AudioCmd::Start { path, reply } => {
                if active.is_some() {
                    tracing::debug!("ignoring duplicate start (key repeat)");
                    let _ = reply.send(Err(anyhow!("recording already in progress")));
                    continue;
                }

                let t0 = Instant::now();

                // Create the output WAV file and slot it into the gate.
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                let wav_writer = match File::create(&path)
                    .context("creating WAV")
                    .and_then(|f| {
                        WavWriter::new(BufWriter::new(f), spec)
                            .context("writing WAV header")
                    }) {
                    Ok(w) => w,
                    Err(e) => {
                        let _ = reply.send(Err(e));
                        continue;
                    }
                };

                // Flip the gate — samples start flowing to disk immediately.
                *writer.lock() = Some(wav_writer);
                active = Some(ActiveRecording {
                    path: path.clone(),
                    started_at: Instant::now(),
                });

                let setup_ms = t0.elapsed().as_millis();
                tracing::info!(?path, setup_ms, "recording started (gate opened)");
                let _ = reply.send(Ok(()));
            }

            AudioCmd::Stop { reply } => {
                let Some(rec) = active.take() else {
                    let _ = reply.send(Err(anyhow!("no recording in progress")));
                    continue;
                };

                let duration_ms = rec.started_at.elapsed().as_millis() as i64;

                // Close the gate — samples stop flowing to disk.
                if let Some(w) = writer.lock().take() {
                    if let Err(e) = w.finalize() {
                        tracing::warn!("WAV finalize error: {e}");
                    }
                }

                let _ = reply.send(Ok(FinishedRecording {
                    path: rec.path,
                    duration_ms,
                }));
            }
        }
    }

    tracing::debug!("audio worker exiting");
    drop(_stream);
}

/// When device init fails, drain the command channel replying with errors.
fn drain_errors(rx: mpsc::Receiver<AudioCmd>, reason: &str) {
    while let Ok(cmd) = rx.recv() {
        match cmd {
            AudioCmd::Start { reply, .. } => {
                let _ = reply.send(Err(anyhow!("audio unavailable: {reason}")));
            }
            AudioCmd::Stop { reply, .. } => {
                let _ = reply.send(Err(anyhow!("audio unavailable: {reason}")));
            }
        }
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    gate: &RecordingGate,
    err_fn: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<Stream>
where
    T: cpal::SizedSample + ToI16Sample + Send + 'static,
{
    let writer = gate.writer.clone();
    let input_channels = gate.channels;

    let stream = device
        .build_input_stream(
            config,
            move |data: &[T], _info| {
                // Fast path: if not recording, discard all samples.
                let mut guard = writer.lock();
                let Some(wav) = guard.as_mut() else { return };

                if input_channels <= 1 {
                    for &s in data {
                        let _ = wav.write_sample(s.to_i16_sample());
                    }
                } else {
                    let n = input_channels as usize;
                    for frame in data.chunks_exact(n) {
                        let mut acc = 0i32;
                        for &s in frame {
                            acc += s.to_i16_sample() as i32;
                        }
                        let mono = (acc / n as i32) as i16;
                        let _ = wav.write_sample(mono);
                    }
                }
            },
            err_fn,
            None,
        )
        .context("building cpal input stream")?;
    Ok(stream)
}

// ── Silence trimming (anti-hallucination) ──────────────────────────────────

/// Trim trailing silence from a recorded WAV file. Whisper hallucinates
/// ("thank you", "gracias", "merci") on silent tails — this removes them.
///
/// * `threshold` — amplitude below which a sample counts as silence (500 ≈ 1.5%)
/// * `min_tail_ms` — only trim if the silent tail exceeds this many ms
pub fn trim_trailing_silence(path: &Path, threshold: i16, min_tail_ms: u32) -> Result<()> {
    let reader = hound::WavReader::open(path)
        .with_context(|| format!("opening WAV for silence trim: {path:?}"))?;
    let spec = reader.spec();
    let sample_rate = spec.sample_rate;

    let samples: Vec<i16> = reader
        .into_samples::<i16>()
        .filter_map(|s| s.ok())
        .collect();

    if samples.is_empty() {
        return Ok(());
    }

    let last_loud = samples.iter().rposition(|&s| s.abs() > threshold);
    let last_loud_idx = match last_loud {
        Some(i) => i,
        None => return Ok(()),
    };

    let buffer_samples = (sample_rate as usize * 150) / 1000;
    let trim_to = (last_loud_idx + buffer_samples).min(samples.len());

    let removed_samples = samples.len() - trim_to;
    let removed_ms = (removed_samples as u64 * 1000) / sample_rate as u64;
    if removed_ms < min_tail_ms as u64 {
        return Ok(());
    }

    tracing::info!(
        path = %path.display(),
        trimmed_ms = removed_ms,
        "trimmed trailing silence from WAV"
    );

    let mut writer = hound::WavWriter::create(path, spec)
        .with_context(|| "rewriting trimmed WAV")?;
    for &s in &samples[..trim_to] {
        writer.write_sample(s)?;
    }
    writer.finalize().context("finalising trimmed WAV")?;
    Ok(())
}

trait ToI16Sample: Copy {
    fn to_i16_sample(self) -> i16;
}

impl ToI16Sample for i16 {
    fn to_i16_sample(self) -> i16 { self }
}

impl ToI16Sample for u16 {
    fn to_i16_sample(self) -> i16 {
        (self as i32 - 32768) as i16
    }
}

impl ToI16Sample for f32 {
    fn to_i16_sample(self) -> i16 {
        let clamped = self.clamp(-1.0, 1.0);
        (clamped * 32767.0) as i16
    }
}
