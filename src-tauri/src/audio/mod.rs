//! Microphone capture via cpal, streaming a WAV file to disk.
//!
//! cpal's `Stream` is `!Send` on Windows (WASAPI/COM thread affinity), so we
//! park it on a dedicated audio worker thread and expose a Send/Sync handle
//! (`AudioController`) that the Flow layer calls into.
//!
//! **Warm-paused mic:** WASAPI shared-mode initialization (esp. Realtek HD
//! with audio enhancements, or Bluetooth) can take 3–6 seconds on the very
//! first `stream.play()`. To avoid that latency on every F8 press, we:
//!   1. Build + play the cpal stream at app startup (warmup happens here)
//!   2. Immediately pause it — stream stays alive, mic samples stop flowing
//!   3. On F8 press: `stream.play()` — resumes in <50ms (already warm)
//!   4. On F8 release: `stream.pause()` — samples stop, stream stays warm
//!
//! Result: first dictation may have warmup-cost during onboarding, every
//! subsequent press is near-instant. Other apps (Teams, Zoom) can still
//! read the mic in WASAPI shared mode — only the "mic in use" indicator
//! stays lit because our stream object is alive.

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

struct ActiveRecording {
    path: PathBuf,
    started_at: Instant,
}

fn worker_loop(rx: mpsc::Receiver<AudioCmd>) {
    // ── 1. Initialise audio device (COM + WASAPI enumeration) ──────────────
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
    let device_name = device.name().unwrap_or_else(|_| "<unnamed>".into());
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
    let sample_format = config.sample_format();
    let dev_init_ms = t0.elapsed().as_millis();
    tracing::info!(
        device = %device_name,
        sample_rate,
        channels,
        ?sample_format,
        dev_init_ms,
        "audio device enumerated"
    );

    // Writer gate: Some(wav) = recording, None = idle (samples discarded).
    let writer: SharedWriter = Arc::new(Mutex::new(None));

    let err_fn = |e: cpal::StreamError| {
        tracing::error!("audio stream error: {e}");
    };

    // ── 2. Build stream once ──────────────────────────────────────────────
    // Force a small buffer size (~10ms) to reduce WASAPI warm-up latency.
    // Default cpal buffer can be 100ms+ on Realtek which compounds startup delay.
    let mut stream_config: cpal::StreamConfig = config.clone().into();
    stream_config.buffer_size = cpal::BufferSize::Fixed(sample_rate / 100); // 10ms

    let t_build = Instant::now();
    let stream_result: Result<Stream> = match sample_format {
        SampleFormat::F32 => build_stream::<f32>(&device, &stream_config, writer.clone(), channels, err_fn),
        SampleFormat::I16 => build_stream::<i16>(&device, &stream_config, writer.clone(), channels, err_fn),
        SampleFormat::U16 => build_stream::<u16>(&device, &stream_config, writer.clone(), channels, err_fn),
        other => {
            tracing::error!("unsupported sample format: {other:?}");
            drain_errors(rx, "unsupported format");
            return;
        }
    };
    let stream = match stream_result {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("building input stream: {e:#}");
            drain_errors(rx, "build_input_stream failed");
            return;
        }
    };
    let build_ms = t_build.elapsed().as_millis();

    // ── 3. Start the always-hot stream ─────────────────────────────────────
    // Empirical: WASAPI shared-mode on Realtek STOPS the audio engine on
    // `pause()`. A subsequent `play()` triggers a 3-5s warm-up before samples
    // actually arrive. The only way to get instant F8 capture is to keep the
    // stream continuously playing — samples are discarded in-callback when
    // not recording (no disk, no buffer, no network). Other apps can still
    // read the mic in shared mode; the only cost is the "mic in use" indicator.
    let t_play = Instant::now();
    if let Err(e) = stream.play() {
        tracing::error!("stream.play() failed: {e}");
        drain_errors(rx, "stream play failed");
        return;
    }
    let play_ms = t_play.elapsed().as_millis();

    let total_init_ms = t0.elapsed().as_millis();
    tracing::info!(
        dev_init_ms,
        build_ms,
        play_ms,
        total_init_ms,
        "audio stream live (always-hot — samples discarded until F8)"
    );

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

                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                let wav_writer = match File::create(&path)
                    .context("creating WAV")
                    .and_then(|f| WavWriter::new(BufWriter::new(f), spec).context("WAV header"))
                {
                    Ok(w) => w,
                    Err(e) => {
                        let _ = reply.send(Err(e));
                        continue;
                    }
                };

                // Open the gate — callback starts writing samples NOW.
                // Stream is already live, so there's no resume latency.
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

                // Close the gate — callback continues running on the live
                // stream but discards samples (samples never hit disk again).
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
    drop(stream);
}

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
    writer: SharedWriter,
    input_channels: u16,
    err_fn: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<Stream>
where
    T: cpal::SizedSample + ToI16Sample + Send + 'static,
{
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Instant;

    let first_callback = Arc::new(Mutex::new(None::<Instant>));
    let callback_count = Arc::new(AtomicU64::new(0));

    let fc = first_callback.clone();
    let cc = callback_count.clone();

    let stream = device
        .build_input_stream(
            config,
            move |data: &[T], _info| {
                // Diagnostic: log when the first callback fires and every 100 thereafter.
                let n = cc.fetch_add(1, Ordering::Relaxed);
                if n == 0 {
                    let mut slot = fc.lock();
                    if slot.is_none() {
                        *slot = Some(Instant::now());
                        tracing::info!(samples = data.len(), "FIRST audio callback fired");
                    }
                } else if n.is_multiple_of(200) {
                    // Check sample energy to see if audio is silent or live.
                    let mut max_abs: i32 = 0;
                    for &s in data.iter().take(64) {
                        let v = s.to_i16_sample().abs() as i32;
                        if v > max_abs { max_abs = v; }
                    }
                    tracing::debug!(
                        callbacks = n,
                        samples_per_cb = data.len(),
                        peak = max_abs,
                        "audio callback heartbeat"
                    );
                }

                // Fast path: if not recording, discard immediately.
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
