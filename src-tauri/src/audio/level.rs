//! Recording-level measurement and quiet-audio rescue.
//!
//! **The failure this exists to prevent.** A too-quiet recording does not fail
//! loudly. Whisper's silence/no-speech handling simply drops segments, and the
//! user gets back a plausible-looking transcript with whole phrases missing —
//! no error, no warning, nothing in the UI to suggest the result is incomplete.
//! Measured on a real DJI Mic 2 clip (24-bit, 48 kHz, RMS **-46.4 dBFS**, about
//! 26 dB under comfortable speech): a full spoken sentence collapsed to a
//! single "uh". The same audio peak-normalised to -3 dBFS transcribed it
//! correctly, with zero clipped samples.
//!
//! So we do two things after every recording:
//!   1. **Measure** RMS + true peak, and record them on the flight recorder so
//!      a bad transcript can be explained after the fact.
//!   2. **Rescue** — if the audio is quiet enough to be at risk, peak-normalise
//!      a copy to `TARGET_PEAK_DBFS` before it goes to the provider. The
//!      recording on disk is never touched; only the copy that gets uploaded.
//!
//! Gain is not a quality loss here. Amplifying quiet 16-bit audio raises the
//! quantisation noise floor along with the signal, but ASR is unbothered by
//! that — it is the same trade every field recorder makes in software, and it
//! is strictly better than silently losing words.

use std::path::{Path, PathBuf};

use anyhow::Result;

use super::wavio;

/// Below this RMS, tell the user their recording was very quiet and words may
/// be missing. Set from the measured fixture at -46.4 dBFS that *did* lose
/// content, with headroom — comfortable speech sits near -20 dBFS.
pub const QUIET_RMS_DBFS: f32 = -40.0;

/// Below this RMS we don't just warn, we normalise before sending. Deliberately
/// higher (i.e. fires earlier) than the warn threshold: normalising audio that
/// was merely a bit quiet is harmless, and the whole point is to catch the
/// damage before it happens rather than explain it afterwards.
pub const NORMALIZE_RMS_DBFS: f32 = -30.0;

/// Peak-normalisation target. -3 dBFS leaves headroom so no sample clips.
pub const TARGET_PEAK_DBFS: f32 = -3.0;

/// Ceiling on how much gain we will apply. A recording that needs more than
/// 40 dB is essentially silence — amplifying it just makes loud noise out of
/// a room tone, and the honest answer is the "too quiet" warning.
const MAX_GAIN_DB: f32 = 40.0;

/// A near-silent file has no meaningful peak to normalise against; treat it as
/// nothing-to-do rather than dividing by ~0 and manufacturing a gain of 10^6.
const SILENCE_PEAK_DBFS: f32 = -70.0;

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct LevelStats {
    pub rms_dbfs: f32,
    pub peak_dbfs: f32,
}

impl LevelStats {
    /// Quiet enough that the transcript is at risk of silently losing content.
    pub fn is_quiet(&self) -> bool {
        self.rms_dbfs < QUIET_RMS_DBFS && self.peak_dbfs > SILENCE_PEAK_DBFS
    }
}

pub struct LevelOutcome {
    pub stats: LevelStats,
    /// Side file holding the normalised copy, when one was produced. `None`
    /// means the original is fine to send as-is.
    pub normalized: Option<PathBuf>,
    /// Gain actually applied, in dB. 0.0 when no normalisation happened.
    pub gain_db: f32,
}

fn to_dbfs(amplitude: f32) -> f32 {
    if amplitude <= 1e-9 {
        -120.0
    } else {
        20.0 * amplitude.log10()
    }
}

pub fn measure_samples(samples: &[f32]) -> LevelStats {
    if samples.is_empty() {
        return LevelStats { rms_dbfs: -120.0, peak_dbfs: -120.0 };
    }
    // f64 accumulator: an hour of 48 kHz audio is ~170M samples, and summing
    // that many squares in f32 loses precision badly.
    let mut sum_sq = 0.0f64;
    let mut peak = 0.0f32;
    for &s in samples {
        sum_sq += (s as f64) * (s as f64);
        let a = s.abs();
        if a > peak {
            peak = a;
        }
    }
    let rms = (sum_sq / samples.len() as f64).sqrt() as f32;
    LevelStats { rms_dbfs: to_dbfs(rms), peak_dbfs: to_dbfs(peak) }
}

