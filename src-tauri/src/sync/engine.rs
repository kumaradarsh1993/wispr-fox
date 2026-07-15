//! Push/pull sync loop — the desktop half of the protocol in
//! `wispr-fox-web/docs/SYNC_DESIGN.md`. Everything here is background and
//! best-effort: a failed cycle logs a warning, emits `wispr:sync_status`
//! with `state: "error"`, and quietly retries on the next trigger. Nothing
//! in this file ever blocks the dictation hot path — it only runs from
//! spawned async tasks kicked off after a recording completes, at launch,
//! and on a ~60s background interval.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::flow::Flow;
use crate::history::{History, RemoteNote};
use crate::secrets::{self, SecretKey};

use super::{auth, config};

const POLL_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncState {
    Idle,
    Syncing,
    Error,
    SignedOut,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncStatusEvent {
    pub state: SyncState,
    pub last_synced_at: Option<String>,
}

/// Managed as Tauri state. All fields are `Arc`/`Clone`-cheap so this can be
/// cloned freely into spawned tasks.
#[derive(Clone)]
pub struct SyncEngine {
    history: History,
    flow: Flow,
    app: AppHandle,
    last_synced_at: Arc<Mutex<Option<chrono::DateTime<Utc>>>>,
    /// Guards against overlapping sync cycles (e.g. the 60s tick firing
    /// while a post-recording sync from the same moment is still running).
    running: Arc<AtomicBool>,
}

static HTTP: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
fn http() -> reqwest::Client {
    HTTP.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .connect_timeout(Duration::from_secs(6))
            .build()
            .expect("reqwest client construction is infallible with default config")
    })
    .clone()
}

const SETTINGS_KEYS: [(&str, SecretKey); 5] = [
    ("key_deepgram", SecretKey::DeepgramStt),
    ("key_groq", SecretKey::GroqStt),
    ("key_openai", SecretKey::OpenAiStt),
    ("key_elevenlabs", SecretKey::ElevenLabsStt),
    ("key_gemini", SecretKey::GeminiLlm),
];

impl SyncEngine {
    pub fn new(history: History, flow: Flow, app: AppHandle) -> Self {
        Self {
            history,
            flow,
            app,
            last_synced_at: Arc::new(Mutex::new(None)),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    fn emit_status(&self, state: SyncState) {
        let last_synced_at = self.last_synced_at.lock().map(|t| t.to_rfc3339());
        let _ = self.app.emit(
            "wispr:sync_status",
            SyncStatusEvent { state, last_synced_at },
        );
    }

    /// This install's persistent device id — generated once and stored in
    /// `sync_meta` (survives across settings.json rewrites, unlike a plain
    /// setting would).
    fn device_id(&self) -> String {
        if let Ok(Some(id)) = self.history.meta_get("device_id") {
            return id;
        }
        let id = uuid::Uuid::new_v4().to_string();
        let _ = self.history.meta_set("device_id", &id);
        id
    }

    /// Run one push+pull cycle if signed in; otherwise just emit
    /// `signed_out` and return. Safe to call as often as you like — an
    /// already-running cycle is skipped rather than overlapped.
    pub async fn sync_once(&self) {
        if !config::is_configured() {
            self.emit_status(SyncState::SignedOut);
            return;
        }
        if auth::current_user().is_none() {
            self.emit_status(SyncState::SignedOut);
            return;
        }
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            tracing::debug!("sync: cycle already in progress, skipping this trigger");
            return;
        }
        self.emit_status(SyncState::Syncing);
        let result = self.run_sync().await;
        self.running.store(false, Ordering::SeqCst);
        match result {
            Ok(()) => {
                *self.last_synced_at.lock() = Some(Utc::now());
                self.emit_status(SyncState::Idle);
            }
            Err(e) => {
                tracing::warn!("sync cycle failed (non-fatal, will retry): {e:#}");
                self.emit_status(SyncState::Error);
            }
        }
    }

    async fn run_sync(&self) -> anyhow::Result<()> {
        let token = auth::ensure_access_token().await?;
        self.register_device(&token).await?;
        self.push_notes(&token).await?;
        self.pull_notes(&token).await?;
        // API-key sync is a nice-to-have layered on top of the notes sync
        // that actually matters — never let a settings hiccup fail the
        // whole cycle (which would also stop notes retrying next time).
        if let Err(e) = self.sync_settings(&token).await {
            tracing::warn!("sync: settings (API key) sync failed (non-fatal): {e:#}");
        }
        Ok(())
    }

    async fn register_device(&self, token: &str) -> anyhow::Result<()> {
        let device_id = self.device_id();
        let device_name = self.flow.settings().device_name;
        let url = format!("{}/rest/v1/devices", config::SUPABASE_URL);
        let body = serde_json::json!({
            "id": device_id,
            "name": device_name,
            "platform": "desktop",
            "last_seen_at": Utc::now().to_rfc3339(),
        });
        let resp = http()
            .post(&url)
            .header("apikey", config::SUPABASE_ANON_KEY)
            .header("Authorization", format!("Bearer {token}"))
            .header("Prefer", "resolution=merge-duplicates")
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("device registration: {e}"))?;
        if !resp.status().is_success() {
            anyhow::bail!("device registration HTTP {}", resp.status().as_u16());
        }
        Ok(())
    }

