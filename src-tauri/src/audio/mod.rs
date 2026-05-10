//! Microphone capture via cpal, streaming a WAV file to disk.
//!
//! cpal's `Stream` is `!Send` on Windows (WASAPI/COM thread affinity), so we
//! park it on a dedicated audio worker thread and expose a Send/Sync handle
//! (`AudioController`) that the Flow layer calls into. Commands flow over an
//! `mpsc::channel`; replies use a `oneshot`. The worker drops the stream on
//! `Stop`, finalises the WAV, and returns the duration.

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
    writer: SharedWriter,
    _stream: Stream,
}

fn worker_loop(rx: mpsc::Receiver<AudioCmd>) {
    let mut active: Option<ActiveRecording> = None;
    while let Ok(cmd) = rx.recv() {
        match cmd {
            AudioCmd::Start { path, reply } => {
                if active.is_some() {
                    let _ = reply.send(Err(anyhow!("recording already in progress")));
                    continue;
                }
                match begin_recording(path) {
                    Ok(rec) => {
                        active = Some(rec);
                        let _ = reply.send(Ok(()));
                    }
                    Err(e) => {
                        let _ = reply.send(Err(e));
                    }
                }
            }
            AudioCmd::Stop { reply } => {
                let Some(rec) = active.take() else {
                    let _ = reply.send(Err(anyhow!("no recording in progress")));
                    continue;
                };
                let _ = reply.send(end_recording(rec));
            }
        }
    }
    tracing::debug!("audio worker exiting");
}

fn begin_recording(out_path: PathBuf) -> Result<ActiveRecording> {
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating audio dir {parent:?}"))?;
    }

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow!("no default input device — check Windows mic privacy settings"))?;

    let config: SupportedStreamConfig = device
        .default_input_config()
        .context("querying default input config")?;

    let sample_rate = config.sample_rate().0;
    let channels = config.channels();

    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: WavSampleFormat::Int,
    };

    let file = File::create(&out_path)
        .with_context(|| format!("creating WAV at {out_path:?}"))?;
    let writer = WavWriter::new(BufWriter::new(file), spec)
        .with_context(|| "writing WAV header")?;
    let writer: SharedWriter = Arc::new(Mutex::new(Some(writer)));

    let err_writer = writer.clone();
    let err_fn = move |e: cpal::StreamError| {
        tracing::error!("audio stream error: {e}");
        let _ = err_writer.lock().take();
    };

    let stream = match config.sample_format() {
        SampleFormat::F32 => build_stream::<f32>(&device, &config.into(), writer.clone(), channels, err_fn)?,
        SampleFormat::I16 => build_stream::<i16>(&device, &config.into(), writer.clone(), channels, err_fn)?,
        SampleFormat::U16 => build_stream::<u16>(&device, &config.into(), writer.clone(), channels, err_fn)?,
        other => return Err(anyhow!("unsupported sample format: {other:?}")),
    };

    stream.play().context("starting cpal stream")?;
    tracing::info!(?out_path, sample_rate, channels, "recording started");

    Ok(ActiveRecording {
        path: out_path,
        started_at: Instant::now(),
        writer,
        _stream: stream,
    })
}

fn end_recording(rec: ActiveRecording) -> Result<FinishedRecording> {
    let duration_ms = rec.started_at.elapsed().as_millis() as i64;
    let ActiveRecording { path, writer, _stream, .. } = rec;
    drop(_stream);
    if let Some(w) = writer.lock().take() {
        w.finalize().context("finalising WAV header")?;
    }
    Ok(FinishedRecording { path, duration_ms })
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
    let stream = device
        .build_input_stream(
            config,
            move |data: &[T], _info| {
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
