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
    /// setting would). Public because the ownership-scoped delete
    /// (`commands::delete_recordings`) needs it to scope the server tombstone
    /// to rows this device originated.
    pub fn device_id(&self) -> String {
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
        // Account purge is checked BEFORE we trust any pulled notes: if another
        // device reset the whole account, we must wipe local state and advance
        // our cursor past the purge marker first, otherwise the pull below would
        // re-seed rows the reset was meant to clear.
        self.apply_purge(&token).await?;
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
        // `devices.user_id` is NOT NULL with no default, and RLS enforces
        // `with check (auth.uid() = user_id)` — so the row must carry our own
        // user id or PostgREST rejects the insert (that rejection is exactly
        // what surfaces as "Sync paused — will retry").
        let user_id = auth::current_user()
            .map(|u| u.user_id)
            .ok_or_else(|| anyhow::anyhow!("device registration: not signed in"))?;
        let url = format!("{}/rest/v1/devices", config::SUPABASE_URL);
        let body = serde_json::json!({
            "id": device_id,
            "user_id": user_id,
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
        // Same NOT NULL + RLS requirement as devices — every note row must
        // carry our user id or the push 4xxs and sync flips to "paused".
        let user_id = auth::current_user()
            .map(|u| u.user_id)
            .ok_or_else(|| anyhow::anyhow!("notes push: not signed in"))?;
        let now = Utc::now().to_rfc3339();
        let rows: Vec<serde_json::Value> = dirty
            .iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "user_id": user_id,
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
        // user_settings.user_id is NOT NULL + RLS-checked too — without it the
        // key push 4xxs (silently, since the caller swallows this), so keys
        // never actually reach the cloud. Include our id like devices/notes.
        let user_id = auth::current_user()
            .map(|u| u.user_id)
            .ok_or_else(|| anyhow::anyhow!("settings sync: not signed in"))?;
        let mut push_rows = Vec::new();
        for (key_name, secret_key) in SETTINGS_KEYS {
            if let Ok(Some(value)) = secrets::get(secret_key) {
                push_rows.push(serde_json::json!({
                    "user_id": user_id,
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

    /// Apply an account-wide purge initiated on any device. The synced marker
    /// is `user_settings['purged_at']` (an RFC3339 UTC timestamp); locally we
    /// remember the last one we applied in `applied_purge_at`. When the server
    /// marker is newer than ours, every local transcript + its audio is wiped
    /// and the pull cursor is jumped to the marker so nothing before the reset
    /// comes back. A fresh install (empty `applied_purge_at`) simply advances
    /// its marker/cursor — wiping empty local state is a harmless no-op.
    ///
    /// `purged_at` is deliberately read here on its own rather than through the
    /// API-key settings loop, so it is never echoed back up or treated as a
    /// round-tripping setting — it is write-on-purge, read-on-sync only.
    async fn apply_purge(&self, token: &str) -> anyhow::Result<()> {
        let url = format!(
            "{}/rest/v1/user_settings?key=eq.purged_at&select=value",
            config::SUPABASE_URL
        );
        let resp = http()
            .get(&url)
            .header("apikey", config::SUPABASE_ANON_KEY)
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("purge check: {e}"))?;
        if !resp.status().is_success() {
            // Never let a purge-marker read failure sink the whole cycle — the
            // normal push/pull is what matters most; we'll re-check next tick.
            return Ok(());
        }
        let rows: Vec<PurgeRow> = resp.json().await.unwrap_or_default();
        let Some(purged_at) = rows.into_iter().next().map(|r| r.value) else {
            return Ok(());
        };
        let applied = self
            .history
            .meta_get("applied_purge_at")?
            .unwrap_or_default();
        // RFC3339 UTC strings sort lexicographically the same as chronologically.
        if purged_at.as_str() > applied.as_str() {
            tracing::info!(purged_at = %purged_at, "sync: applying account purge (wiping local history)");
            self.wipe_all_local();
            self.history.meta_set("applied_purge_at", &purged_at)?;
            self.history.meta_set("pull_cursor", &purged_at)?;
            let _ = self.app.emit("wispr:history_changed", ());
        }
        Ok(())
    }

    /// Initiate an account-wide purge from this device — the one operation
    /// allowed to cross device ownership. Sets the synced `purged_at` marker
    /// FIRST (so the reset still propagates to every other device even if the
    /// bulk server delete below fails), then best-effort hard-DELETEs all of
    /// the user's notes (own rows, other devices' rows, AND orphans whose
    /// originating device is gone), then wipes local state and advances the
    /// local markers so this device doesn't re-apply its own purge on the next
    /// sync. Destructive and irreversible — the UI gates it behind an explicit
    /// press-and-hold + confirm.
    pub async fn purge_all(&self) -> anyhow::Result<()> {
        if !config::is_configured() {
            anyhow::bail!("Sync not configured in this build");
        }
        let user_id = auth::current_user()
            .map(|u| u.user_id)
            .ok_or_else(|| anyhow::anyhow!("purge: not signed in"))?;
        let token = auth::ensure_access_token().await?;
        let now = Utc::now().to_rfc3339();

        // 1. Set the synced marker first. This is the durable signal every
        //    other device reads on sync; if the delete half fails, propagation
        //    still happens.
        let settings_url = format!("{}/rest/v1/user_settings", config::SUPABASE_URL);
        let resp = http()
            .post(&settings_url)
            .header("apikey", config::SUPABASE_ANON_KEY)
            .header("Authorization", format!("Bearer {token}"))
            .header("Prefer", "resolution=merge-duplicates")
            .json(&serde_json::json!({
                "user_id": user_id,
                "key": "purged_at",
                "value": now,
                "updated_at": now,
            }))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("purge marker: {e}"))?;
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("purge marker HTTP failed: {body}");
        }

        // 2. Best-effort hard delete of every note on the account. RLS already
        //    scopes to this user; the explicit `user_id` filter also satisfies
        //    PostgREST's guard against an unfiltered bulk delete.
        let notes_url = format!(
            "{}/rest/v1/notes?user_id=eq.{}",
            config::SUPABASE_URL,
            percent_encode(&user_id),
        );
        if let Err(e) = http()
            .delete(&notes_url)
            .header("apikey", config::SUPABASE_ANON_KEY)
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
        {
            tracing::warn!("purge: server note wipe failed (marker set, will still propagate): {e:#}");
        }

        // 3. Wipe local state and advance our own markers so we treat this
        //    purge as already applied.
        self.wipe_all_local();
        self.history.meta_set("applied_purge_at", &now)?;
        self.history.meta_set("pull_cursor", &now)?;
        let _ = self.app.emit("wispr:history_changed", ());
        Ok(())
    }

    /// Delete every local transcript row + its audio file, then clear the audio
    /// directory outright so nothing orphaned survives on disk. Used by both
    /// `purge_all` (this device initiated) and `apply_purge` (a purge arrived
    /// from another device).
    fn wipe_all_local(&self) {
        if let Ok(recs) = self.history.list_recent(1_000_000) {
            for r in &recs {
                if !r.audio_path.as_os_str().is_empty() {
                    let _ = std::fs::remove_file(&r.audio_path);
                }
                let _ = self.history.delete(&r.id);
            }
        }
        if let Ok(dir) = self.app.path().app_data_dir() {
            let audio = dir.join("audio");
            if audio.is_dir() {
                let _ = std::fs::remove_dir_all(&audio);
                let _ = std::fs::create_dir_all(&audio);
            }
        }
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

/// The single `user_settings` row that carries the account purge marker.
#[derive(Debug, Deserialize)]
struct PurgeRow {
    value: String,
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

/// Best-effort cloud tombstone for an ownership-scoped delete: sets
/// `deleted_at` and blanks the text columns so other devices remove their
/// local copies on their next pull. Scoped server-side to `device_id` so this
/// can only ever tombstone rows THIS device originated — the delete policy is
/// "a client may delete only what it originated", and RLS alone proves a row
/// belongs to the user, never that it belongs to this device. Local deletion
/// proceeds regardless of whether this succeeds — the user's local intent
/// shouldn't be blocked by a network hiccup.
pub async fn tombstone_remote(ids: &[String], device_id: &str) {
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
    let url = format!(
        "{}/rest/v1/notes?id=in.({id_list})&device_id=eq.{}",
        config::SUPABASE_URL,
        percent_encode(device_id),
    );
    // updated_at MUST advance: other devices pull by `updated_at > cursor`, so
    // a tombstone that leaves it untouched would never reach them (the schema
    // has no trigger to bump it). Web and Android tombstones set it too.
    let now = Utc::now().to_rfc3339();
    let body = serde_json::json!({
        "deleted_at": now,
        "updated_at": now,
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
