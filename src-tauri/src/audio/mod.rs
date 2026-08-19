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
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering};
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
pub mod denoise;
pub mod devices;
pub mod level;
pub mod wavio;

type SharedWriter = Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureSource {
    Dictation,
    Preview,
}

impl CaptureSource {
    fn as_u8(self) -> u8 {
        match self {
            Self::Dictation => 1,
            Self::Preview => 2,
        }
    }

    fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Dictation),
            2 => Some(Self::Preview),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct MicReady {
    pub generation: u64,
    pub source: CaptureSource,
    pub ready_ms: i64,
}

/// Live capture telemetry, shared between the cpal callback (writer) and the
/// event-emitter task in `lib.rs` (reader). All lock-free — the callback runs
/// on a real-time audio thread and must never block.
///
/// `ready_ms` is the load-bearing one. The app used to behave as though
/// recording begins the instant the hotkey is pressed; with a Bluetooth mic
/// that is false by 2–10 seconds (the transmitter has to negotiate out of its
/// noise-cancellation mode, then the SCO link has to come up), and everything
/// spoken in that window is simply not in the WAV. The head-gap was already
/// measured — but only at *stop*, as a post-mortem. Publishing it live is what
/// lets the avatar say "hold on" until audio is genuinely flowing.
#[derive(Debug)]
pub struct Meter {
    /// Perceptually-stretched level (f32 bits, 0.0–1.0) for the wave skin.
    level: AtomicU32,
    /// True RMS of the last buffer (f32 bits, 0.0–1.0), un-stretched — the
    /// meter in Settings needs real dBFS, not a pretty curve.
    rms: AtomicU32,
    /// Peak absolute amplitude of the last buffer (f32 bits, 0.0–1.0).
    peak: AtomicU32,
    /// True while any capture stream — dictation or mic test — is live.
    active: AtomicBool,
    /// Milliseconds from the start command to the first audio callback.
    /// -1 while we are still waiting. Reset to -1 on every start/stop.
    ready_ms: AtomicI64,
    /// Monotonic capture identity. A readiness edge is meaningful only for the
    /// dictation/preview generation that armed the meter.
    generation: AtomicU64,
    source: std::sync::atomic::AtomicU8,
}

impl Meter {
    fn new() -> Self {
        Self {
            level: AtomicU32::new(0),
            rms: AtomicU32::new(0),
            peak: AtomicU32::new(0),
            active: AtomicBool::new(false),
            ready_ms: AtomicI64::new(-1),
            generation: AtomicU64::new(0),
            source: std::sync::atomic::AtomicU8::new(0),
        }
    }

    /// Called on start: arm the meter and clear the previous run's readings.
    fn arm(&self, generation: u64, source: CaptureSource) {
        self.ready_ms.store(-1, Ordering::Relaxed);
        self.level.store(0, Ordering::Relaxed);
        self.rms.store(0, Ordering::Relaxed);
        self.peak.store(0, Ordering::Relaxed);
        self.generation.store(generation, Ordering::Relaxed);
        self.source.store(source.as_u8(), Ordering::Relaxed);
        self.active.store(true, Ordering::Relaxed);
    }

    /// Called on stop: park everything back at rest.
    fn disarm(&self) {
        self.active.store(false, Ordering::Relaxed);
        self.level.store(0, Ordering::Relaxed);
        self.rms.store(0, Ordering::Relaxed);
        self.peak.store(0, Ordering::Relaxed);
        self.ready_ms.store(-1, Ordering::Relaxed);
    }

    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }
    pub fn rms(&self) -> f32 {
        f32::from_bits(self.rms.load(Ordering::Relaxed))
    }
    pub fn peak(&self) -> f32 {
        f32::from_bits(self.peak.load(Ordering::Relaxed))
    }
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }
    pub fn ready_event(&self) -> Option<MicReady> {
        let ready_ms = self.ready_ms.load(Ordering::Acquire);
        if ready_ms < 0 {
            return None;
        }
        Some(MicReady {
            generation: self.generation.load(Ordering::Relaxed),
            source: CaptureSource::from_u8(self.source.load(Ordering::Relaxed))?,
            ready_ms,
        })
    }
}