/// Measure `src`, and if it is quiet enough to risk dropped words, write a
/// peak-normalised copy alongside it.
///
/// Never mutates `src`. Errors are the caller's cue to carry on with the
/// original — a measurement problem must not cost the user a dictation.
pub fn analyze_and_rescue(src: &Path) -> Result<LevelOutcome> {
    let decoded = wavio::read_mono_f32(src)?;
    anyhow::ensure!(!decoded.samples.is_empty(), "empty WAV");
    let stats = measure_samples(&decoded.samples);

    let too_quiet = stats.rms_dbfs < NORMALIZE_RMS_DBFS;
    let has_signal = stats.peak_dbfs > SILENCE_PEAK_DBFS;
    if !too_quiet || !has_signal {
        return Ok(LevelOutcome { stats, normalized: None, gain_db: 0.0 });
    }

    let gain_db = (TARGET_PEAK_DBFS - stats.peak_dbfs).clamp(0.0, MAX_GAIN_DB);
    if gain_db < 1.0 {
        // Already peaking near the target — the low RMS is dynamic range, not
        // a gain problem, and there is nothing safe left to add.
        return Ok(LevelOutcome { stats, normalized: None, gain_db: 0.0 });
    }

    let factor = 10f32.powf(gain_db / 20.0);
    let boosted: Vec<f32> = decoded
        .samples
        .iter()
        .map(|s| (s * factor).clamp(-1.0, 1.0))
        .collect();

    let dst = wavio::tagged_path(src, "gain");
    wavio::write_mono_i16(&dst, &boosted, decoded.sample_rate)?;

    tracing::info!(
        src = %src.display(),
        rms_dbfs = stats.rms_dbfs,
        peak_dbfs = stats.peak_dbfs,
        gain_db,
        "quiet recording — normalised copy sent to speech-to-text"
    );

    Ok(LevelOutcome { stats, normalized: Some(dst), gain_db })
}

/// User-facing warning for a recording that came in under `QUIET_RMS_DBFS`.
/// Written to fit the floater bubble's hard 2-line cap.
pub fn quiet_warning(stats: &LevelStats, rescued: bool) -> String {
    if rescued {
        format!(
            "Mic was very quiet ({:.0} dBFS) — I boosted it before transcribing, but move the mic closer or raise its gain to avoid losing words.",
            stats.rms_dbfs
        )
    } else {
        format!(
            "Mic was very quiet ({:.0} dBFS) — words may be missing from this transcript. Move the mic closer or raise its input gain.",
            stats.rms_dbfs
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(amplitude: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (i as f32 * 0.1).sin() * amplitude)
            .collect()
    }

    #[test]
    fn full_scale_sine_measures_near_zero_dbfs() {
        let s = measure_samples(&tone(1.0, 10_000));
        assert!(s.peak_dbfs > -0.5, "peak {} should be ~0 dBFS", s.peak_dbfs);
        // A sine's RMS is peak/sqrt(2) = -3.01 dBFS.
        assert!(
            (s.rms_dbfs + 3.01).abs() < 0.3,
            "rms {} should be ~-3 dBFS",
            s.rms_dbfs
        );
    }

    /// The DJI fixture's measured level. It must land in "quiet" territory,
    /// because that is the case that silently ate a whole spoken phrase.
    #[test]
    fn dji_grade_level_is_flagged_quiet() {
        // -46 dBFS RMS ≈ amplitude 0.005 for a sine (peak = rms * sqrt(2)).
        let s = measure_samples(&tone(0.00708, 10_000));
        assert!(
            s.rms_dbfs < QUIET_RMS_DBFS,
            "rms {} should be under the {QUIET_RMS_DBFS} dBFS warn threshold",
            s.rms_dbfs
        );
        assert!(s.is_quiet());
    }

    #[test]
    fn healthy_level_is_not_flagged() {
        // -20 dBFS RMS: comfortable speech.
        let s = measure_samples(&tone(0.1414, 10_000));
        assert!(!s.is_quiet(), "rms {} should not warn", s.rms_dbfs);
        assert!(s.rms_dbfs > NORMALIZE_RMS_DBFS, "and should not normalise");
    }

    #[test]
    fn digital_silence_is_not_rescued() {
        let s = measure_samples(&vec![0.0; 1000]);
        assert!(!s.is_quiet(), "silence is not a gain problem");
    }

    /// Normalising must reach the target without clipping — the property that
    /// made this safe to turn on by default.
    #[test]
    fn rescue_boosts_to_target_without_clipping() {
        let dir = std::env::temp_dir();
        let src = dir.join("wispr-level-quiet.wav");
        wavio::write_mono_i16(&src, &tone(0.00708, 48_000), 48_000).unwrap();

        let out = analyze_and_rescue(&src).unwrap();
        let dst = out.normalized.expect("quiet audio should be rescued");
        assert!(out.gain_db > 20.0, "expected a big boost, got {}", out.gain_db);

        let after = measure_samples(&wavio::read_mono_f32(&dst).unwrap().samples);
        assert!(
            (after.peak_dbfs - TARGET_PEAK_DBFS).abs() < 1.0,
            "peak should land near {TARGET_PEAK_DBFS} dBFS, got {}",
            after.peak_dbfs
        );
        assert!(after.peak_dbfs < 0.0, "must not clip");

        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&dst);
    }

    #[test]
    fn healthy_audio_is_left_alone() {
        let dir = std::env::temp_dir();
        let src = dir.join("wispr-level-ok.wav");
        wavio::write_mono_i16(&src, &tone(0.1414, 16_000), 16_000).unwrap();
        let out = analyze_and_rescue(&src).unwrap();
        assert!(out.normalized.is_none(), "healthy audio must not be rewritten");
        let _ = std::fs::remove_file(&src);
    }
}
