//! Daily API-usage counter. Persisted to disk so it survives restarts.
//!
//! Storage: `%APPDATA%/com.wispr-fox.app/usage.json`. The file is tiny and
//! drives lightweight UI indicators, so a single JSON file is simpler than a
//! database. Old files with only aggregate counters continue to deserialize.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::llm::TokenUsage;

const FILENAME: &str = "usage.json";
const DEEPGRAM_FREE_CREDIT_USD: f64 = 200.0;
const DEEPGRAM_NOVA3_MULTILINGUAL_USD_PER_MIN: f64 = 0.0092;
const MAX_USAGE_DAYS: usize = 90;
const SNAPSHOT_DAYS: usize = 14;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelUsage {
    /// "stt" or "llm".
    pub stage: String,
    pub provider: String,
    pub model: String,
    pub calls: u32,
    #[serde(default)]
    pub audio_seconds: f64,
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub total_tokens: u64,
    /// Conservative estimate where the provider exposes stable metering.
    /// Currently only Deepgram STT is priced here; LLM pricing changes too
    /// often to hard-code safely.
    #[serde(default)]
    pub estimated_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageDay {
    pub date: String,
    #[serde(default)]
    pub model_usage: Vec<ModelUsage>,
}

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
    #[serde(default)]
    pub model_usage: Vec<ModelUsage>,
    /// API-only convenience for future stats screens. It is not persisted in
    /// `today`, because historical days live under UsageFile.days.
    #[serde(default, skip_serializing)]
    pub recent_days: Vec<UsageDay>,
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
    #[serde(default)]
    days: Vec<UsageDay>,
}

#[derive(Debug, Clone, Default)]
struct UsageState {
    today: DailyUsage,
    days: Vec<UsageDay>,
}

#[derive(Clone)]
pub struct UsageTracker {
    path: PathBuf,
    inner: Arc<Mutex<UsageState>>,
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

        let mut state = UsageState {
            today: loaded.today,
            days: loaded.days,
        };
        Self::normalize_deepgram_fields(&mut state.today);
        Self::roll_if_new_day(&mut state);
        Self::dedupe_and_trim_days(&mut state.days, &state.today.date);