/// Absolute floor before a shortfall is worth mentioning at all. Below this it
/// is tail-drain rounding, not lost speech.
pub const CAPTURE_GAP_FLOOR_MS: i64 = 1_000;

/// ...and it must ALSO be this share of the recording. See `is_capture_gap`.
pub const CAPTURE_GAP_PERCENT: i64 = 3;

/// Did enough audio go missing to be worth interrupting the user about?
///
/// **The flat-threshold version of this was wrong, and it fired on healthy
/// recordings.** WASAPI sheds the occasional capture buffer whenever the audio
/// callback stalls — and this callback writes the WAV inline, so a disk hiccup
/// is enough. That loss scales with how long the stream ran. A real 5m39s
/// dictation lost 3.4s (1.0%), spread so thinly that the WAV contained no
/// silence, no splice, and transcribed cleanly end to end; a flat 1000ms of
/// slack called that "mic dropped mid-recording" and told the user to record it
/// all again.
///
/// A genuine drop — device unplugged, exclusive-mode steal, driver reset —
/// costs a large FRACTION of the take, not a rounding error. So the shortfall
/// must clear both an absolute floor and a share of the duration. A cpal stream
/// error is always reported regardless, because that is a direct signal rather
/// than an inference.
pub fn is_capture_gap(duration_ms: i64, captured_ms: i64, stream_errored: bool) -> bool {
    if stream_errored {
        return true;
    }
    let lost = duration_ms - captured_ms;
    lost > CAPTURE_GAP_FLOOR_MS && lost * 100 > duration_ms * CAPTURE_GAP_PERCENT
}

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
    /// Time from the Start command reaching the audio worker to the FIRST
    /// cpal callback delivering samples — i.e. how long the microphone took
    /// to actually wake up after the key went down. Words spoken inside this
    /// window are simply not in the WAV (head-gap). Healthy devices: well
    /// under 500ms. Windows "audio enhancements" / exclusive-mode arbitration
    /// can stretch it to 3–8s. -1 if no callback ever fired.
    pub mic_ready_ms: i64,
    /// Input device the capture actually ran on. This is the RESOLVED name,
    /// not the user's saved preference — if the chosen mic was unplugged or
    /// unpaired we silently fall back to the system default, and the flight
    /// recorder needs to be able to answer "why did it use the laptop mic?".
    pub device_name: String,
    /// True when the user had picked a specific device and we could not find
    /// it, so this recording used the system default instead.
    pub device_fallback: bool,
}

enum AudioCmd {
    Start {
        path: PathBuf,
        device: Option<String>,
        generation: u64,
        reply: oneshot::Sender<Result<()>>,
    },
    Stop {
        reply: oneshot::Sender<Result<FinishedRecording>>,
    },
    /// Open a capture stream that feeds the meter but writes no file — the
    /// Settings "test your mic" path. Independent of the dictation flow so a
    /// user can check their input without producing a history row.
    StartPreview {
        device: Option<String>,
        generation: u64,
        reply: oneshot::Sender<Result<String>>,
    },
    StopPreview {
        reply: oneshot::Sender<Result<()>>,
    },
}

#[derive(Clone)]
pub struct AudioController {
    tx: mpsc::Sender<AudioCmd>,
    meter: Arc<Meter>,
    next_generation: Arc<AtomicU64>,
}

impl AudioController {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel::<AudioCmd>();
        let meter = Arc::new(Meter::new());
        let meter_for_worker = meter.clone();
        std::thread::Builder::new()
            .name("wispr-audio".into())
            .spawn(move || worker_loop(rx, meter_for_worker))
            .expect("spawn audio thread");
        Self {
            tx,
            meter,
            next_generation: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Shared handle to the live capture telemetry for the emitter task.
    pub fn meter(&self) -> Arc<Meter> {
        self.meter.clone()
    }

    /// `device` is the user's saved input-device name, or `None` for "system
    /// default". A saved device that is no longer present falls back to the
    /// default rather than failing — a missing mic must never cost a dictation.
    pub fn reserve_generation(&self) -> u64 {
        self.next_generation.fetch_add(1, Ordering::Relaxed)
    }

    pub async fn start(
        &self,
        path: PathBuf,
        device: Option<String>,
        generation: u64,
    ) -> Result<()> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(AudioCmd::Start {
                path,
                device,
                generation,
                reply: reply_tx,
            })
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

    /// Start the mic-test preview. Returns the resolved device name.
    pub async fn start_preview(&self, device: Option<String>) -> Result<String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let generation = self.reserve_generation();
        self.tx
            .send(AudioCmd::StartPreview {
                device,
                generation,
                reply: reply_tx,
            })
            .map_err(|_| anyhow!("audio worker thread is gone"))?;
        reply_rx
            .await
            .map_err(|_| anyhow!("audio worker dropped reply"))?
    }

