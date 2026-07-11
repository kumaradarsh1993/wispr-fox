//! Microphone capture via cpal, streaming a WAV file to disk.
//!
//! cpal's `Stream` is `!Send` on Windows (WASAPI/COM thread affinity), so we
//! park it on a dedicated audio worker thread and expose a Send/Sync handle
//! (`AudioController`) that the Flow layer calls into.
//!
//! **Cold-start capture:** a fresh cpal stream is built on every press and
//! dropped on release (see `worker_loop`). The old "warm-paused" design —
//! stream built at startup and kept alive forever — was retired: it pinned
//! the "mic in use" indicator on, and a permanently-open WASAPI stream makes
//! Windows hold an "audio stream is currently in use" power request that
//! blocks system sleep. Cost per press: ~200ms build + driver warmup.

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, SupportedStreamConfig};
use hound::{SampleFormat as WavSampleFormat, WavSpec, WavWriter};
use parking_lot::Mutex;
use tokio::sync::oneshot;

pub mod cues;
pub mod devices;

type SharedWriter = Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>;

#[derive(Debug)]
pub struct FinishedRecording {
    pub path: PathBuf,
    /// Wall-clock time the recording was active (key-down → key-up). This is
    /// how long the USER spoke; it is NOT proof that this much audio actually
    /// landed in the WAV — see `captured_ms`.
    pub duration_ms: i64,
    /// Actual audio written to the WAV, derived from the real sample count
    /// (`samples_written / sample_rate`). If the mic stream dropped mid-
    /// recording, this is much less than `duration_ms` and the transcript will
    /// be truncated. The two numbers together are how we detect a dropped mic.
    pub captured_ms: i64,
    /// True if cpal reported a stream error at any point during the recording
    /// (device switch, format change, buffer disconnect). A strong signal that
    /// audio was lost even if `captured_ms` looks plausible.
    pub stream_errored: bool,
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
    /// Live input level (f32 bits, 0.0–1.0), written by the cpal callback
    /// while a recording is in flight and pinned to 0.0 otherwise. Read by
    /// the `wispr:level` emitter task that feeds the wave-bar avatar.
    level: Arc<AtomicU32>,
}

impl AudioController {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel::<AudioCmd>();
        let level = Arc::new(AtomicU32::new(0));
        let level_for_worker = level.clone();
        std::thread::Builder::new()
            .name("wispr-audio".into())
            .spawn(move || worker_loop(rx, level_for_worker))
            .expect("spawn audio thread");
        Self { tx, level }
    }

    /// Shared handle to the live mic level for the emitter task.
    pub fn level_handle(&self) -> Arc<AtomicU32> {
        self.level.clone()
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
    _stream: Stream,
    /// Device sample rate — needed at stop time to convert the sample count
    /// into milliseconds of captured audio.
    sample_rate: u32,
    /// Count of mono samples actually written to the WAV. Incremented in the
    /// capture callback on every successful write; the source of truth for how
    /// much audio we really got (vs. how long the timer ran).
    samples_written: Arc<AtomicU64>,
    /// Set by the cpal error callback if the stream faults mid-recording.
    stream_error: Arc<AtomicBool>,
}

