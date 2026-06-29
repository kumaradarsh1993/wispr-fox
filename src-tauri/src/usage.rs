//! Daily API-usage counter. Persisted to disk so it survives restarts.
//!
//! Groq-style call counters reset at midnight UTC. Deepgram credit usage is
//! cumulative because its free credit is account-level, not a daily allowance.
//!
//! Storage: `%APPDATA%/com.wispr-fox.app/usage.json`. Single-file format,
//! atomic write via temp + rename. No DB required for this — the data is
//! tiny and only drives lightweight UI indicators.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

const FILENAME: &str = "usage.json";
const DEEPGRAM_FREE_CREDIT_USD: f64 = 200.0;
const DEEPGRAM_NOVA3_MULTILINGUAL_USD_PER_MIN: f64 = 0.0092;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DailyUsage {
    /// UTC date in YYYY-MM-DD format. Reset when this changes.
    pub date: String,
    pub stt_count: u32,
    pub llm_count: u32,
    /// Cumulative Deepgram audio duration estimated against the free credit.
    #[serde(default)]
    pub deepgram_audio_seconds: f64,
    /// Cumulative Deepgram estimate, using Nova-3 multilingual pay-as-you-go.
    #[serde(default)]
    pub deepgram_estimated_usd: f64,
    #[serde(default = "deepgram_free_credit_usd")]
    pub deepgram_free_credit_usd: f64,
    #[serde(default = "deepgram_rate_usd_per_min")]
    pub deepgram_rate_usd_per_min: f64,
}

fn deepgram_free_credit_usd() -> f64 {
    DEEPGRAM_FREE_CREDIT_USD
}

fn deepgram_rate_usd_per_min() -> f64 {
    DEEPGRAM_NOVA3_MULTILINGUAL_USD_PER_MIN
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
        let deepgram_audio_seconds = today.deepgram_audio_seconds;
        let deepgram_estimated_usd = today.deepgram_estimated_usd;
        let today_str = Utc::now().format("%Y-%m-%d").to_string();
        if today.date != today_str {
            today = DailyUsage {
                date: today_str,
                stt_count: 0,
                llm_count: 0,
                deepgram_audio_seconds,
                deepgram_estimated_usd,
                deepgram_free_credit_usd: DEEPGRAM_FREE_CREDIT_USD,
                deepgram_rate_usd_per_min: DEEPGRAM_NOVA3_MULTILINGUAL_USD_PER_MIN,
            };
        }
        today.deepgram_free_credit_usd = DEEPGRAM_FREE_CREDIT_USD;
        today.deepgram_rate_usd_per_min = DEEPGRAM_NOVA3_MULTILINGUAL_USD_PER_MIN;

        Ok(Self {
            path,
            inner: Arc::new(Mutex::new(today)),
        })
    }

    fn roll_if_new_day(guard: &mut DailyUsage) {
        let today_str = Utc::now().format("%Y-%m-%d").to_string();
        if guard.date != today_str {
            let deepgram_audio_seconds = guard.deepgram_audio_seconds;
            let deepgram_estimated_usd = guard.deepgram_estimated_usd;
            *guard = DailyUsage {
                date: today_str,
                stt_count: 0,
                llm_count: 0,
                deepgram_audio_seconds,
                deepgram_estimated_usd,
                deepgram_free_credit_usd: DEEPGRAM_FREE_CREDIT_USD,
                deepgram_rate_usd_per_min: DEEPGRAM_NOVA3_MULTILINGUAL_USD_PER_MIN,
            };
        }
    }

    pub fn snapshot(&self) -> DailyUsage {
        let mut g = self.inner.lock();
        Self::roll_if_new_day(&mut g);
        g.clone()
    }

    pub fn record_stt(&self, provider: &str, billable_seconds: f64) {
        let mut g = self.inner.lock();
        Self::roll_if_new_day(&mut g);
        g.stt_count += 1;
        if provider == "deepgram" {
            let seconds = billable_seconds.max(0.0);
            g.deepgram_audio_seconds += seconds;
            g.deepgram_estimated_usd += (seconds / 60.0) * DEEPGRAM_NOVA3_MULTILINGUAL_USD_PER_MIN;
            g.deepgram_free_credit_usd = DEEPGRAM_FREE_CREDIT_USD;
            g.deepgram_rate_usd_per_min = DEEPGRAM_NOVA3_MULTILINGUAL_USD_PER_MIN;
        }
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
