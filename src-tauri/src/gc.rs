//! Hourly retention sweep: delete audio files + history rows older than
//! `retention_days`. Runs as a tokio interval task spawned from `lib::run`.

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
            let days = settings.lock().retention_days as i64;
            if days <= 0 {
                continue;
            }
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
    });
}
