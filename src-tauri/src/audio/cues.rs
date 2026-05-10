//! Audio cues for recording start/stop.
//!
//! By default plays a generated 360Hz "dop" tone — short, low-pitched, soft
//! attack. To customise: drop audio files (WAV / MP3 / OGG) at
//! `%APPDATA%/com.wispr-fox.app/sounds/start.{wav,mp3,ogg}` and
//! `.../sounds/stop.{wav,mp3,ogg}`. The app picks them up automatically on
//! next launch — no config needed.
//!
//! rodio's `OutputStream` is `!Send` on Windows (WASAPI thread affinity),
//! so we own it on a dedicated audio-output thread and accept play commands
//! via an mpsc channel.

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::OnceLock;
use std::time::Duration;

use rodio::source::{Source, SineWave};
use rodio::{Decoder, OutputStream};

enum CueCmd {
    Start,
    Stop,
}

static SENDER: OnceLock<Option<mpsc::Sender<CueCmd>>> = OnceLock::new();

fn sender() -> Option<&'static mpsc::Sender<CueCmd>> {
    SENDER
        .get_or_init(|| {
            let (tx, rx) = mpsc::channel::<CueCmd>();
            let spawn_ok = std::thread::Builder::new()
                .name("wispr-cues".into())
                .spawn(move || cue_worker(rx))
                .is_ok();
            if spawn_ok { Some(tx) } else {
                tracing::warn!("audio cues: failed to spawn output thread");
                None
            }
        })
        .as_ref()
}

fn sounds_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.wispr-fox.app")
        .join("sounds")
}

/// Search for `{base}.wav`, `{base}.mp3`, `{base}.ogg` in the user's sounds dir.
fn find_custom(base: &str) -> Option<PathBuf> {
    let dir = sounds_dir();
    for ext in &["wav", "mp3", "ogg"] {
        let p = dir.join(format!("{base}.{ext}"));
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn cue_worker(rx: mpsc::Receiver<CueCmd>) {
    let stream = match OutputStream::try_default() {
        Ok((s, h)) => Some((s, h)),
        Err(e) => {
            tracing::warn!("audio cues: no default output device: {e}");
            None
        }
    };

    while let Ok(cmd) = rx.recv() {
        let Some((_s, handle)) = &stream else { continue };
        let base = match cmd {
            CueCmd::Start => "start",
            CueCmd::Stop => "stop",
        };

        // 1. Try user-supplied file.
        if let Some(path) = find_custom(base) {
            match File::open(&path).and_then(|f| {
                Decoder::new(BufReader::new(f))
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            }) {
                Ok(source) => {
                    if let Err(e) = handle.play_raw(source.convert_samples()) {
                        tracing::debug!("custom cue play failed: {e}");
                    }
                    continue;
                }
                Err(e) => {
                    tracing::warn!("could not decode custom cue {path:?}: {e}");
                    // Fall through to generated tone.
                }
            }
        }

        // 2. Fallback to generated tones — single "dop" on start, double on stop.
        let play_dop = |h: &rodio::OutputStreamHandle| {
            let source = SineWave::new(360.0)
                .take_duration(Duration::from_millis(80))
                .fade_in(Duration::from_millis(8))
                .amplify(0.18);
            if let Err(e) = h.play_raw(source.convert_samples()) {
                tracing::debug!("generated cue play failed: {e}");
            }
        };

        // Single "dop" on both start and stop — user feedback: double-dop on
        // stop felt heavy. One dop in, one dop out, that's it.
        let _ = cmd;
        play_dop(handle);
    }
}

pub fn play_start() {
    if let Some(tx) = sender() {
        let _ = tx.send(CueCmd::Start);
    }
}

pub fn play_stop() {
    if let Some(tx) = sender() {
        let _ = tx.send(CueCmd::Stop);
    }
}
