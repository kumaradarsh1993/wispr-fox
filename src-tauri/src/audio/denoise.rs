//! Offline noise reduction for recorded WAVs, applied between capture and STT.
//!
//! Two tiers, picked by the `noise_reduction` setting:
//!   • **On** — 4th-order Butterworth high-pass at 90 Hz. Laptop-fan rumble
//!     concentrates below ~300 Hz with a strong tonal peak around 70 Hz;
//!     speech information for ASR lives above 100 Hz, so this is a
//!     zero-distortion win (−9 dB at 70 Hz, −1.6 dB at 100 Hz, flat above
//!     150 Hz). Cost: ~10 multiply-adds per sample.
//!   • **Aggressive** — the high-pass PLUS an RNNoise pass (`nnnoiseless`,
//!     pure Rust). RNNoise is built for speech and runs far faster than
//!     realtime, but it operates at 48 kHz — other rates get linearly
//!     resampled up front (output stays at 48 kHz; STT doesn't care).
//!
//! The processed audio is written to a SIDE file — the raw recording on disk
//! is never modified, so history playback always has the untouched original
//! and a bad denoise can be diagnosed after the fact.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use hound::{SampleFormat as WavSampleFormat, WavSpec, WavWriter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseReduction {
    Off,
    /// High-pass only.
    On,
    /// High-pass + RNNoise.
    Aggressive,
}

impl NoiseReduction {
    /// Parse the settings string; anything unrecognised is Off (fail-open —
    /// a corrupt setting must never break dictation).
    pub fn parse(s: &str) -> Self {
        match s {
            "on" => Self::On,
            "aggressive" => Self::Aggressive,
            _ => Self::Off,
        }
    }
}

pub struct DenoiseOutcome {
    /// Path of the processed side file to feed to STT.
    pub path: PathBuf,
    pub elapsed_ms: i64,
}

/// Process `src` at the requested level into a sibling `.denoised.wav` file.
/// `src` itself is never touched. Returns Err on any failure — the caller
/// falls back to the raw recording (denoising must never lose a dictation).
pub fn denoise_to_side_file(src: &Path, level: NoiseReduction) -> Result<DenoiseOutcome> {
    let t0 = std::time::Instant::now();

    // Format-tolerant read. This used to be `into_samples::<i16>()` with a
    // `filter_map(ok)`, which meant a 24-bit or 32-bit-float file (any external
    // field recorder's default) decoded to ZERO samples, tripped the empty
    // guard below, and fell back to raw — so noise reduction silently did
    // nothing on exactly the recordings most likely to need it.
    //
    // The filters below work in i16-scale amplitude (RNNoise's native
    // convention), so scale up from the normalised -1.0..1.0 the reader gives.
    let decoded = super::wavio::read_mono_f32(src)?;
    let sample_rate = decoded.sample_rate;
    let samples: Vec<f32> = decoded.samples.iter().map(|s| s * 32768.0).collect();
    anyhow::ensure!(!samples.is_empty(), "empty WAV");

    // Stage 1 (both tiers): 90 Hz high-pass at the file's native rate.
    let mut audio = highpass_rumble(&samples, sample_rate);
    let mut out_rate = sample_rate;

    // Stage 2 (aggressive): RNNoise at 48 kHz.
    if level == NoiseReduction::Aggressive {
        if out_rate != 48_000 {
            audio = resample_linear(&audio, out_rate, 48_000);
            out_rate = 48_000;
        }
        audio = rnnoise(&audio);
    }

    let dst = side_path(src);
    let mut writer = WavWriter::create(
        &dst,
        WavSpec {
            channels: 1,
            sample_rate: out_rate,
            bits_per_sample: 16,
            sample_format: WavSampleFormat::Int,
        },
    )
    .with_context(|| format!("creating denoised WAV at {dst:?}"))?;
    for &s in &audio {
        writer.write_sample(s.clamp(-32768.0, 32767.0) as i16)?;
    }
    writer.finalize().context("finalising denoised WAV")?;

    Ok(DenoiseOutcome {
        path: dst,
        elapsed_ms: t0.elapsed().as_millis() as i64,
    })
}

/// Sibling path for the processed copy: `foo.wav` → `foo.denoised.wav`.
pub fn side_path(src: &Path) -> PathBuf {
    let mut name = src
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "recording".into());
    name.push_str(".denoised.wav");
    src.with_file_name(name)
}