    pub async fn stop_preview(&self) -> Result<()> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(AudioCmd::StopPreview { reply: reply_tx })
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
    /// When the Start command was received — the anchor for `mic_ready_ms`.
    cmd_received_at: Instant,
    /// Stamped by the capture callback when the first buffer arrives.
    first_callback: Arc<Mutex<Option<Instant>>>,
    /// Resolved capture device, and whether we fell back to the default.
    device_name: String,
    device_fallback: bool,
}

fn worker_loop(rx: mpsc::Receiver<AudioCmd>, meter: Arc<Meter>) {
    // True cold-start: build a fresh cpal stream on every F8 press, drop it
    // on release. This handles device changes (user toggling Windows audio
    // settings, plugging headphones, etc.) without needing to restart the app.
    //
    // Mic indicator turns off between recordings (stream object is dropped).
    // Cost per recording: ~200ms build + cpal/WASAPI warmup (10-200ms on good
    // drivers, can be 1-5s on Realtek with audio enhancements enabled).

    let mut active: Option<ActiveRecording> = None;
    // The mic-test preview stream. Parked on this thread for the same reason
    // the recording stream is: cpal's `Stream` is `!Send` on Windows.
    let mut preview: Option<Stream> = None;
    let writer: SharedWriter = Arc::new(Mutex::new(None));

    while let Ok(cmd) = rx.recv() {
        match cmd {
            AudioCmd::Start {
                path,
                device,
                generation,
                reply,
            } => {
                if active.is_some() {
                    tracing::debug!("ignoring duplicate start (key repeat)");
                    let _ = reply.send(Err(anyhow!("recording already in progress")));
                    continue;
                }

                // A dictation always wins over a mic test. Dropping the preview
                // also releases the device, which matters on drivers that only
                // allow one capture client.
                if preview.take().is_some() {
                    tracing::debug!("dropping mic preview — dictation takes priority");
                }

                let t0 = Instant::now();

                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).ok();
                }

                meter.arm(generation, CaptureSource::Dictation);

                // Build a fresh stream + open WAV writer.
                match begin_cold_recording(&path, &writer, meter.clone(), device.as_deref(), t0) {
                    Ok(started) => {
                        let setup_ms = t0.elapsed().as_millis();
                        tracing::info!(
                            ?path,
                            setup_ms,
                            sample_rate = started.sample_rate,
                            device = %started.device_name,
                            fallback = started.device_fallback,
                            "recording started (cold)"
                        );
                        active = Some(ActiveRecording {
                            path: path.clone(),
                            started_at: Instant::now(),
                            _stream: started.stream,
                            sample_rate: started.sample_rate,
                            samples_written: started.samples_written,
                            stream_error: started.stream_error,
                            cmd_received_at: t0,
                            first_callback: started.first_callback,
                            device_name: started.device_name,
                            device_fallback: started.device_fallback,
                        });
                        let _ = reply.send(Ok(()));
                    }
                    Err(e) => {
                        // Ensure gate is closed if begin_cold_recording partially set it.
                        *writer.lock() = None;
                        meter.disarm();
                        tracing::error!("begin_cold_recording failed: {e:#}");
                        let _ = reply.send(Err(e));
                    }
                }
            }

            AudioCmd::StartPreview {
                device,
                generation,
                reply,
            } => {
                if active.is_some() {
                    let _ = reply.send(Err(anyhow!(
                        "can't test the mic while a recording is in progress"
                    )));
                    continue;
                }
                // Restarting the preview (e.g. the user picked a different
                // device) must drop the old stream first, or two clients fight
                // over the same device.
                preview = None;
                meter.arm(generation, CaptureSource::Preview);
                let t0 = Instant::now();
                match begin_preview(meter.clone(), device.as_deref(), t0) {
                    Ok((stream, name)) => {
                        tracing::info!(device = %name, "mic preview started");
                        preview = Some(stream);
                        let _ = reply.send(Ok(name));
                    }
                    Err(e) => {
                        meter.disarm();
                        tracing::warn!("mic preview failed: {e:#}");
                        let _ = reply.send(Err(e));
                    }
                }
            }

            AudioCmd::StopPreview { reply } => {
                let had = preview.take().is_some();
                // Only park the meter if a dictation isn't using it.
                if active.is_none() {
                    meter.disarm();
                }
                if had {
                    tracing::debug!("mic preview stopped");
                }
                let _ = reply.send(Ok(()));
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
                // Head-gap: how long the mic took to wake up after the key
                // went down. Anything the user said before the first callback
                // never reached the WAV.
                let mic_ready_ms = (*rec.first_callback.lock())
                    .map(|t| t.saturating_duration_since(rec.cmd_received_at).as_millis() as i64)
                    .unwrap_or(-1);

                let ActiveRecording {
                    path,
                    _stream,
                    sample_rate,
                    samples_written,
                    stream_error,
                    device_name,
                    device_fallback,
                    ..
                } = rec;
                drop(_stream);
                meter.disarm();

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
                if is_capture_gap(duration_ms, captured_ms, stream_errored) {
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
                    mic_ready_ms,
                    device_name,
                    device_fallback,
                }));
            }
        }
    }

    tracing::debug!("audio worker exiting");
}