fn worker_loop(rx: mpsc::Receiver<AudioCmd>, level: Arc<AtomicU32>) {
    // True cold-start: build a fresh cpal stream on every F8 press, drop it
    // on release. This handles device changes (user toggling Windows audio
    // settings, plugging headphones, etc.) without needing to restart the app.
    //
    // Mic indicator turns off between recordings (stream object is dropped).
    // Cost per recording: ~200ms build + cpal/WASAPI warmup (10-200ms on good
    // drivers, can be 1-5s on Realtek with audio enhancements enabled).

    let mut active: Option<ActiveRecording> = None;
    let writer: SharedWriter = Arc::new(Mutex::new(None));

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

                // Build a fresh stream + open WAV writer.
                match begin_cold_recording(&path, &writer, level.clone()) {
                    Ok((stream, sample_rate, samples_written, stream_error)) => {
                        let setup_ms = t0.elapsed().as_millis();
                        tracing::info!(?path, setup_ms, sample_rate, "recording started (cold)");
                        active = Some(ActiveRecording {
                            path: path.clone(),
                            started_at: Instant::now(),
                            _stream: stream,
                            sample_rate,
                            samples_written,
                            stream_error,
                        });
                        let _ = reply.send(Ok(()));
                    }
                    Err(e) => {
                        // Ensure gate is closed if begin_cold_recording partially set it.
                        *writer.lock() = None;
                        level.store(0, Ordering::Relaxed);
                        tracing::error!("begin_cold_recording failed: {e:#}");
                        let _ = reply.send(Err(e));
                    }
                }
            }

            AudioCmd::Stop { reply } => {
                let Some(rec) = active.take() else {
                    let _ = reply.send(Err(anyhow!("no recording in progress")));
                    continue;
                };

                let duration_ms = rec.started_at.elapsed().as_millis() as i64;

                // Tail drain. WASAPI hands us audio in buffered chunks, so at
                // (see below) — sleep first so the final callbacks land before
                // we read the sample count.
                // the instant the key is released the final ~tens-to-hundreds
                // of milliseconds of speech are still sitting in the OS capture
                // buffer, not yet delivered to our callback. Closing the writer
                // and dropping the stream right now discards them — that's the
                // long-standing "it ate my last word" bug. Keep the stream
                // alive a beat longer so those trailing callbacks land in the
                // WAV first, THEN finalise. (Any genuine silence captured here
                // gets removed by trim_trailing_silence downstream.)
                std::thread::sleep(std::time::Duration::from_millis(220));

                // Close the gate; then drop the stream so the mic indicator
                // turns off and the device is released to other apps.
                if let Some(w) = writer.lock().take() {
                    if let Err(e) = w.finalize() {
                        tracing::warn!("WAV finalize error: {e}");
                    }
                }
                let ActiveRecording {
                    path,
                    _stream,
                    sample_rate,
                    samples_written,
                    stream_error,
                    ..
                } = rec;
                drop(_stream);
                level.store(0, Ordering::Relaxed);

                // Convert the real sample count into milliseconds of audio. This
                // is the ground truth we compare against the wall-clock timer to
                // detect a mic that dropped mid-recording.
                let samples = samples_written.load(Ordering::Relaxed);
                let captured_ms = if sample_rate > 0 {
                    (samples as i64 * 1000) / sample_rate as i64
                } else {
                    0
                };
                let stream_errored = stream_error.load(Ordering::Relaxed);
                if stream_errored || captured_ms + 750 < duration_ms {
                    // 750ms slack absorbs the normal tail-drain rounding; beyond
                    // that, real audio went missing.
                    tracing::warn!(
                        duration_ms,
                        captured_ms,
                        stream_errored,
                        "capture gap detected — WAV holds less audio than the timer ran"
                    );
                }

                let _ = reply.send(Ok(FinishedRecording {
                    path,
                    duration_ms,
                    captured_ms,
                    stream_errored,
                }));
            }
        }
    }

    tracing::debug!("audio worker exiting");
}

