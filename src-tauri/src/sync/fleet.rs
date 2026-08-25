//! The fleet — every device signed into this account, what to call it, and
//! what it has dictated.
//!
//! Three things live here, and all three ride on tables that ALREADY exist
//! (`devices` and the generic `user_settings` KV described in
//! `wispr-fox-web/docs/SYNC_DESIGN.md`). **No schema migration is required**,
//! which is deliberate: the Supabase project is the user's, DDL is a one-time
//! manual step, and this feature was not worth spending one on.
//!
//! 1. **The registry** — a read of the `devices` table the sync engine has
//!    been populating since v3.0.0. Nothing new is written; it was simply
//!    never surfaced in the UI.
//!
//! 2. **Display identity** (`device_meta:<device_id>`) — the icon and label
//!    the user assigns so "Alienware" reads as "Home desktop" with a house on
//!    it. Any device may write any device's meta: the user is as likely to
//!    label the laptop while sitting at the desktop as the other way round.
//!    Last write wins, which is correct for a single-user preference.
//!
//! 3. **Analytics** (`device_stats:<device_id>`) — this device's lifetime
//!    rollup, so Insights can answer "how much have I dictated, across
//!    everything" instead of one honest-but-partial answer per machine. A
//!    device writes ONLY its own key, so there is no read-modify-write race
//!    between devices no matter how many are online at once.
//!
//! Audio still never leaves the machine that recorded it, and neither does
//! transcript text beyond what the existing notes sync already carries. What
//! is added here is counts and dates.

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::history::{History, StatsSummary};

use super::config;

/// Cap on per-day rows carried in a device's published rollup.
///
/// The Insights chart never looks back further than 90 days, so 180 is two
/// full windows of headroom. Lifetime totals are carried separately and are
/// NOT truncated — the headline "time saved" figure stays exact forever even
/// though the daily series behind the chart is trimmed. Without this cap a
/// long-lived install would publish an unbounded row into a `text` column.
const MAX_PUBLISHED_DAYS: usize = 180;

pub const META_PREFIX: &str = "device_meta:";
pub const STATS_PREFIX: &str = "device_stats:";

/// Local cache of the last successfully assembled fleet, so Settings and
/// Insights render instantly and keep working offline.
const FLEET_CACHE_KEY: &str = "fleet_cache_v1";
/// Content hash of the stats blob we last successfully published. Mirrors the
/// dedup that `sync_settings` does for API keys, and for the same reason: the
/// sync loop runs every ~60s and re-pushing an unchanged blob every minute is
/// pure write amplification against the user's database.
const STATS_PUSH_HASH_KEY: &str = "fleet_stats_push_hash";

/// A device's user-assigned presentation. Both fields are optional: an
/// un-personalised device falls back to its registered hostname and a generic
/// platform glyph.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeviceMeta {
    /// Icon id from `src/lib/device-icons.ts`. Free-form on purpose — an
    /// unknown id degrades to the default glyph rather than failing to parse,
    /// so a newer client can add icons without breaking an older one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Overrides the registered device name for display only. The `devices`
    /// row keeps the real hostname so the user can still tell machines apart
    /// if they label two of them the same thing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// What one device publishes about itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceStats {
    /// Envelope version. A reader that does not recognise the version skips
    /// the blob rather than guessing at its shape.
    pub v: u32,
    pub summary: StatsSummary,
    pub published_at: String,
}

/// One row of the `devices` table.
#[derive(Debug, Clone, Deserialize)]
struct DeviceRow {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    platform: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    last_seen_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct KvRow {
    key: String,
    value: String,
}

/// A device as the UI sees it: registry row + assigned identity + its rollup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetDevice {
    pub id: String,
    /// Registered hostname / model, straight from the `devices` table.
    pub name: Option<String>,
    pub platform: Option<String>,
    pub created_at: Option<String>,
    pub last_seen_at: Option<String>,
    pub icon: Option<String>,
    pub label: Option<String>,
    /// True for the install this process is running on. The UI pins it first
    /// and refuses to let the user "forget" it.
    pub this_device: bool,
    /// None when that device has not published a rollup yet — an older client,
    /// or one that has not synced since this feature shipped. The UI must say
    /// "not reporting yet" rather than showing a confident zero.
    pub stats: Option<StatsSummary>,
}

fn http() -> reqwest::Client {
    super::engine::http()
}

fn content_hash(s: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    format!("{:x}", h.finalize())
}

/// Trim a summary to what is worth publishing (see `MAX_PUBLISHED_DAYS`).
/// Lifetime totals are copied through untouched.
fn publishable(mut summary: StatsSummary) -> StatsSummary {
    if summary.days.len() > MAX_PUBLISHED_DAYS {
        let cut = summary.days.len() - MAX_PUBLISHED_DAYS;
        summary.days.drain(..cut);
    }
    summary
}

