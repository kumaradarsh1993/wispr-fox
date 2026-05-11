//! Daily API-usage counter. Persisted to disk so it survives restarts.
//!
//! Groq's free tier resets at midnight UTC; we track per-UTC-day counts of
//! STT (Whisper) and LLM (Llama) requests. The frontend reads this for the
//! footer indicator ("142 / 2000 today").
//!
//! Storage: `%APPDATA%/com.wispr-fox.app/usage.json`. Single-file format,
//! atomic write via temp + rename. No DB required for this — the data is
//! tiny and we only ever care about today + yesterday.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

const FILENAME: &str = "usage.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DailyUsage {
    /// UTC date in YYYY-MM-DD format. Reset when this changes.
    pub date: String,
    pub stt_count: u32,
    pub llm_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct UsageFile {
    today: DailyUsage,
}

#[derive(Clone)]
pub struct UsageTracker {
    path: PathBuf,
    inner: Arc<Mutex<DailyUsage>>,
}

impl UsageTracker {
    pub fn open() -> Result<Self> {
        let base = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("com.wispr-fox.app");
        fs::create_dir_all(&base).ok();
        let path = base.join(FILENAME);

        let loaded: UsageFile = fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let mut today = loaded.today;
        let today_str = Utc::now().format("%Y-%m-%d").to_string();
        if today.date != today_str {
            today = DailyUsage {
                date: today_str,
                stt_count: 0,
                llm_count: 0,
            };
        }

        Ok(Self {
            path,
            inner: Arc::new(Mutex::new(today)),
        })
    }

    fn roll_if_new_day(guard: &mut DailyUsage) {
        let today_str = Utc::now().format("%Y-%m-%d").to_string();
        if guard.date != today_str {
            *guard = DailyUsage {
                date: today_str,
                stt_count: 0,
                llm_count: 0,
            };
        }
    }

    pub fn snapshot(&self) -> DailyUsage {
        let mut g = self.inner.lock();
        Self::roll_if_new_day(&mut g);
        g.clone()
    }

    pub fn record_stt(&self) {
        let mut g = self.inner.lock();
        Self::roll_if_new_day(&mut g);
        g.stt_count += 1;
        let snap = g.clone();
        drop(g);
        self.persist(&snap);
    }

    pub fn record_llm(&self) {
        let mut g = self.inner.lock();
        Self::roll_if_new_day(&mut g);
        g.llm_count += 1;
        let snap = g.clone();
        drop(g);
        self.persist(&snap);
    }

    fn persist(&self, today: &DailyUsage) {
        let file = UsageFile { today: today.clone() };
        let Ok(json) = serde_json::to_string_pretty(&file) else { return };
        let tmp = self.path.with_extension("json.tmp");
        if fs::write(&tmp, &json).is_ok() {
            let _ = fs::rename(&tmp, &self.path);
        }
    }
}

/// Returns the timestamp for next UTC midnight — the frontend uses this to
/// schedule a refresh of the indicator at rollover time.
pub fn next_reset_utc() -> DateTime<Utc> {
    let now = Utc::now();
    let tomorrow = (now + chrono::Duration::days(1)).date_naive();
    tomorrow.and_hms_opt(0, 0, 0).unwrap().and_utc()
}