/// Pick the capture device for this run.
///
/// `preferred` is the user's saved device name (`None` = system default). The
/// fallback is deliberate and load-bearing: the common case for an external mic
/// is that it is *switched off or unpaired* when the user presses the hotkey,
/// and a picker that hard-fails there would turn "I forgot to turn my mic on"
/// into "the app is broken and ate my dictation". So a missing device silently
/// drops to the system default, and we report that it happened so the UI can
/// say so afterwards.
///
/// Matching is exact-then-prefix. Windows sometimes decorates the enumerated
/// name (`"Headset (DJI MIC2 Hands-Free AG Audio)"`), and a saved name from an
/// earlier session can differ in that trailing decoration alone.
fn resolve_input_device(
    host: &cpal::Host,
    preferred: Option<&str>,
) -> Result<(cpal::Device, String, bool)> {
    let default = || {
        host.default_input_device().ok_or_else(|| {
            anyhow!("no default input device — check Windows mic privacy settings")
        })
    };

    let Some(want) = preferred.map(str::trim).filter(|s| !s.is_empty()) else {
        let dev = default()?;
        let name = dev.name().unwrap_or_else(|_| "<unnamed>".into());
        return Ok((dev, name, false));
    };

    if let Ok(devices) = host.input_devices() {
        let mut prefix_match: Option<(cpal::Device, String)> = None;
        for dev in devices {
            let Ok(name) = dev.name() else { continue };
            if name == want {
                return Ok((dev, name, false));
            }
            if prefix_match.is_none() && (name.starts_with(want) || want.starts_with(&name)) {
                prefix_match = Some((dev, name));
            }
        }
        if let Some((dev, name)) = prefix_match {
            tracing::info!(saved = want, resolved = %name, "input device matched by prefix");
            return Ok((dev, name, false));
        }
    }

    let dev = default()?;
    let name = dev.name().unwrap_or_else(|_| "<unnamed>".into());
    tracing::warn!(
        saved = want,
        fallback = %name,
        "saved input device not present — using system default for this recording"
    );
    Ok((dev, name, true))
}

struct StartedStream {
    stream: Stream,
    sample_rate: u32,
    samples_written: Arc<AtomicU64>,
    stream_error: Arc<AtomicBool>,
    first_callback: Arc<Mutex<Option<Instant>>>,
    device_name: String,
    device_fallback: bool,
}