/// Build cpal stream, query device fresh, play it. Returns the live stream.
#[allow(clippy::type_complexity)]
fn begin_cold_recording(
    out_path: &Path,
    writer: &SharedWriter,
    level: Arc<AtomicU32>,
) -> Result<(Stream, u32, Arc<AtomicU64>, Arc<AtomicBool>)> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow!("no default input device — check Windows mic privacy settings"))?;
    let device_name = device.name().unwrap_or_else(|_| "<unnamed>".into());
    let config: SupportedStreamConfig = device
        .default_input_config()
        .context("querying default input config")?;

    let sample_rate = config.sample_rate().0;
    let channels = config.channels();
    let sample_format = config.sample_format();

    // 10ms buffer for low latency.
    let mut stream_config: cpal::StreamConfig = config.clone().into();
    stream_config.buffer_size = cpal::BufferSize::Fixed(sample_rate / 100);

    // Open the WAV writer and slot it into the gate before play() so we don't
    // miss the very first samples.
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: WavSampleFormat::Int,
    };
    let file = File::create(out_path)
        .with_context(|| format!("creating WAV at {out_path:?}"))?;
    let wav_writer = WavWriter::new(BufWriter::new(file), spec)
        .context("writing WAV header")?;
    *writer.lock() = Some(wav_writer);

    let samples_written = Arc::new(AtomicU64::new(0));
    let stream_error = Arc::new(AtomicBool::new(false));

    // Mid-recording stream faults (device switch, format change, buffer
    // disconnect) used to be logged and forgotten — the callbacks would stop,
    // no more audio would be written, but the wall-clock timer kept running, so
    // a dropped mic produced a full-duration card with a truncated transcript.
    // Now we latch the error so the pipeline can WARN the user their recording
    // is incomplete instead of silently pasting a partial result.
    let stream_error_cb = stream_error.clone();
    let err_fn = move |e: cpal::StreamError| {
        stream_error_cb.store(true, Ordering::Relaxed);
        tracing::error!("audio stream error (recording may be truncated): {e}");
    };

    let stream = match sample_format {
        SampleFormat::F32 => build_stream::<f32>(&device, &stream_config, writer.clone(), channels, level, samples_written.clone(), err_fn),
        SampleFormat::I16 => build_stream::<i16>(&device, &stream_config, writer.clone(), channels, level, samples_written.clone(), err_fn),
        SampleFormat::U16 => build_stream::<u16>(&device, &stream_config, writer.clone(), channels, level, samples_written.clone(), err_fn),
        other => return Err(anyhow!("unsupported sample format: {other:?}")),
    }?;

    stream.play().with_context(|| format!("stream.play() on device '{device_name}'"))?;

    Ok((stream, sample_rate, samples_written, stream_error))
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

#[allow(clippy::too_many_arguments)]
fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    writer: SharedWriter,
    input_channels: u16,
    level: Arc<AtomicU32>,
    samples_written: Arc<AtomicU64>,
    err_fn: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<Stream>
where
    T: cpal::SizedSample + ToI16Sample + Send + 'static,
{
    use std::sync::atomic::AtomicU64;
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

                // Live level for the wave-bar avatar: RMS over this ~10ms
                // buffer, perceptually stretched (sqrt) so normal speech
                // spans roughly 0.2–0.9. Written only while the gate is
                // open; worker_loop pins it back to 0 on stop.
                if !data.is_empty() {
                    let mut sum_sq = 0.0f32;
                    for &s in data {
                        let v = s.to_i16_sample() as f32 / 32768.0;
                        sum_sq += v * v;
                    }
                    let rms = (sum_sq / data.len() as f32).sqrt();
                    let perceptual = (rms.sqrt() * 2.4).min(1.0);
                    level.store(perceptual.to_bits(), Ordering::Relaxed);
                }

                // Count mono samples ACTUALLY written (write_sample Ok). A write
                // that fails mid-recording used to be silently dropped; now a
                // divergence between this count and the wall-clock timer is what
                // exposes the loss. Tallied per-callback (one atomic add) rather
                // than per-sample to keep the real-time path cheap.
                let mut wrote: u64 = 0;
                if input_channels <= 1 {
                    for &s in data {
                        if wav.write_sample(s.to_i16_sample()).is_ok() {
                            wrote += 1;
                        }
                    }
                } else {
                    let n = input_channels as usize;
                    for frame in data.chunks_exact(n) {
                        let mut acc = 0i32;
                        for &s in frame {
                            acc += s.to_i16_sample() as i32;
                        }
                        let mono = (acc / n as i32) as i16;
                        if wav.write_sample(mono).is_ok() {
                            wrote += 1;
                        }
                    }
                }
                if wrote > 0 {
                    samples_written.fetch_add(wrote, Ordering::Relaxed);
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

    // Keep a generous 300ms pad after the last loud sample. Soft word
    // endings (consonants, trailing-off speech) sit below the loudness
    // threshold; a tight pad would clip them. 300ms keeps the ending intact
    // while still trimming long silent tails that make Whisper hallucinate.
    let buffer_samples = (sample_rate as usize * 300) / 1000;
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