    async fn push_notes(&self, token: &str) -> anyhow::Result<()> {
        let dirty = self.history.list_dirty()?;
        if dirty.is_empty() {
            return Ok(());
        }
        let device_id = self.device_id();
        let now = Utc::now().to_rfc3339();
        let rows: Vec<serde_json::Value> = dirty
            .iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "device_id": device_id,
                    "platform": "desktop",
                    "origin": if r.source == "upload" { "upload" } else { "mic" },
                    "title": r.title,
                    "transcript": r.transcript,
                    "cleaned_text": r.cleaned_text,
                    "drafted_text": r.drafted_text,
                    "duration_ms": r.duration_ms,
                    "stt_provider": r.stt_provider,
                    "llm_provider": r.llm_provider,
                    "created_at": r.created_at.to_rfc3339(),
                    "updated_at": now,
                })
            })
            .collect();

        let url = format!("{}/rest/v1/notes", config::SUPABASE_URL);
        let resp = http()
            .post(&url)
            .header("apikey", config::SUPABASE_ANON_KEY)
            .header("Authorization", format!("Bearer {token}"))
            .header("Prefer", "resolution=merge-duplicates")
            .json(&rows)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("notes push: {e}"))?;
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("notes push HTTP failed: {body}");
        }
        let ids: Vec<String> = dirty.into_iter().map(|r| r.id).collect();
        self.history.mark_clean(&ids)?;
        Ok(())
    }

    async fn pull_notes(&self, token: &str) -> anyhow::Result<()> {
        let cursor = self
            .history
            .meta_get("pull_cursor")?
            .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string());
        let my_device_id = self.device_id();
        let url = format!(
            "{}/rest/v1/notes?updated_at=gt.{}&order=updated_at.asc",
            config::SUPABASE_URL,
            percent_encode(&cursor),
        );
        let resp = http()
            .get(&url)
            .header("apikey", config::SUPABASE_ANON_KEY)
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("notes pull: {e}"))?;
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("notes pull HTTP failed: {body}");
        }
        let rows: Vec<RemoteNoteRow> = resp
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("decoding notes pull response: {e}"))?;

        let mut max_updated = cursor;
        let mut changed = false;
        for row in rows {
            if row.updated_at.as_str() > max_updated.as_str() {
                max_updated = row.updated_at.clone();
            }
            // Our own row echoing back from a prior push — we're already
            // the authoritative local copy, nothing to do.
            if row.device_id == my_device_id {
                continue;
            }
            if self.history.exclusion_contains(&row.id)? {
                continue;
            }
            if row.deleted_at.is_some() {
                self.history.apply_tombstone(&row.id)?;
                changed = true;
                continue;
            }
            let note = RemoteNote {
                id: row.id,
                device_id: row.device_id,
                platform: row.platform,
                device_name: row.device_name,
                title: row.title,
                transcript: row.transcript,
                cleaned_text: row.cleaned_text,
                drafted_text: row.drafted_text,
                duration_ms: row.duration_ms,
                stt_provider: row.stt_provider,
                llm_provider: row.llm_provider,
                created_at: row.created_at,
                updated_at: row.updated_at,
                deleted_at: row.deleted_at,
            };
            if self.history.upsert_remote(&note)? {
                changed = true;
            }
        }
        self.history.meta_set("pull_cursor", &max_updated)?;
        if changed {
            let _ = self.app.emit("wispr:history_changed", ());
        }
        Ok(())
    }

    /// Push locally-saved API keys and pull any newer ones from other
    /// devices. Deliberately lower-stakes than the notes sync above — a
    /// failure here is logged and swallowed by the caller so it never blocks
    /// transcript sync.
    async fn sync_settings(&self, token: &str) -> anyhow::Result<()> {
        let mut push_rows = Vec::new();
        for (key_name, secret_key) in SETTINGS_KEYS {
            if let Ok(Some(value)) = secrets::get(secret_key) {
                push_rows.push(serde_json::json!({
                    "key": key_name,
                    "value": value,
                    "updated_at": Utc::now().to_rfc3339(),
                }));
            }
        }
        if !push_rows.is_empty() {
            let url = format!("{}/rest/v1/user_settings", config::SUPABASE_URL);
            let _ = http()
                .post(&url)
                .header("apikey", config::SUPABASE_ANON_KEY)
                .header("Authorization", format!("Bearer {token}"))
                .header("Prefer", "resolution=merge-duplicates")
                .json(&push_rows)
                .send()
                .await;
        }

        let url = format!("{}/rest/v1/user_settings", config::SUPABASE_URL);
        let resp = http()
            .get(&url)
            .header("apikey", config::SUPABASE_ANON_KEY)
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("settings pull: {e}"))?;
        if !resp.status().is_success() {
            return Ok(());
        }
        let rows: Vec<SettingsRow> = resp.json().await.unwrap_or_default();
        for row in rows {
            let Some((_, secret_key)) = SETTINGS_KEYS.iter().find(|(k, _)| *k == row.key) else {
                continue;
            };
            let cursor_key = format!("settings_cursor:{}", row.key);
            let last = self.history.meta_get(&cursor_key)?.unwrap_or_default();
            if row.updated_at.as_str() > last.as_str() {
                let _ = secrets::set(*secret_key, &row.value);
                self.history.meta_set(&cursor_key, &row.updated_at)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct RemoteNoteRow {
    id: String,
    device_id: String,
    #[serde(default = "default_platform")]
    platform: String,
    #[serde(default)]
    device_name: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    transcript: Option<String>,
    #[serde(default)]
    cleaned_text: Option<String>,
    #[serde(default)]
    drafted_text: Option<String>,
    #[serde(default)]
    duration_ms: i64,
    #[serde(default)]
    stt_provider: Option<String>,
    #[serde(default)]
    llm_provider: Option<String>,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    deleted_at: Option<String>,
}

fn default_platform() -> String {
    "desktop".to_string()
}

#[derive(Debug, Deserialize)]
struct SettingsRow {
    key: String,
    value: String,
    updated_at: String,
}

fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Fire-and-forget trigger called after a recording pipeline finishes. A
/// complete no-op unless sync is configured AND the user is signed in —
/// never adds latency to the dictation path since it's only ever called
/// after `Status::Done` is already persisted.
pub fn notify_recording_done(app: &AppHandle) {
    if !config::is_configured() || auth::current_user().is_none() {
        return;
    }
    let Some(engine) = app.try_state::<SyncEngine>() else {
        return;
    };
    let engine = engine.inner().clone();
    tauri::async_runtime::spawn(async move {
        engine.sync_once().await;
    });
}

/// Spawn the ~60s background poll. Started once at launch; inert (each tick
/// just emits `signed_out` and returns) until the user signs in.
pub fn spawn_background_poll(engine: SyncEngine) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(POLL_INTERVAL);
        loop {
            interval.tick().await;
            engine.sync_once().await;
        }
    });
}

