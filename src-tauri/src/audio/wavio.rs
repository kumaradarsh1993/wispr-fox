//! Format-tolerant WAV reading, and the mono/16-bit normalisation every
//! downstream stage assumes.
//!
//! **Why this exists.** The rest of the app was written against the recorder's
//! own output — always 16-bit integer PCM, always mono — and several places
//! hard-coded that assumption with `into_samples::<i16>()`. That is fine for a
//! file we wrote ourselves and wrong for anything the *user* hands us:
//!
//!   * `stt/chunk.rs` collected with `?`, so a 24-bit or 32-bit-float upload
//!     over the 20 MB chunk threshold failed the whole transcription. External
//!     field recorders (DJI, Zoom, Tascam, Røde) record 24-bit by default —
//!     at 48 kHz mono that is 144 KB/s, i.e. every upload past ~2.3 minutes.
//!   * `audio/denoise.rs` used `filter_map(|s| s.ok())`, which silently dropped
//!     *every* sample of a non-i16 file, hit the "empty WAV" guard, and fell
//!     back to raw — graceful, but noise reduction quietly did nothing.
//!
//! `read_mono_f32` handles 16/24/32-bit integer and 32/64-bit float, and
//! down-mixes multi-channel to mono, so those call sites stop caring.
//!
//! Amplitude convention: **-1.0 ..= 1.0**. hound reports integer samples in
//! their native width, so each is divided by that width's full scale.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use hound::{SampleFormat as WavSampleFormat, WavSpec, WavWriter};

/// Sample rate we transcode uploads down to. Every STT provider we support
/// resamples to 16 kHz internally (Whisper's native rate), so this is lossless
/// as far as the transcript is concerned while shrinking a 24-bit/48 kHz file
/// about 9x on the wire.
pub const STT_SAMPLE_RATE: u32 = 16_000;

pub struct DecodedWav {
    /// Mono samples in -1.0 ..= 1.0.
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    /// The source file's declared bit depth, for logging/diagnostics.
    pub bits_per_sample: u16,
    pub is_float: bool,
    pub channels: u16,
}

/// Read any WAV we can decode into mono f32. Errors only on a genuinely
/// unreadable file or an exotic bit depth (e.g. 8-bit, which no capture device
/// in this workflow produces).
pub fn read_mono_f32(path: &Path) -> Result<DecodedWav> {
    let reader = hound::WavReader::open(path)
        .with_context(|| format!("opening WAV {path:?}"))?;
    let spec = reader.spec();
    let channels = spec.channels.max(1);

    // Full-scale divisor for the declared bit depth. hound sign-extends
    // integer samples into i32, so 24-bit values arrive as ±8_388_608.
    let interleaved: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (WavSampleFormat::Float, _) => reader
            .into_samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .context("reading float WAV samples")?,
        (WavSampleFormat::Int, bits) => {
            anyhow::ensure!(
                (16..=32).contains(&bits),
                "unsupported WAV bit depth: {bits}-bit integer"
            );
            let scale = (1i64 << (bits - 1)) as f32;
            reader
                .into_samples::<i32>()
                .collect::<Result<Vec<_>, _>>()
                .context("reading integer WAV samples")?
                .into_iter()
                .map(|s| s as f32 / scale)
                .collect()
        }
    };

    let samples = if channels <= 1 {
        interleaved
    } else {
        let n = channels as usize;
        interleaved
            .chunks_exact(n)
            .map(|frame| frame.iter().sum::<f32>() / n as f32)
            .collect()
    };

    Ok(DecodedWav {
        samples,
        sample_rate: spec.sample_rate.max(1),
        bits_per_sample: spec.bits_per_sample,
        is_float: matches!(spec.sample_format, WavSampleFormat::Float),
        channels,
    })
}

/// Write mono f32 (-1.0..=1.0) as 16-bit integer PCM.
pub fn write_mono_i16(path: &Path, samples: &[f32], sample_rate: u32) -> Result<()> {
    let mut writer = WavWriter::create(
        path,
        WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: WavSampleFormat::Int,
        },
    )
    .with_context(|| format!("creating WAV at {path:?}"))?;
    for &s in samples {
        // Clamp before scaling: a float WAV can legitimately exceed ±1.0
        // (that is the entire point of 32-bit float capture), and wrapping
        // those into i16 would produce a burst of white noise.
        let v = (s.clamp(-1.0, 1.0) * 32767.0).round();
        writer.write_sample(v as i16)?;
    }
    writer.finalize().context("finalising WAV")?;
    Ok(())
}