/// Publish this device's rollup, skipping the write when nothing has changed
/// since the last successful push.
pub async fn push_stats(
    history: &History,
    token: &str,
    user_id: &str,
    device_id: &str,
) -> anyhow::Result<()> {
    let summary = publishable(history.stats_summary()?);
    let blob = DeviceStats {
        v: 1,
        summary,
        published_at: Utc::now().to_rfc3339(),
    };
    // Hash the SUMMARY, not the envelope — `published_at` changes on every
    // call and would defeat the dedup entirely.
    let payload = serde_json::to_string(&blob.summary)?;
    let hash = content_hash(&payload);
    if history.meta_get(STATS_PUSH_HASH_KEY)?.as_deref() == Some(hash.as_str()) {
        return Ok(());
    }

    let value = serde_json::to_string(&blob)?;
    let url = format!("{}/rest/v1/user_settings", config::SUPABASE_URL);
    let row = serde_json::json!({
        "user_id": user_id,
        "key": format!("{STATS_PREFIX}{device_id}"),
        "value": value,
        "updated_at": Utc::now().to_rfc3339(),
    });
    let resp = http()
        .post(&url)
        .header("apikey", config::SUPABASE_ANON_KEY)
        .header("Authorization", format!("Bearer {token}"))
        .header("Prefer", "resolution=merge-duplicates")
        .json(&row)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("fleet stats push: {e}"))?;
    if !resp.status().is_success() {
        anyhow::bail!("fleet stats push HTTP {}", resp.status().as_u16());
    }
    // Only remember the hash once the server accepted it, so a 4xx or a
    // dropped connection retries next cycle instead of being treated as done.
    history.meta_set(STATS_PUSH_HASH_KEY, &hash)?;
    Ok(())
}

/// Write one device's display identity. Callable for ANY device in the
/// account, not just this one.
pub async fn put_device_meta(
    token: &str,
    user_id: &str,
    device_id: &str,
    meta: &DeviceMeta,
) -> anyhow::Result<()> {
    let url = format!("{}/rest/v1/user_settings", config::SUPABASE_URL);
    let row = serde_json::json!({
        "user_id": user_id,
        "key": format!("{META_PREFIX}{device_id}"),
        "value": serde_json::to_string(meta)?,
        "updated_at": Utc::now().to_rfc3339(),
    });
    let resp = http()
        .post(&url)
        .header("apikey", config::SUPABASE_ANON_KEY)
        .header("Authorization", format!("Bearer {token}"))
        .header("Prefer", "resolution=merge-duplicates")
        .json(&row)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("device meta push: {e}"))?;
    if !resp.status().is_success() {
        anyhow::bail!("device meta push HTTP {}", resp.status().as_u16());
    }
    Ok(())
}

/// Pull the registry plus every `device_meta:` / `device_stats:` row, join
/// them, and cache the result locally.
pub async fn pull_fleet(
    history: &History,
    token: &str,
    this_device_id: &str,
) -> anyhow::Result<Vec<FleetDevice>> {
    let devices_url = format!(
        "{}/rest/v1/devices?select=id,name,platform,created_at,last_seen_at",
        config::SUPABASE_URL
    );
    let resp = http()
        .get(&devices_url)
        .header("apikey", config::SUPABASE_ANON_KEY)
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("fleet devices pull: {e}"))?;
    if !resp.status().is_success() {
        anyhow::bail!("fleet devices pull HTTP {}", resp.status().as_u16());
    }
    let devices: Vec<DeviceRow> = resp.json().await.unwrap_or_default();

    let kv_url = format!("{}/rest/v1/user_settings?select=key,value", config::SUPABASE_URL);
    let resp = http()
        .get(&kv_url)
        .header("apikey", config::SUPABASE_ANON_KEY)
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("fleet settings pull: {e}"))?;
    // A settings read failing must not lose the registry we just fetched —
    // fall back to no metadata rather than failing the whole pull.
    let kv: Vec<KvRow> = if resp.status().is_success() {
        resp.json().await.unwrap_or_default()
    } else {
        Vec::new()
    };

    let mut fleet: Vec<FleetDevice> = devices
        .into_iter()
        .map(|d| {
            let meta = kv
                .iter()
                .find(|r| r.key == format!("{META_PREFIX}{}", d.id))
                .and_then(|r| serde_json::from_str::<DeviceMeta>(&r.value).ok())
                .unwrap_or_default();
            // A stats blob we cannot parse, or one from a future envelope
            // version, is dropped rather than guessed at.
            let stats = kv
                .iter()
                .find(|r| r.key == format!("{STATS_PREFIX}{}", d.id))
                .and_then(|r| serde_json::from_str::<DeviceStats>(&r.value).ok())
                .filter(|b| b.v == 1)
                .map(|b| b.summary);
            let this_device = d.id == this_device_id;
            FleetDevice {
                this_device,
                id: d.id,
                name: d.name,
                platform: d.platform,
                created_at: d.created_at,
                last_seen_at: d.last_seen_at,
                icon: meta.icon,
                label: meta.label,
                stats,
            }
        })
        .collect();

    // This device first, then most-recently-seen. The user's own machine
    // being anywhere but the top of its own device list reads as a bug.
    fleet.sort_by(|a, b| {
        b.this_device
            .cmp(&a.this_device)
            .then_with(|| b.last_seen_at.cmp(&a.last_seen_at))
    });

    if let Ok(json) = serde_json::to_string(&fleet) {
        let _ = history.meta_set(FLEET_CACHE_KEY, &json);
    }
    Ok(fleet)
}