/// Best-effort cloud tombstone for an "everywhere" delete: sets `deleted_at`
/// and blanks the text columns so other devices remove their local copies on
/// their next pull. Local deletion proceeds regardless of whether this
/// succeeds — the user's local intent shouldn't be blocked by a network
/// hiccup, and the exclusion list (for "this device only" deletes) has no
/// bearing here since this is the "delete everywhere" path.
pub async fn tombstone_remote(ids: &[String]) {
    if ids.is_empty() || !config::is_configured() || auth::current_user().is_none() {
        return;
    }
    let Ok(token) = auth::ensure_access_token().await else {
        return;
    };
    let id_list = ids
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let url = format!("{}/rest/v1/notes?id=in.({id_list})", config::SUPABASE_URL);
    let body = serde_json::json!({
        "deleted_at": Utc::now().to_rfc3339(),
        "title": null,
        "transcript": null,
        "cleaned_text": null,
        "drafted_text": null,
    });
    if let Err(e) = http()
        .patch(&url)
        .header("apikey", config::SUPABASE_ANON_KEY)
        .header("Authorization", format!("Bearer {token}"))
        .json(&body)
        .send()
        .await
    {
        tracing::warn!("sync: cloud tombstone failed (deleted locally regardless): {e:#}");
    }
}
