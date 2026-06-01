//! Hourly retention sweep: delete audio files + history rows older than
//! `retention_days`, AND enforce the `retention_max_mb` storage cap, AND
//! prune empty date folders left behind by either of those.
//!
//! Runs as a tokio interval task spawned from `lib::run`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use parking_lot::Mutex;

use crate::history::History;
use crate::settings::AppSettings;

const TICK: Duration = Duration::from_secs(3600);

pub fn spawn(history: History, settings: Arc<Mutex<AppSettings>>) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(TICK);
        loop {
            interval.tick().await;
            run_once(&history, &settings);
        }
    });
}

/// One sweep iteration — split out so tests / manual triggers can run the
/// same logic. Idempotent and self-contained.
fn run_once(history: &History, settings: &Arc<Mutex<AppSettings>>) {
    let (days, max_mb) = {
        let s = settings.lock();
        (s.retention_days as i64, s.retention_max_mb)
    };

    // Age-based purge — anything older than retention_days goes.
    if days > 0 {
        let cutoff = Utc::now() - ChronoDuration::days(days);
        match history.list_purgeable(cutoff) {
            Ok(rows) => {
                for (id, path) in rows {
                    let _ = std::fs::remove_file(&path);
                    if let Err(e) = history.delete(&id) {
                        tracing::warn!(%id, "history.delete failed: {e:#}");
                    }
                }
            }
            Err(e) => tracing::warn!("list_purgeable failed: {e:#}"),
        }
    }

    // Storage-cap purge — even within the retention window, if the audio
    // folder exceeds the user's max-MB cap, delete oldest files until we're
    // back under. This was declared in settings but NEVER enforced until
    // v1.1.0-nightly.7 — audit found audio could grow unbounded inside the
    // retention window (e.g. lots of long sessions in a single day).
    if max_mb > 0 {
        if let Some(audio_dir) = current_audio_dir() {
            enforce_storage_cap(&audio_dir, history, max_mb);
            prune_empty_date_folders(&audio_dir);
        }
    }
}

/// Audio root from the app-data dir. Returns None on platforms without an
/// app-data dir (shouldn't happen on desktop targets but we don't want to
/// panic if it does — the sweep just no-ops).
fn current_audio_dir() -> Option<PathBuf> {
    // dirs::data_dir() matches Tauri's app_data_dir() result on Windows/macOS/
    // Linux desktop. We can't reach into Tauri's AppHandle from here (gc has
    // no handle), but dirs gives the same path.
    let base = dirs::data_dir()?.join("com.wispr-fox.app").join("audio");
    if base.exists() {
        Some(base)
    } else {
        None
    }
}

/// Delete oldest audio files (and their matching history rows) until the
/// total size is below `max_mb`. Walks the per-day subfolders.
fn enforce_storage_cap(audio_dir: &Path, history: &History, max_mb: u64) {
    let max_bytes = max_mb.saturating_mul(1024 * 1024);

    // Collect (path, size, mtime) for every audio file.
    let mut entries: Vec<(PathBuf, u64, std::time::SystemTime)> = Vec::new();
    let Ok(date_dirs) = std::fs::read_dir(audio_dir) else { return };
    for date_dir in date_dirs.flatten() {
        let Ok(meta) = date_dir.metadata() else { continue };
        if !meta.is_dir() {
            continue;
        }
        let Ok(files) = std::fs::read_dir(date_dir.path()) else { continue };
        for f in files.flatten() {
            let Ok(fmeta) = f.metadata() else { continue };
            if !fmeta.is_file() {
                continue;
            }
            let mtime = fmeta.modified().unwrap_or(std::time::UNIX_EPOCH);
            entries.push((f.path(), fmeta.len(), mtime));
        }
    }

    let total: u64 = entries.iter().map(|(_, sz, _)| *sz).sum();
    if total <= max_bytes {
        return;
    }

    // Sort oldest-first.
    entries.sort_by_key(|(_, _, mtime)| *mtime);
    let mut bytes_remaining = total;
    let target = max_bytes.saturating_sub(max_bytes / 10); // delete to 90% of cap so we don't run the sweep every hour
    let mut removed_count = 0usize;
    let mut removed_bytes = 0u64;
    for (path, size, _) in entries {
        if bytes_remaining <= target {
            break;
        }
        // Try to delete the matching history row first (best-effort lookup).
        if let Ok(Some(id)) = history.find_by_audio_path(&path) {
            let _ = history.delete(&id);
        }
        let _ = std::fs::remove_file(&path);
        bytes_remaining = bytes_remaining.saturating_sub(size);
        removed_bytes = removed_bytes.saturating_add(size);
        removed_count += 1;
    }
    if removed_count > 0 {
        tracing::info!(
            removed_count,
            removed_mb = removed_bytes / (1024 * 1024),
            cap_mb = max_mb,
            "storage-cap sweep removed oldest audio"
        );
    }
}

/// After purging files, day-folders may be empty (e.g. `audio/2026-05-12/`
/// after we deleted all that day's recordings). Remove empty folders so the
/// directory listing stays tidy and explorer doesn't fill with ghost dates.
fn prune_empty_date_folders(audio_dir: &Path) {
    let Ok(date_dirs) = std::fs::read_dir(audio_dir) else { return };
    for date_dir in date_dirs.flatten() {
        let Ok(meta) = date_dir.metadata() else { continue };
        if !meta.is_dir() {
            continue;
        }
        if let Ok(mut contents) = std::fs::read_dir(date_dir.path()) {
            if contents.next().is_none() {
                let _ = std::fs::remove_dir(date_dir.path());
            }
        }
    }
}