/// The last fleet we successfully assembled. Used to paint the UI before the
/// first network round-trip lands, and whenever the user is offline.
pub fn cached_fleet(history: &History) -> Vec<FleetDevice> {
    history
        .meta_get(FLEET_CACHE_KEY)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str::<Vec<FleetDevice>>(&s).ok())
        .unwrap_or_default()
}

/// Drop a cached fleet entry. Called on sign-out so the next account does not
/// briefly see the previous account's devices.
pub fn clear_cache(history: &History) {
    let _ = history.meta_set(FLEET_CACHE_KEY, "[]");
    let _ = history.meta_set(STATS_PUSH_HASH_KEY, "");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::DailyStat;

    fn day(date: &str, words: i64) -> DailyStat {
        DailyStat {
            date: date.to_string(),
            sessions: 1,
            words,
            dictation_ms: 1000,
            light_count: 1,
            draft_count: 0,
        }
    }

    /// The daily series is trimmed to the newest `MAX_PUBLISHED_DAYS`, and the
    /// lifetime totals are NOT touched by that trim. Getting this backwards
    /// would silently shrink the headline "time saved" number on every device
    /// that has been running for more than six months.
    #[test]
    fn publishable_trims_days_but_keeps_lifetime_totals() {
        let days: Vec<DailyStat> = (0..MAX_PUBLISHED_DAYS + 40)
            .map(|i| day(&format!("2020-01-{:02}", (i % 28) + 1), i as i64))
            .collect();
        let oldest_kept = days[40].words;
        let newest = days[days.len() - 1].words;

        let summary = StatsSummary {
            days,
            total_sessions: 9_999,
            total_words: 1_234_567,
            total_dictation_ms: 42,
            first_day: Some("2020-01-01".to_string()),
        };
        let out = publishable(summary);

        assert_eq!(out.days.len(), MAX_PUBLISHED_DAYS);
        // Trimmed from the FRONT: the newest days are the ones the chart needs.
        assert_eq!(out.days[0].words, oldest_kept);
        assert_eq!(out.days[out.days.len() - 1].words, newest);
        assert_eq!(out.total_sessions, 9_999);
        assert_eq!(out.total_words, 1_234_567);
        assert_eq!(out.first_day.as_deref(), Some("2020-01-01"));
    }

    /// A short history is published verbatim.
    #[test]
    fn publishable_leaves_short_history_alone() {
        let summary = StatsSummary {
            days: vec![day("2026-08-01", 10), day("2026-08-02", 20)],
            total_sessions: 2,
            total_words: 30,
            total_dictation_ms: 2000,
            first_day: Some("2026-08-01".to_string()),
        };
        let out = publishable(summary);
        assert_eq!(out.days.len(), 2);
        assert_eq!(out.total_words, 30);
    }

    /// An unparseable or wrong-version blob must not take the whole pull down
    /// with it, and must not be silently read as zeros.
    #[test]
    fn stats_blob_roundtrips_and_rejects_other_versions() {
        let blob = DeviceStats {
            v: 1,
            summary: StatsSummary {
                days: vec![day("2026-08-01", 10)],
                total_sessions: 1,
                total_words: 10,
                total_dictation_ms: 1000,
                first_day: Some("2026-08-01".to_string()),
            },
            published_at: "2026-08-25T00:00:00Z".to_string(),
        };
        let s = serde_json::to_string(&blob).unwrap();
        let back: DeviceStats = serde_json::from_str(&s).unwrap();
        assert_eq!(back.v, 1);
        assert_eq!(back.summary.total_words, 10);

        assert!(serde_json::from_str::<DeviceStats>("{\"nope\":1}").is_err());
        let future = s.replace("\"v\":1", "\"v\":2");
        let parsed: DeviceStats = serde_json::from_str(&future).unwrap();
        assert_ne!(parsed.v, 1, "a v2 blob must not be accepted as v1");
    }

    /// Absent metadata must produce an empty identity, never a panic — most
    /// devices will have no icon assigned for most of their life.
    #[test]
    fn device_meta_defaults_are_empty() {
        let m = DeviceMeta::default();
        assert!(m.icon.is_none() && m.label.is_none());
        assert_eq!(serde_json::to_string(&m).unwrap(), "{}");
        let parsed: DeviceMeta = serde_json::from_str("{}").unwrap();
        assert!(parsed.icon.is_none());
        // Forward-compatible: an unknown field from a newer client is ignored.
        let parsed: DeviceMeta =
            serde_json::from_str("{\"icon\":\"home\",\"colour\":\"red\"}").unwrap();
        assert_eq!(parsed.icon.as_deref(), Some("home"));
    }
}
