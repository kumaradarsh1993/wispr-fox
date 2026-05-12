//! WAV chunking for STT providers with per-file size limits.
//!
//! Groq's Whisper endpoint caps uploads at 25 MB. A ~5-minute recording at
//! the device's native sample rate (48 kHz mono i16 = 96 KB/s) hits ~28 MB
//! and gets rejected with "file too large". This module splits the WAV into
//! N near-equal chunks below a target ceiling so each one fits the limit;
//! the caller transcribes each chunk and concatenates the resulting text.
//!
//! Trade-off: cuts may fall mid-word. Whisper is robust to this in
//! practice — joining the chunk transcripts with a space produces clean
//! continuous output for normal speech. If quality regressions become an
//! issue, future work: snap split points to silences (analogous to
//! `audio::trim_trailing_silence`).
//!
//! All chunks are written next to the source WAV with a `.chunkN.wav`
//! suffix so the caller can identify + delete them after transcription.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use hound::{WavReader, WavWriter};

/// Target byte ceiling for each chunk. Set well below Groq's 25 MB hard
/// limit so WAV header overhead + any encoding-size drift can't push us
/// over. Empirically: at 48 kHz mono i16 this gives ~3 min 38 s per chunk.
pub const TARGET_CHUNK_BYTES: u64 = 20 * 1024 * 1024;

/// Split `src` into chunks each under `target_max_bytes`. If `src` is
/// already under the limit, returns a single-element vec pointing at `src`
/// itself (no copy, no temp file). Otherwise produces N temp files named
/// `<stem>.chunk{0..N-1}.wav` in the same directory as `src`.
///
/// Caller is responsible for deleting the chunk files when done. The vec's
/// first element equals `src` ONLY in the no-split case — callers can use
/// `chunks.len() > 1 || chunks[0] != src` to know whether cleanup is owed.
pub fn split_wav_if_needed(src: &Path, target_max_bytes: u64) -> Result<Vec<PathBuf>> {
    let meta = std::fs::metadata(src)
        .with_context(|| format!("stat {src:?}"))?;
    if meta.len() <= target_max_bytes {
        return Ok(vec![src.to_path_buf()]);
    }

    let reader = WavReader::open(src)
        .with_context(|| format!("opening WAV {src:?}"))?;
    let spec = reader.spec();

    // Bytes per audio frame: bits/sample → bytes/sample × channels.
    // hound `duration()` returns frame count (NOT sample count) for
    // multi-channel files; for mono they're equal.
    let bytes_per_frame = (spec.bits_per_sample as u64 / 8) * spec.channels as u64;
    if bytes_per_frame == 0 {
        anyhow::bail!("invalid WAV spec: {spec:?}");
    }

    // Read everything in. For typical dictation (< 10 min) this is at most
    // ~60 MB of i16 — comfortably in-memory. Streaming chunker is a future
    // optimisation if we ever support hour-long recordings.
    let samples: Vec<i16> = reader
        .into_samples::<i16>()
        .collect::<Result<Vec<_>, _>>()
        .context("reading WAV samples")?;

    let total_bytes = samples.len() as u64 * 2; // i16
    // Number of chunks needed so each is ≤ target_max_bytes. Ceiling div.
    let n_chunks = ((total_bytes + target_max_bytes - 1) / target_max_bytes) as usize;
    let n_chunks = n_chunks.max(2); // we know we're splitting; never collapse
    // Distribute samples as evenly as possible across chunks (ceiling).
    let chunk_samples = (samples.len() + n_chunks - 1) / n_chunks;

    let parent_dir = src.parent().unwrap_or_else(|| Path::new("."));
    let stem = src
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("clip")
        .to_owned();

    let mut chunks = Vec::with_capacity(n_chunks);
    for (i, slice) in samples.chunks(chunk_samples).enumerate() {
        let chunk_path = parent_dir.join(format!("{stem}.chunk{i}.wav"));
        let mut writer = WavWriter::create(&chunk_path, spec)
            .with_context(|| format!("creating chunk WAV {chunk_path:?}"))?;
        for &s in slice {
            writer.write_sample(s).context("writing chunk sample")?;
        }
        writer.finalize().context("finalising chunk WAV")?;
        chunks.push(chunk_path);
    }

    tracing::info!(
        src = ?src,
        original_bytes = total_bytes,
        n_chunks,
        chunk_samples,
        bytes_per_frame,
        "WAV split for STT (exceeds per-request limit)"
    );

    Ok(chunks)
}

/// Best-effort cleanup of chunk files. Errors are logged but not surfaced —
/// at worst we leak ~25 MB of temp WAVs into the audio dir, which the
/// retention sweeper will collect on its hourly tick.
pub fn cleanup_chunks(chunks: &[PathBuf], src: &Path) {
    for c in chunks {
        if c == src {
            continue; // no-split case, don't delete the original
        }
        if let Err(e) = std::fs::remove_file(c) {
            tracing::warn!(chunk = ?c, "chunk cleanup failed: {e:#}");
        }
    }
}
