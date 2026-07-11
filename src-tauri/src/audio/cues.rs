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

use parking_lot::Mutex as ParkingMutex;
use rodio::source::{Source, SineWave};
use rodio::{Decoder, OutputStream};

enum CueCmd {
    Start,
    Stop,
}

/// User-selected sound filenames (within the sounds/ folder). Empty = use
/// the legacy default ("start.{wav,mp3,ogg}" → "stop.{wav,mp3,ogg}") and
/// then the built-in generated tone.
static USER_START_SOUND: OnceLock<ParkingMutex<String>> = OnceLock::new();
static USER_STOP_SOUND: OnceLock<ParkingMutex<String>> = OnceLock::new();
static CUES_ENABLED: OnceLock<ParkingMutex<bool>> = OnceLock::new();

fn user_start_sound() -> &'static ParkingMutex<String> {
    USER_START_SOUND.get_or_init(|| ParkingMutex::new(String::new()))
}
fn user_stop_sound() -> &'static ParkingMutex<String> {
    USER_STOP_SOUND.get_or_init(|| ParkingMutex::new(String::new()))
}
fn cues_enabled() -> &'static ParkingMutex<bool> {
    CUES_ENABLED.get_or_init(|| ParkingMutex::new(true))
}

/// Push the user's settings into the cues system. Called whenever settings
/// change (the JS side invokes a Tauri command after saving). Empty strings
/// mean "fall back to legacy default lookup".
pub fn configure(start: &str, stop: &str, enabled: bool) {
    *user_start_sound().lock() = start.to_string();
    *user_stop_sound().lock() = stop.to_string();
    *cues_enabled().lock() = enabled;
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

/// Resolve a user-supplied filename to a full path inside sounds dir.
/// Returns None if the file isn't there (e.g. user deleted it).
fn resolve_user_file(filename: &str) -> Option<PathBuf> {
    if filename.is_empty() {
        return None;
    }
    let p = sounds_dir().join(filename);
    if p.is_file() { Some(p) } else { None }
}

fn cue_worker(rx: mpsc::Receiver<CueCmd>) {
    // The output stream is opened ON DEMAND and dropped after a short idle
    // window — never held for the app's lifetime. rodio's OutputStream keeps
    // the WASAPI render stream actively playing (silence when no source), and
    // Windows holds a "An audio stream is currently in use" power request for
    // as long as any render stream is live — a permanently-open stream here
    // blocked automatic system sleep on the whole machine (found 2026-07-12:
    // one dictation after launch and the laptop never slept again).
    // 30 s keeps the start→stop cue pair latency-free within a dictation and
    // easily outlives the 80 ms tones before the stream is released.
    const IDLE_DROP: Duration = Duration::from_secs(30);
    let mut stream: Option<(OutputStream, rodio::OutputStreamHandle)> = None;

    loop {
        let cmd = if stream.is_some() {
            match rx.recv_timeout(IDLE_DROP) {
                Ok(cmd) => cmd,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    tracing::debug!("audio cues: idle — releasing output stream");
                    stream = None;
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        } else {
            match rx.recv() {
                Ok(cmd) => cmd,
                Err(_) => break,
            }
        };

        if !*cues_enabled().lock() {
            continue;
        }
        if stream.is_none() {
            match OutputStream::try_default() {
                Ok((s, h)) => stream = Some((s, h)),
                Err(e) => {
                    tracing::warn!("audio cues: no default output device: {e}");
                    continue;
                }
            }
        }
        let Some((_s, handle)) = &stream else { continue };

        let (user_pick, base) = match cmd {
            CueCmd::Start => (user_start_sound().lock().clone(), "start"),
            CueCmd::Stop => (user_stop_sound().lock().clone(), "stop"),
        };

        // 1a. User-selected filename from Settings has priority.
        if let Some(path) = resolve_user_file(&user_pick) {
            match File::open(&path).and_then(|f| {
                Decoder::new(BufReader::new(f))
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            }) {
                Ok(source) => {
                    if let Err(e) = handle.play_raw(source.convert_samples()) {
                        tracing::debug!("user-pick cue play failed: {e}");
                    }
                    continue;
                }
                Err(e) => {
                    tracing::warn!("user-pick cue {path:?} failed to decode: {e}");
                }
            }
        }

        // 1b. Legacy default ("start.wav" / "stop.wav" in sounds dir).
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