        Ok(Self {
            path,
            inner: Arc::new(Mutex::new(state)),
        })
    }

    fn normalize_deepgram_fields(today: &mut DailyUsage) {
        today.deepgram_free_credit_usd = DEEPGRAM_FREE_CREDIT_USD;
        today.deepgram_rate_usd_per_min = DEEPGRAM_NOVA3_MULTILINGUAL_USD_PER_MIN;
    }

    fn today_str() -> String {
        Utc::now().format("%Y-%m-%d").to_string()
    }

    fn roll_if_new_day(state: &mut UsageState) {
        let today_str = Self::today_str();
        if state.today.date.is_empty() {
            state.today.date = today_str;
            Self::normalize_deepgram_fields(&mut state.today);
            return;
        }

        if state.today.date != today_str {
            Self::archive_day(&mut state.days, &state.today);

            let deepgram_audio_seconds = state.today.deepgram_audio_seconds;
            let deepgram_estimated_usd = state.today.deepgram_estimated_usd;
            state.today = DailyUsage {
                date: today_str,
                stt_count: 0,
                llm_count: 0,
                deepgram_audio_seconds,
                deepgram_estimated_usd,
                deepgram_free_credit_usd: DEEPGRAM_FREE_CREDIT_USD,
                deepgram_rate_usd_per_min: DEEPGRAM_NOVA3_MULTILINGUAL_USD_PER_MIN,
                model_usage: Vec::new(),
                recent_days: Vec::new(),
            };
        }
        Self::normalize_deepgram_fields(&mut state.today);
        Self::dedupe_and_trim_days(&mut state.days, &state.today.date);
    }

    fn archive_day(days: &mut Vec<UsageDay>, day: &DailyUsage) {
        if day.date.is_empty() || !Self::has_usage(day) {
            return;
        }
        days.retain(|d| d.date != day.date);
        days.push(UsageDay {
            date: day.date.clone(),
            model_usage: day.model_usage.clone(),
        });
        Self::dedupe_and_trim_days(days, "");
    }

    fn has_usage(day: &DailyUsage) -> bool {
        day.stt_count > 0 || day.llm_count > 0 || !day.model_usage.is_empty()
    }

    fn dedupe_and_trim_days(days: &mut Vec<UsageDay>, current_date: &str) {
        days.retain(|d| !d.date.is_empty() && d.date != current_date);
        days.sort_by(|a, b| a.date.cmp(&b.date));
        days.dedup_by(|a, b| a.date == b.date);
        if days.len() > MAX_USAGE_DAYS {
            let drain_to = days.len() - MAX_USAGE_DAYS;
            days.drain(0..drain_to);
        }
    }

    fn strip_snapshot_fields(today: &DailyUsage) -> DailyUsage {
        let mut day = today.clone();
        day.recent_days.clear();
        day
    }

    fn snapshot_from_state(state: &UsageState) -> DailyUsage {
        let mut snapshot = state.today.clone();
        let mut recent = state.days.clone();
        if Self::has_usage(&state.today) {
            recent.push(UsageDay {
                date: state.today.date.clone(),
                model_usage: state.today.model_usage.clone(),
            });
        }
        recent.sort_by(|a, b| a.date.cmp(&b.date));
        if recent.len() > SNAPSHOT_DAYS {
            let drain_to = recent.len() - SNAPSHOT_DAYS;
            recent.drain(0..drain_to);
        }
        recent.reverse();
        snapshot.recent_days = recent;
        snapshot
    }

    fn model_label(provider: &str, model: &str) -> String {
        if model.trim().is_empty() {
            provider.to_string()
        } else {
            model.to_string()
        }
    }

    fn bucket_mut<'a>(
        day: &'a mut DailyUsage,
        stage: &str,
        provider: &str,
        model: &str,
    ) -> &'a mut ModelUsage {
        let model = Self::model_label(provider, model);
        if let Some(index) = day
            .model_usage
            .iter()
            .position(|b| b.stage == stage && b.provider == provider && b.model == model)
        {
            return &mut day.model_usage[index];
        }
        day.model_usage.push(ModelUsage {
            stage: stage.to_string(),
            provider: provider.to_string(),
            model,
            calls: 0,
            audio_seconds: 0.0,
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            estimated_usd: 0.0,
        });
        day.model_usage.last_mut().expect("bucket was just pushed")
    }

    pub fn snapshot(&self) -> DailyUsage {
        let mut g = self.inner.lock();
        Self::roll_if_new_day(&mut g);
        Self::snapshot_from_state(&g)
    }

    pub fn record_stt(&self, provider: &str, model: &str, billable_seconds: f64) {
        let mut g = self.inner.lock();
        Self::roll_if_new_day(&mut g);
        g.today.stt_count += 1;

        let seconds = billable_seconds.max(0.0);
        let estimated_usd = if provider == "deepgram" {
            let cost = (seconds / 60.0) * DEEPGRAM_NOVA3_MULTILINGUAL_USD_PER_MIN;
            g.today.deepgram_audio_seconds += seconds;
            g.today.deepgram_estimated_usd += cost;
            cost
        } else {
            0.0
        };
        Self::normalize_deepgram_fields(&mut g.today);

        let bucket = Self::bucket_mut(&mut g.today, "stt", provider, model);
        bucket.calls += 1;
        bucket.audio_seconds += seconds;
        bucket.estimated_usd += estimated_usd;

        let snap = g.clone();
        drop(g);
        self.persist_state(&snap);
    }

    pub fn record_llm(&self, provider: &str, model: &str, usage: Option<&TokenUsage>) {
        let mut g = self.inner.lock();
        Self::roll_if_new_day(&mut g);
        g.today.llm_count += 1;

        let bucket = Self::bucket_mut(&mut g.today, "llm", provider, model);
        bucket.calls += 1;
        if let Some(usage) = usage {
            bucket.input_tokens += usage.input_tokens;
            bucket.output_tokens += usage.output_tokens;
            bucket.total_tokens += usage
                .total_tokens
                .max(usage.input_tokens + usage.output_tokens);
        }

        let snap = g.clone();
        drop(g);
        self.persist_state(&snap);
    }

    fn persist_state(&self, state: &UsageState) {
        let file = UsageFile {
            today: Self::strip_snapshot_fields(&state.today),
            days: state.days.clone(),
        };
        let Ok(json) = serde_json::to_string_pretty(&file) else {
            return;
        };
        let tmp = self.path.with_extension("json.tmp");
        if fs::write(&tmp, &json).is_ok() {
            let _ = fs::rename(&tmp, &self.path);
        }
    }
}

/// Returns the timestamp for next UTC midnight. The frontend uses this to
/// schedule a refresh of the indicator at rollover time.
pub fn next_reset_utc() -> DateTime<Utc> {
    let now = Utc::now();
    let tomorrow = (now + chrono::Duration::days(1)).date_naive();
    tomorrow.and_hms_opt(0, 0, 0).unwrap().and_utc()
}