/// 4th-order Butterworth high-pass at 90 Hz: two cascaded RBJ-cookbook
/// biquads with the canonical 4th-order section Qs (0.5412 / 1.3066),
/// coefficients derived for the actual sample rate. f64 state keeps the
/// recursion numerically clean over multi-minute recordings.
fn highpass_rumble(input: &[f32], sample_rate: u32) -> Vec<f32> {
    const CUTOFF_HZ: f64 = 90.0;
    const SECTION_QS: [f64; 2] = [0.541_196, 1.306_563];

    let fs = sample_rate.max(8000) as f64;
    let w0 = 2.0 * std::f64::consts::PI * CUTOFF_HZ / fs;
    let (sin_w0, cos_w0) = w0.sin_cos();

    let mut out: Vec<f32> = input.to_vec();
    for q in SECTION_QS {
        let alpha = sin_w0 / (2.0 * q);
        let b0 = (1.0 + cos_w0) / 2.0;
        let b1 = -(1.0 + cos_w0);
        let b2 = (1.0 + cos_w0) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;
        let (b0, b1, b2, a1, a2) = (b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0);

        let (mut x1, mut x2, mut y1, mut y2) = (0.0f64, 0.0, 0.0, 0.0);
        for s in out.iter_mut() {
            let x = *s as f64;
            let y = b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
            x2 = x1;
            x1 = x;
            y2 = y1;
            y1 = y;
            *s = y as f32;
        }
    }
    out
}

/// Linear-interpolation resampler. Good enough for feeding ASR (the common
/// cases are 44.1 kHz / 16 kHz mics being lifted to RNNoise's 48 kHz); not
/// meant for hi-fi playback.
fn resample_linear(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() {
        return input.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let out_len = ((input.len() as f64) / ratio).floor() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f64 * ratio;
        let idx = pos as usize;
        let frac = (pos - idx as f64) as f32;
        let a = input[idx];
        let b = input[(idx + 1).min(input.len() - 1)];
        out.push(a + (b - a) * frac);
    }
    out
}

/// RNNoise over the whole clip. Input/output are 48 kHz mono f32 in i16 range
/// (nnnoiseless's native convention — no rescaling needed on either side).
/// The model needs a few frames to converge, so the first frame is processed
/// twice and its first output discarded (standard nnnoiseless warm-up trick).
fn rnnoise(input: &[f32]) -> Vec<f32> {
    use nnnoiseless::DenoiseState;
    const FRAME: usize = DenoiseState::FRAME_SIZE;

    let mut state = DenoiseState::new();
    let mut out = Vec::with_capacity(input.len() + FRAME);
    let mut out_frame = [0.0f32; FRAME];

    // Warm-up: run the first frame through and throw the result away.
    let mut first = [0.0f32; FRAME];
    let n = input.len().min(FRAME);
    first[..n].copy_from_slice(&input[..n]);
    state.process_frame(&mut out_frame, &first);

    for chunk in input.chunks(FRAME) {
        if chunk.len() == FRAME {
            state.process_frame(&mut out_frame, chunk);
            out.extend_from_slice(&out_frame);
        } else {
            // Zero-pad the final partial frame, keep only the real samples.
            let mut padded = [0.0f32; FRAME];
            padded[..chunk.len()].copy_from_slice(chunk);
            state.process_frame(&mut out_frame, &padded);
            out.extend_from_slice(&out_frame[..chunk.len()]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic fan-noise test: 70 Hz rumble + 1 kHz "speech" tone. The
    /// high-pass must gut the rumble while leaving the tone intact.
    #[test]
    fn highpass_kills_rumble_keeps_speech() {
        let sr = 48_000u32;
        let n = sr as usize; // 1s
        let rumble: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * 70.0 * i as f32 / sr as f32).sin() * 8000.0)
            .collect();
        let speech: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr as f32).sin() * 8000.0)
            .collect();

        let rms = |v: &[f32]| (v.iter().map(|s| s * s).sum::<f32>() / v.len() as f32).sqrt();
        // Skip the first 100ms of output — filter transient.
        let tail = |v: Vec<f32>| v[sr as usize / 10..].to_vec();

        let rumble_out = tail(highpass_rumble(&rumble, sr));
        let speech_out = tail(highpass_rumble(&speech, sr));

        assert!(
            rms(&rumble_out) < rms(&rumble) * 0.42,
            "70 Hz rumble should drop by >7.5 dB, got {} -> {}",
            rms(&rumble),
            rms(&rumble_out)
        );
        assert!(
            rms(&speech_out) > rms(&speech) * 0.9,
            "1 kHz tone should pass nearly untouched, got {} -> {}",
            rms(&speech),
            rms(&speech_out)
        );
    }

    #[test]
    fn resample_preserves_duration() {
        let input = vec![0.5f32; 44_100];
        let out = resample_linear(&input, 44_100, 48_000);
        assert!((out.len() as i64 - 48_000).abs() <= 2, "got {}", out.len());
    }

    #[test]
    fn side_path_shape() {
        let p = side_path(Path::new("/tmp/2026-07-18/rec.wav"));
        assert_eq!(p, Path::new("/tmp/2026-07-18/rec.denoised.wav"));
    }
}