/// Build cpal stream, query device fresh, play it. Returns the live stream.
fn begin_cold_recording(
    out_path: &Path,
    writer: &SharedWriter,
    meter: Arc<Meter>,
    preferred_device: Option<&str>,
    cmd_at: Instant,
) -> Result<StartedStream> {
    let host = cpal::default_host();
    let (device, device_name, device_fallback) = resolve_input_device(&host, preferred_device)?;
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
    let first_callback: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));

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

    let ctx = CaptureCtx {
        writer: Some(writer.clone()),
        channels,
        meter,
        samples_written: samples_written.clone(),
        first_callback: first_callback.clone(),
        cmd_at,
    };

    let stream = match sample_format {
        SampleFormat::F32 => build_stream::<f32>(&device, &stream_config, ctx, err_fn),
        SampleFormat::I16 => build_stream::<i16>(&device, &stream_config, ctx, err_fn),
        SampleFormat::U16 => build_stream::<u16>(&device, &stream_config, ctx, err_fn),
        other => return Err(anyhow!("unsupported sample format: {other:?}")),
    }?;

    stream.play().with_context(|| format!("stream.play() on device '{device_name}'"))?;

    Ok(StartedStream {
        stream,
        sample_rate,
        samples_written,
        stream_error,
        first_callback,
        device_name,
        device_fallback,
    })
}

/// Build a capture stream that feeds the meter only — no WAV, no history row.
///
/// This is the mic test. It matters more than it sounds: after a sleep/wake
/// cycle a Bluetooth transmitter can keep its "connected" LED and stay listed
/// by Windows while delivering **no audio at all**, recoverable only by
/// power-cycling the device. A device list can't see that; a live meter can,
/// and so can the head-gap number this stream also produces.
fn begin_preview(
    meter: Arc<Meter>,
    preferred_device: Option<&str>,
    cmd_at: Instant,
) -> Result<(Stream, String)> {
    let host = cpal::default_host();
    let (device, device_name, _fallback) = resolve_input_device(&host, preferred_device)?;
    let config: SupportedStreamConfig = device
        .default_input_config()
        .context("querying default input config")?;

    let sample_rate = config.sample_rate().0;
    let channels = config.channels();
    let sample_format = config.sample_format();
    let mut stream_config: cpal::StreamConfig = config.clone().into();
    stream_config.buffer_size = cpal::BufferSize::Fixed(sample_rate / 100);

    let err_fn = move |e: cpal::StreamError| {
        tracing::warn!("mic preview stream error: {e}");
    };

    let ctx = CaptureCtx {
        writer: None,
        channels,
        meter,
        samples_written: Arc::new(AtomicU64::new(0)),
        first_callback: Arc::new(Mutex::new(None)),
        cmd_at,
    };

    let stream = match sample_format {
        SampleFormat::F32 => build_stream::<f32>(&device, &stream_config, ctx, err_fn),
        SampleFormat::I16 => build_stream::<i16>(&device, &stream_config, ctx, err_fn),
        SampleFormat::U16 => build_stream::<u16>(&device, &stream_config, ctx, err_fn),
        other => return Err(anyhow!("unsupported sample format: {other:?}")),
    }?;

    stream
        .play()
        .with_context(|| format!("stream.play() on device '{device_name}'"))?;
    Ok((stream, device_name))
}

#[allow(dead_code)]
fn drain_errors(rx: mpsc::Receiver<AudioCmd>, reason: &str) {
    while let Ok(cmd) = rx.recv() {
        match cmd {
            AudioCmd::Start { reply, .. } => {
                let _ = reply.send(Err(anyhow!("audio unavailable: {reason}")));
            }
            AudioCmd::Stop { reply, .. } => {
                let _ = reply.send(Err(anyhow!("audio unavailable: {reason}")));
            }
            AudioCmd::StartPreview { reply, .. } => {
                let _ = reply.send(Err(anyhow!("audio unavailable: {reason}")));
            }
            AudioCmd::StopPreview { reply, .. } => {
                let _ = reply.send(Err(anyhow!("audio unavailable: {reason}")));
            }
        }
    }
}

