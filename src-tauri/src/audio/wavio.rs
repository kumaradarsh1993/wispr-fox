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

/// Repair a WAV whose header sizes were never written, in place.
///
/// **Why this exists.** A WAV states its own length in two places — the `RIFF`
/// chunk size near byte 4 and the `data` chunk size just before the audio — and
/// both can only be filled in when recording STOPS. If the app is killed
/// mid-recording (crash, forced quit, or an update that replaces the running
/// app — which is exactly how a 13.5-minute session was nearly lost on
/// 16-Sep-2026), those fields stay zero while the real audio sits right behind
/// them. Every decoder trusts the header, so the file reads as "contains no
/// audio" and the recording looks destroyed when in fact only 8 bytes are wrong.
///
/// This rewrites those 8 bytes from the file's true size on disk. Nothing else
/// in the file is touched and no samples are re-encoded. It runs from
/// `read_mono_f32`, so every consumer — playback, chunked upload, the STT
/// shrink, a Rerun from History — heals the file on first touch instead of
/// needing a separate recovery pass.
///
/// Returns `Ok(true)` when a repair was written. Fail-open: a header that looks
/// fine, an unreadable or read-only file, and an exotic layout all return
/// `Ok(false)` / `Err` and leave the caller to proceed as before.
pub fn repair_truncated_header(path: &Path) -> Result<bool> {
    use std::io::{Read, Seek, SeekFrom, Write};

    let file_len = std::fs::metadata(path)?.len();
    // Smallest plausible WAV: 12-byte RIFF/WAVE + 24-byte fmt + 8-byte data
    // header. Anything shorter has no audio to rescue.
    if file_len < 46 {
        return Ok(false);
    }

    let mut f = std::fs::OpenOptions::new().read(true).open(path)?;
    let mut head = [0u8; 12];
    f.read_exact(&mut head)?;
    if &head[0..4] != b"RIFF" || &head[8..12] != b"WAVE" {
        return Ok(false); // not a RIFF/WAVE file (or it is an exotic RF64)
    }
    let riff_size = u32::from_le_bytes([head[4], head[5], head[6], head[7]]);

    // Walk the chunk list looking for `data`. Chunks are id(4) + size(4) + body,
    // body padded to an even length.
    let mut cursor: u64 = 12;
    let mut data_size_at: Option<u64> = None;
    let mut data_body_at: u64 = 0;
    let mut data_size: u32 = 0;
    while cursor + 8 <= file_len {
        f.seek(SeekFrom::Start(cursor))?;
        let mut ch = [0u8; 8];
        f.read_exact(&mut ch)?;
        let id = [ch[0], ch[1], ch[2], ch[3]];
        let size = u32::from_le_bytes([ch[4], ch[5], ch[6], ch[7]]);
        if &id == b"data" {
            data_size_at = Some(cursor + 4);
            data_body_at = cursor + 8;
            data_size = size;
            break;
        }
        if size == 0 {
            // A zero-length non-data chunk means the header was mangled beyond
            // what we can safely walk; don't guess.
            return Ok(false);
        }
        cursor += 8 + size as u64 + (size as u64 % 2);
    }

    let Some(size_at) = data_size_at else {
        return Ok(false);
    };

    let true_data = file_len - data_body_at;
    let true_riff = file_len - 8;
    // Only step in when the declared size is missing or overruns the file. A
    // header that is merely SMALLER than the file is legitimate (trailing
    // LIST/id3 metadata chunks after the audio), so leave it alone.
    let data_broken = data_size == 0 || data_body_at + data_size as u64 > file_len;
    let riff_broken = riff_size == 0 || riff_size as u64 > true_riff;
    if !data_broken && !riff_broken {
        return Ok(false);
    }
    // Need at least one 16-bit frame of real audio to be worth repairing.
    if true_data < 2 {
        return Ok(false);
    }

    drop(f);
    let mut w = std::fs::OpenOptions::new().read(true).write(true).open(path)?;
    if data_broken {
        let clamped = u32::try_from(true_data).unwrap_or(u32::MAX);
        w.seek(SeekFrom::Start(size_at))?;
        w.write_all(&clamped.to_le_bytes())?;
    }
    if riff_broken {
        let clamped = u32::try_from(true_riff).unwrap_or(u32::MAX);
        w.seek(SeekFrom::Start(4))?;
        w.write_all(&clamped.to_le_bytes())?;
    }
    w.flush()?;

    tracing::warn!(
        ?path,
        file_len,
        declared_data = data_size,
        repaired_data = true_data,
        "WAV header had no length (recording was interrupted) — repaired in place"
    );
    Ok(true)
}