/// Linear-interpolation resampler. Adequate for feeding ASR — every provider
/// resamples again on their side anyway — and deliberately not hi-fi.
pub fn resample_linear(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() || from == 0 || to == 0 {
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

/// Sibling path with an inserted tag: `foo.wav` → `foo.<tag>.wav`.
pub fn tagged_path(src: &Path, tag: &str) -> PathBuf {
    let mut name = src
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "audio".into());
    name.push('.');
    name.push_str(tag);
    name.push_str(".wav");
    src.with_file_name(name)
}

/// Transcode a WAV in place to canonical 16 kHz mono 16-bit PCM, but only when
/// it isn't already 16-bit integer. Returns `Ok(true)` when a rewrite happened.
///
/// Used at UPLOAD INGEST time — the recorder's own files are already canonical
/// and skip out on the first check. Rewriting in place (via a temp sibling +
/// rename) keeps a single audio file per history row, so playback, the (i)
/// inspector, and the retention sweeper all keep working unchanged.
pub fn canonicalize_in_place(path: &Path) -> Result<bool> {
    // Cheap pre-check: read only the header before committing to a full decode.
    let spec = hound::WavReader::open(path)
        .with_context(|| format!("opening WAV {path:?}"))?
        .spec();
    if matches!(spec.sample_format, WavSampleFormat::Int)
        && spec.bits_per_sample == 16
        && spec.channels == 1
    {
        return Ok(false);
    }

    let decoded = read_mono_f32(path)?;
    anyhow::ensure!(!decoded.samples.is_empty(), "WAV contains no audio");

    let (samples, rate) = if decoded.sample_rate > STT_SAMPLE_RATE {
        (
            resample_linear(&decoded.samples, decoded.sample_rate, STT_SAMPLE_RATE),
            STT_SAMPLE_RATE,
        )
    } else {
        (decoded.samples, decoded.sample_rate)
    };

    let tmp = tagged_path(path, "canon");
    write_mono_i16(&tmp, &samples, rate)?;
    std::fs::rename(&tmp, path)
        .with_context(|| format!("replacing {path:?} with canonicalised audio"))?;

    tracing::info!(
        path = %path.display(),
        from_bits = decoded.bits_per_sample,
        from_float = decoded.is_float,
        from_channels = decoded.channels,
        from_rate = decoded.sample_rate,
        to_rate = rate,
        "upload transcoded to 16-bit mono PCM for transcription"
    );
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("wispr-wavio-{name}"))
    }

    /// The regression that motivated this module: a 24-bit file must decode to
    /// real audio, not to an empty vec (denoise) or an error (chunker).
    #[test]
    fn reads_24_bit_integer() {
        let path = tmp("24bit.wav");
        let spec = WavSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 24,
            sample_format: WavSampleFormat::Int,
        };
        let mut w = WavWriter::create(&path, spec).unwrap();
        // Half scale for 24-bit.
        for _ in 0..1000 {
            w.write_sample(4_194_304i32).unwrap();
        }
        w.finalize().unwrap();

        let d = read_mono_f32(&path).unwrap();
        assert_eq!(d.samples.len(), 1000);
        assert_eq!(d.bits_per_sample, 24);
        assert!(
            (d.samples[0] - 0.5).abs() < 1e-4,
            "24-bit half-scale should decode to ~0.5, got {}",
            d.samples[0]
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn downmixes_stereo_to_mono() {
        let path = tmp("stereo.wav");
        let spec = WavSpec {
            channels: 2,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: WavSampleFormat::Int,
        };
        let mut w = WavWriter::create(&path, spec).unwrap();
        for _ in 0..100 {
            w.write_sample(16_384i16).unwrap(); // L = +0.5
            w.write_sample(-16_384i16).unwrap(); // R = -0.5
        }
        w.finalize().unwrap();

        let d = read_mono_f32(&path).unwrap();
        assert_eq!(d.samples.len(), 100, "one mono sample per stereo frame");
        assert!(d.samples[0].abs() < 1e-3, "L+R should cancel to ~0");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn canonicalize_skips_already_canonical_files() {
        let path = tmp("canon-noop.wav");
        write_mono_i16(&path, &[0.1, -0.1, 0.2], 16_000).unwrap();
        assert!(!canonicalize_in_place(&path).unwrap(), "no rewrite expected");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn canonicalize_rewrites_float_and_downsamples() {
        let path = tmp("canon-float.wav");
        let spec = WavSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 32,
            sample_format: WavSampleFormat::Float,
        };
        let mut w = WavWriter::create(&path, spec).unwrap();
        for _ in 0..4800 {
            w.write_sample(0.25f32).unwrap();
        }
        w.finalize().unwrap();

        assert!(canonicalize_in_place(&path).unwrap(), "rewrite expected");
        let after = hound::WavReader::open(&path).unwrap().spec();
        assert_eq!(after.bits_per_sample, 16);
        assert_eq!(after.sample_rate, STT_SAMPLE_RATE);
        assert!(matches!(after.sample_format, WavSampleFormat::Int));
        let _ = std::fs::remove_file(&path);
    }

    /// 32-bit float can exceed ±1.0 by design; clamping must not wrap.
    #[test]
    fn write_clamps_out_of_range_float() {
        let path = tmp("clamp.wav");
        write_mono_i16(&path, &[4.0, -4.0], 16_000).unwrap();
        let d = read_mono_f32(&path).unwrap();
        assert!(d.samples[0] > 0.99 && d.samples[1] < -0.99);
        let _ = std::fs::remove_file(&path);
    }
}