/// Everything the capture callback needs, bundled so the two stream builders
/// (dictation and mic-test preview) share one code path. `writer: None` is the
/// preview case — meter only, nothing hits disk.
struct CaptureCtx {
    writer: Option<SharedWriter>,
    channels: u16,
    meter: Arc<Meter>,
    samples_written: Arc<AtomicU64>,
    first_callback: Arc<Mutex<Option<Instant>>>,
    /// Anchor for the head-gap measurement — when the start command was issued.
    cmd_at: Instant,
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    ctx: CaptureCtx,
    err_fn: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<Stream>
where
    T: cpal::SizedSample + ToI16Sample + Send + 'static,
{
    use std::sync::atomic::AtomicU64;

    let CaptureCtx {
        writer,
        channels: input_channels,
        meter,
        samples_written,
        first_callback,
        cmd_at,
    } = ctx;

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
                        // Publish the head-gap the moment it is known, not at
                        // stop. This is what the "hold on" avatar state waits
                        // for — until this fires, nothing the user says is
                        // reaching the WAV, and they deserve to see that.
                        let gap = cmd_at.elapsed().as_millis() as i64;
                        meter.ready_ms.store(gap, Ordering::Release);
                        tracing::info!(
                            samples = data.len(),
                            mic_ready_ms = gap,
                            "FIRST audio callback fired"
                        );
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

                // Meter first, and unconditionally — the preview has no writer,
                // and even during a dictation the level should reflect what the
                // mic is delivering. RMS is kept raw (for true dBFS in the mic
                // test) alongside the perceptually-stretched value the wave
                // skin has always used.
                if !data.is_empty() {
                    let mut sum_sq = 0.0f32;
                    let mut peak = 0.0f32;
                    for &s in data {
                        let v = s.to_i16_sample() as f32 / 32768.0;
                        sum_sq += v * v;
                        let a = v.abs();
                        if a > peak {
                            peak = a;
                        }
                    }
                    let rms = (sum_sq / data.len() as f32).sqrt();
                    let perceptual = (rms.sqrt() * 2.4).min(1.0);
                    meter.level.store(perceptual.to_bits(), Ordering::Relaxed);
                    meter.rms.store(rms.to_bits(), Ordering::Relaxed);
                    meter.peak.store(peak.to_bits(), Ordering::Relaxed);
                }

                // Preview stream: metering is the whole job, nothing to write.
                let Some(writer) = writer.as_ref() else { return };

                // Fast path: if not recording, discard immediately.
                let mut guard = writer.lock();
                let Some(wav) = guard.as_mut() else { return };

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

#[cfg(test)]
mod capture_gap_tests {
    use super::*;

    /// Real numbers from the flight recorder. These are HEALTHY recordings that
    /// the old flat-1000ms rule called "mic dropped mid-recording" — the 339s
    /// one produced a clean, coherent 3,981-character transcript with no
    /// silence and no splice anywhere in the WAV.
    #[test]
    fn thin_distributed_loss_on_a_long_take_is_not_a_drop() {
        for (duration_ms, captured_ms) in [(339_438, 336_268), (316_768, 310_100)] {
            let lost = duration_ms - captured_ms;
            assert!(
                lost > CAPTURE_GAP_FLOOR_MS,
                "this case must clear the old flat floor, or it proves nothing"
            );
            assert!(
                !is_capture_gap(duration_ms, captured_ms, false),
                "{lost}ms lost from {duration_ms}ms ({:.1}%) is buffer churn, not a dropped mic",
                100.0 * lost as f64 / duration_ms as f64
            );
        }
    }

    /// The normal case: the 220ms tail drain means captured EXCEEDS duration.
    #[test]
    fn the_healthy_baseline_never_warns() {
        assert!(!is_capture_gap(75_421, 75_630, false));
        assert!(!is_capture_gap(692_500, 692_710, false));
    }

    #[test]
    fn a_real_drop_still_warns() {
        // Mic yanked a third of the way through a 100s take.
        assert!(is_capture_gap(100_000, 66_000, false));
        // Short recording, proportionally large loss.
        assert!(is_capture_gap(20_000, 17_000, false));
    }

    /// A cpal stream error is a direct signal, not an inference — always report
    /// it, however small the shortfall looks.
    #[test]
    fn a_stream_error_always_warns() {
        assert!(is_capture_gap(339_438, 336_268, true));
        assert!(is_capture_gap(1_000, 1_200, true));
    }

    /// Sub-second shortfalls stay quiet even when proportionally large, so a
    /// very short take can't trip on rounding alone.
    #[test]
    fn rounding_on_a_tiny_take_stays_quiet() {
        assert!(!is_capture_gap(2_000, 1_400, false));
    }
}