/// Read any WAV we can decode into mono f32. Errors only on a genuinely
/// unreadable file or an exotic bit depth (e.g. 8-bit, which no capture device
/// in this workflow produces).
pub fn read_mono_f32(path: &Path) -> Result<DecodedWav> {
    // Heal an interrupted recording's zeroed header before any decoder sees it
    // (see repair_truncated_header). Best-effort: a failure here just means the
    // open below reports the original problem.
    if let Err(e) = repair_truncated_header(path) {
        tracing::debug!(?path, error = %e, "WAV header repair check failed; decoding as-is");
    }
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
/// Downsample a recording to a 16 kHz mono 16-bit SIDE FILE for the STT
/// upload, leaving the original untouched for playback and re-diagnosis.
///
/// Why this exists: live dictations upload the mic's NATIVE rate. On Macs
/// that is 48 kHz — 96 KB per second of speech — so the upload is 3x the
/// bytes the provider needs (every Whisper endpoint resamples to 16 kHz
/// server-side; this is also what `canonicalize_in_place` relies on for
/// dragged-in files, where the ~9x shrink comment was measured). Worse,
/// past ~3.5 minutes the 48 kHz file crosses `TARGET_CHUNK_BYTES` and gets
/// split into chunks that upload SEQUENTIALLY, each paying its own
/// round-trip. Measured on an M-series MacBook on a 171 Mbps uplink, STT
/// wall-time tracked upload size almost perfectly (~49 s for a 21-minute
/// dictation). Shrinking to 16 kHz cuts the bytes 3x and keeps recordings
/// under the chunk threshold up to ~10.5 minutes.
///
/// Returns `None` when the file is already at or below 16 kHz (nothing to
/// gain — e.g. Windows array mics that capture at 16 kHz). Fail-open by
/// design: callers treat any error as "send the original".
pub fn shrink_for_stt(path: &Path) -> Result<Option<PathBuf>> {
    let spec = hound::WavReader::open(path)
        .with_context(|| format!("opening WAV {path:?}"))?
        .spec();
    if spec.sample_rate <= STT_SAMPLE_RATE {
        return Ok(None);
    }

    let decoded = read_mono_f32(path)?;
    anyhow::ensure!(!decoded.samples.is_empty(), "WAV contains no audio");
    let samples = resample_linear(&decoded.samples, decoded.sample_rate, STT_SAMPLE_RATE);

    let out = tagged_path(path, "stt16k");
    write_mono_i16(&out, &samples, STT_SAMPLE_RATE)?;
    Ok(Some(out))
}

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

    /// A recording interrupted mid-capture (app killed, crash, in-place update)
    /// keeps its audio but never gets its RIFF/data lengths written, so every
    /// decoder calls it empty. Reading it must repair those 8 bytes and return
    /// the audio — this is the 16-Sep-2026 "contains no audio" incident, where
    /// 13.5 minutes of speech were sitting behind a zeroed header.
    #[test]
    fn read_repairs_zeroed_header_from_interrupted_recording() {
        use std::io::{Seek, SeekFrom, Write};
        let path = tmp("zeroheader.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: WavSampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for i in 0..24_000 {
            w.write_sample(((i % 100) as i16) * 50).unwrap();
        }
        w.finalize().unwrap();

        // Simulate the kill: blank both length fields. The data chunk header of
        // a canonical 44-byte hound WAV starts at 36, so its size lives at 40.
        {
            let mut f = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
            f.seek(SeekFrom::Start(4)).unwrap();
            f.write_all(&0u32.to_le_bytes()).unwrap();
            f.seek(SeekFrom::Start(40)).unwrap();
            f.write_all(&0u32.to_le_bytes()).unwrap();
        }
        // Sanity: the file really is unreadable in that state.
        assert!(
            hound::WavReader::open(&path)
                .map(|r| r.len() == 0)
                .unwrap_or(true),
            "zeroed header should read as empty before repair"
        );

        let decoded = read_mono_f32(&path).expect("repair should make it readable");
        assert_eq!(decoded.sample_rate, 48_000);
        assert_eq!(decoded.samples.len(), 24_000);
        // Idempotent: a second read finds a healthy header and changes nothing.
        assert!(!repair_truncated_header(&path).unwrap());
        let _ = std::fs::remove_file(&path);
    }

    /// Trailing metadata chunks (LIST, id3) make the data size legitimately
    /// smaller than the file. That must NOT be "repaired" into swallowing them.
    #[test]
    fn repair_leaves_a_healthy_header_alone() {
        let path = tmp("healthy.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: WavSampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for i in 0..1_000 {
            w.write_sample((i % 50) as i16).unwrap();
        }
        w.finalize().unwrap();
        let before = std::fs::read(&path).unwrap();
        assert!(!repair_truncated_header(&path).unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let _ = std::fs::remove_file(&path);
    }

    /// A 48 kHz recording (every Mac mic) must shrink to a 16 kHz side file
    /// roughly a third the size, with the original left byte-identical.
    #[test]
    fn shrink_for_stt_downsamples_48k() {
        let path = tmp("shrink48k.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: WavSampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for i in 0..48_000 {
            w.write_sample(((i % 100) as i16) * 50).unwrap();
        }
        w.finalize().unwrap();
        let orig_bytes = std::fs::metadata(&path).unwrap().len();

        let out = shrink_for_stt(&path).unwrap().expect("48k must shrink");
        let out_spec = hound::WavReader::open(&out).unwrap().spec();
        assert_eq!(out_spec.sample_rate, STT_SAMPLE_RATE);
        assert_eq!(out_spec.channels, 1);
        let out_bytes = std::fs::metadata(&out).unwrap().len();
        assert!(out_bytes < orig_bytes / 2, "expected ~3x shrink, got {orig_bytes} -> {out_bytes}");
        // Original untouched.
        assert_eq!(std::fs::metadata(&path).unwrap().len(), orig_bytes);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&out);
    }

    /// Audio already at 16 kHz (Windows array mics) has nothing to gain —
    /// no side file, the caller sends the original.
    #[test]
    fn shrink_for_stt_skips_16k() {
        let path = tmp("shrink16k.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: WavSampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for _ in 0..16_000 {
            w.write_sample(1000i16).unwrap();
        }
        w.finalize().unwrap();
        assert!(shrink_for_stt(&path).unwrap().is_none());
        let _ = std::fs::remove_file(&path);
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
