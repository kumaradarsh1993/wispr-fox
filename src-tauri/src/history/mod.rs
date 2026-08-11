//! SQLite-backed recording history.
//!
//! Schema is denormalised: one row per recording, lifecycle tracked via the
//! `status` column. Audio file lives next to the row at `audio_path`. The GC
//! task purges rows + files older than `retention_days` on its hourly tick.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::llm::ClippyMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Recording,
    Transcribing,
    Cleaning,
    Injecting,
    Done,
    Error,
}

/// Which "alt" output column to write into — the cleaned-raw column or
/// the drafted column. Lets the same history setter route to either
/// destination based on the mode that produced the output.
#[derive(Debug, Clone, Copy)]
pub enum AltKind {
    Cleaned,
    Drafted,
    MeetingNotes,
}

impl Status {
    fn as_str(&self) -> &'static str {
        match self {
            Status::Recording => "recording",
            Status::Transcribing => "transcribing",
            Status::Cleaning => "cleaning",
            Status::Injecting => "injecting",
            Status::Done => "done",
            Status::Error => "error",
        }
    }

    fn parse(s: &str) -> Status {
        match s {
            "recording" => Status::Recording,
            "transcribing" => Status::Transcribing,
            "cleaning" => Status::Cleaning,
            "injecting" => Status::Injecting,
            "done" => Status::Done,
            _ => Status::Error,
        }
    }
}

fn mode_str(m: ClippyMode) -> &'static str {
    match m {
        ClippyMode::Light => "light",
        ClippyMode::Advanced => "advanced",
        ClippyMode::Drafting => "drafting",
    }
}

fn mode_parse(s: &str) -> ClippyMode {
    match s {
        "advanced" => ClippyMode::Advanced,
        "drafting" => ClippyMode::Drafting,
        _ => ClippyMode::Light,
    }
}

/// One day's rolled-up dictation stats. Lives in its own `daily_stats` table
/// that is incremented once per completed recording and is NEVER pruned by the
/// retention GC — so the analytics dashboard shows LIFETIME numbers even after
/// the underlying audio + history rows have aged out (7-day default retention).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyStat {
    /// Local calendar date, "YYYY-MM-DD".
    pub date: String,
    pub sessions: i64,
    pub words: i64,
    pub dictation_ms: i64,
    /// Sessions run in Light/Advanced (cleanup) modes.
    pub light_count: i64,
    /// Sessions run in Drafting mode.
    pub draft_count: i64,
}

/// Aggregate analytics payload for the dashboard: per-day rows (ascending by
/// date) plus lifetime totals. Tiny — one row per active day.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsSummary {
    pub days: Vec<DailyStat>,
    pub total_sessions: i64,
    pub total_words: i64,
    pub total_dictation_ms: i64,
    /// First day with any recorded activity ("YYYY-MM-DD"), or None if empty.
    pub first_day: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recording {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub audio_path: PathBuf,
    pub duration_ms: i64,
    pub mode: ClippyMode,
    pub status: Status,
    pub transcript: Option<String>,
    pub cleaned_text: Option<String>,
    /// "Drafted" version — output of the drafting/elaboration prompt (F9).
    /// Separate from `cleaned_text` (the moderate "cleaned raw" output)
    /// so the user can have BOTH a cleaned version and a drafted version
    /// of the same recording on demand.
    pub drafted_text: Option<String>,
    pub meeting_notes_text: Option<String>,
    /// Versioned JSON envelope containing provider-normalized speaker turns.
    pub speaker_turns: Option<String>,
    /// JSON object mapping stable placeholders ("Speaker 1") to user names.
    pub speaker_names: Option<String>,
    pub is_meeting: bool,
    pub diarization_enabled: bool,
    pub stt_provider: Option<String>,
    pub llm_provider: Option<String>,
    pub clippy_used: bool,
    pub clippy_note: Option<String>,
    pub retry_count: i32,
    pub error: Option<String>,
    /// LLM-generated one-line descriptor ("what did I talk about here").
    /// Filled asynchronously after the pipeline completes; None until then
    /// (or forever, if auto-titling is off / the call failed).
    pub title: Option<String>,
    /// Wall-clock milliseconds the STT (speech-to-text) request took, start
    /// to finish. This is the single most useful debugging number: a slow
    /// transcription shows up here directly. None until STT completes.
    pub stt_ms: Option<i64>,
    /// Wall-clock milliseconds the LLM cleanup/draft request took. None when
    /// cleanup didn't run (raw-only) or hasn't finished.
    pub cleanup_ms: Option<i64>,
    /// End-to-end turnaround in ms — from the moment recording stopped to the
    /// moment text was delivered. Lets the user see "20s total, 19s of it STT".
    pub total_ms: Option<i64>,
    /// Compact JSON array of timeline events, `[{"ms":123,"msg":"…"}]`, where
    /// `ms` is elapsed-since-pipeline-start. A per-recording flight recorder so
    /// a slow/failed run can be diagnosed after the fact. None on old rows.
    pub event_log: Option<String>,
    /// Milliseconds of audio actually captured to the WAV (from the real sample
    /// count). Compared against `duration_ms` (wall-clock) to detect a mic that
    /// dropped mid-recording: if this is materially smaller, the transcript is
    /// truncated because the audio itself is. None on old rows.
    pub audio_captured_ms: Option<i64>,
    /// How this recording entered the app: `"mic"` for a live dictation (the
    /// default, and what old NULL rows map to) or `"upload"` for a user-supplied
    /// audio file dragged in or picked from disk. Drives the "Uploaded" badge.
    pub source: String,
    /// Which client produced this recording: `"desktop"` / `"web"` / `"mobile"`.
    /// NULL (pre-sync rows, or genuinely unknown) reads as `"desktop"` — this
    /// build only ever creates local rows on desktop.
    pub platform: String,
    /// Human-readable device name at the time this recording was made (from
    /// the synced device's `device_name` setting). None on old rows and on
    /// remote rows pulled before a device ever set a name.
    pub device_name: Option<String>,
    /// True when this row has local edits that haven't been pushed to the
    /// cloud yet. Only rows with `status = done` and `dirty = true` are
    /// eligible for the next push.
    pub dirty: bool,
    /// True when this row was pulled down from another device via sync — its
    /// `audio_path` will not exist locally (audio never syncs), so playback
    /// UI must hide/disable itself for these rows.
    pub remote: bool,
}

#[derive(Clone)]
pub struct History {
    inner: Arc<Mutex<Connection>>,
}

impl History {
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating db dir {parent:?}"))?;
        }
        let conn = Connection::open(db_path)
            .with_context(|| format!("opening sqlite at {db_path:?}"))?;
        conn.pragma_update(None, "journal_mode", "WAL").ok();
        conn.pragma_update(None, "foreign_keys", "ON").ok();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS recordings (
              id            TEXT PRIMARY KEY,
              created_at    TEXT NOT NULL,
              audio_path    TEXT NOT NULL,
              duration_ms   INTEGER NOT NULL DEFAULT 0,
              mode          TEXT NOT NULL,
              status        TEXT NOT NULL,
              transcript    TEXT,
              cleaned_text  TEXT,
              drafted_text  TEXT,
              meeting_notes_text TEXT,
              speaker_turns TEXT,
              speaker_names TEXT,
              is_meeting INTEGER NOT NULL DEFAULT 0,
              diarization_enabled INTEGER NOT NULL DEFAULT 0,
              stt_provider  TEXT,
              llm_provider  TEXT,
              clippy_used   INTEGER NOT NULL DEFAULT 0,
              clippy_note   TEXT,
              retry_count   INTEGER NOT NULL DEFAULT 0,
              error         TEXT,
              title         TEXT,
              stt_ms        INTEGER,
              cleanup_ms    INTEGER,
              total_ms      INTEGER,
              event_log     TEXT,
              audio_captured_ms INTEGER
            );
            CREATE INDEX IF NOT EXISTS recordings_created_at_idx
              ON recordings(created_at);

            -- Lifetime analytics rollup. One row per LOCAL calendar day,
            -- incremented on each completed recording. Deliberately NOT touched
            -- by the retention GC so the stats dashboard survives the 7-day
            -- audio/row purge (and app updates — it's in the same persisted DB).
            CREATE TABLE IF NOT EXISTS daily_stats (
              date          TEXT PRIMARY KEY,
              sessions      INTEGER NOT NULL DEFAULT 0,
              words         INTEGER NOT NULL DEFAULT 0,
              dictation_ms  INTEGER NOT NULL DEFAULT 0,
              light_count   INTEGER NOT NULL DEFAULT 0,
              draft_count   INTEGER NOT NULL DEFAULT 0
            );

            -- v3.0.0: accounts + cross-device sync. `sync_meta` is a tiny KV
            -- store for the device id + pull cursor; `sync_exclusions` records
            -- ids the user deleted "this device only" so a later pull doesn't
            -- resurrect them from the cloud copy.
            CREATE TABLE IF NOT EXISTS sync_meta (
              key   TEXT PRIMARY KEY,
              value TEXT
            );
            CREATE TABLE IF NOT EXISTS sync_exclusions (
              id TEXT PRIMARY KEY
            );
            "#,
        )?;

        // Idempotent migrations for users upgrading from ≤0.1.1 (which
        // had no `drafted_text` column). ALTER will error with "duplicate
        // column name" if the column already exists — that's expected on
        // every launch after the first, so we ignore the result.
        let _ = conn.execute("ALTER TABLE recordings ADD COLUMN drafted_text TEXT", []);
        let _ = conn.execute("ALTER TABLE recordings ADD COLUMN meeting_notes_text TEXT", []);
        let _ = conn.execute("ALTER TABLE recordings ADD COLUMN speaker_turns TEXT", []);
        let _ = conn.execute("ALTER TABLE recordings ADD COLUMN speaker_names TEXT", []);
        let _ = conn.execute("ALTER TABLE recordings ADD COLUMN is_meeting INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE recordings ADD COLUMN diarization_enabled INTEGER NOT NULL DEFAULT 0", []);
        // v2.1.0: LLM-generated one-line name for each recording (auto-title).
        let _ = conn.execute("ALTER TABLE recordings ADD COLUMN title TEXT", []);
        // v2.1.0-nightly.7: per-recording timing + event-log flight recorder,
        // so slow/failed transcriptions are diagnosable from the (i) inspector.
        let _ = conn.execute("ALTER TABLE recordings ADD COLUMN stt_ms INTEGER", []);
        let _ = conn.execute("ALTER TABLE recordings ADD COLUMN cleanup_ms INTEGER", []);
        let _ = conn.execute("ALTER TABLE recordings ADD COLUMN total_ms INTEGER", []);
        let _ = conn.execute("ALTER TABLE recordings ADD COLUMN event_log TEXT", []);
        // v2.1.0-nightly.8: actual audio captured (sample-count derived), vs the
        // wall-clock duration_ms — divergence = a mic that dropped mid-recording.
        let _ = conn.execute(
            "ALTER TABLE recordings ADD COLUMN audio_captured_ms INTEGER",
            [],
        );
        // v2.2.0: origin of the recording — "mic" (live dictation) or "upload"
        // (user-supplied audio file). NULL on pre-upgrade rows → treated as
        // "mic" on read, so old dictations keep behaving as before.
        let _ = conn.execute("ALTER TABLE recordings ADD COLUMN source TEXT", []);

        // v3.0.0: accounts + cross-device sync columns. NULL/0 on pre-upgrade
        // rows is the correct default: they were all made on this desktop
        // (platform → "desktop" on read), have no device name recorded, are
        // not yet marked dirty (nothing to push until the user signs in —
        // first sign-in explicitly marks every `done` row dirty), and are not
        // remote (they're local rows, obviously — `remote` only gets set on
        // rows pulled down from another device).
        let _ = conn.execute("ALTER TABLE recordings ADD COLUMN platform TEXT", []);
        let _ = conn.execute("ALTER TABLE recordings ADD COLUMN device_name TEXT", []);
        let _ = conn.execute(
            "ALTER TABLE recordings ADD COLUMN dirty INTEGER NOT NULL DEFAULT 0",
            [],
        );
        let _ = conn.execute("ALTER TABLE recordings ADD COLUMN remote INTEGER", []);

        // Repair for the `mark_all_done_dirty` defect shipped in v3.0.0: it
        // marked PULLED rows dirty as well as local ones. `list_dirty` refuses
        // to push remote rows, so `mark_clean` never cleared them and they were
        // stuck dirty forever — and `upsert_remote` skips locally-dirty rows,
        // so those transcripts stopped accepting any further cloud update,
        // tombstones included (a delete on the owning device never reached this
        // one). `dirty` has no meaning on a remote row: it exists solely to say
        // "this device has local work to push", and by definition a remote row
        // has none. Clearing it is therefore always safe, and idempotent.
        let _ = conn.execute(
            "UPDATE recordings SET dirty = 0 WHERE COALESCE(remote, 0) = 1 AND dirty = 1",
            [],
        );

        // For existing rows where the user pressed F9 (drafting): their
        // drafted output landed in `cleaned_text` under the old single-
        // output schema. Move it into `drafted_text` so the new history
        // UI shows it under the "Drafted" tab instead of "Cleaned".
        // Idempotent — only acts on rows that haven't been migrated yet.
        let migrated = conn.execute(
            r#"UPDATE recordings
               SET drafted_text = cleaned_text,
                   cleaned_text = NULL
               WHERE mode = 'drafting'
                 AND drafted_text IS NULL
                 AND cleaned_text IS NOT NULL"#,
            [],
        ).unwrap_or(0);
        if migrated > 0 {
            tracing::info!(rows = migrated, "history: migrated drafting rows to drafted_text column");
        }

        Ok(Self { inner: Arc::new(Mutex::new(conn)) })
    }

    /// `device_name` is this desktop install's current display name (from
    /// Settings → Account, or the hostname default) — stamped onto every new
    /// row alongside `platform = "desktop"` so History/Settings can show
    /// "Desktop · <name>" badges once sync is in play. Old rows (before this
    /// column existed) stay NULL and fall back to plain "Desktop" on read.
    pub fn insert_new(
        &self,
        audio_path: &Path,
        mode: ClippyMode,
        device_name: &str,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let conn = self.inner.lock();
        conn.execute(
            r#"INSERT INTO recordings
               (id, created_at, audio_path, mode, status, platform, device_name)
               VALUES (?1, ?2, ?3, ?4, ?5, 'desktop', ?6)"#,
            params![
                id,
                now,
                audio_path.to_string_lossy(),
                mode_str(mode),
                Status::Recording.as_str(),
                device_name,
            ],
        )?;
        Ok(id)
    }

    /// Insert a row for a user-uploaded audio file. Unlike `insert_new` it
    /// starts in `Transcribing` (there is no live recording phase) and marks
    /// `source = "upload"` so the History UI shows an "Uploaded" badge.
    pub fn insert_upload(&self, audio_path: &Path, mode: ClippyMode, device_name: &str) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let conn = self.inner.lock();
        conn.execute(
            r#"INSERT INTO recordings
               (id, created_at, audio_path, mode, status, source, platform, device_name)
               VALUES (?1, ?2, ?3, ?4, ?5, 'upload', 'desktop', ?6)"#,
            params![
                id,
                now,
                audio_path.to_string_lossy(),
                mode_str(mode),
                Status::Transcribing.as_str(),
                device_name,
            ],
        )?;
        Ok(id)
    }

    pub fn update_status(&self, id: &str, status: Status) -> Result<()> {
        let conn = self.inner.lock();
        // When transitioning out of an error state (e.g. retry path),
        // clear the stale error text. Otherwise a successful retry would
        // leave the previous failure message in the row, and any UI that
        // surfaces `error` (info popover, telemetry) would show stale data.
        //
        // Reaching `done` is also a sync trigger: mark the row dirty so the
        // next sync cycle pushes it. Only terminal `done` rows are ever
        // synced (per SYNC_DESIGN.md), so no other status flips the flag.
        if matches!(status, Status::Done) {
            conn.execute(
                "UPDATE recordings SET status = ?1, error = NULL, dirty = 1 WHERE id = ?2",
                params![status.as_str(), id],
            )?;
        } else if !matches!(status, Status::Error) {
            conn.execute(
                "UPDATE recordings SET status = ?1, error = NULL WHERE id = ?2",
                params![status.as_str(), id],
            )?;
        } else {
            conn.execute(
                "UPDATE recordings SET status = ?1 WHERE id = ?2",
                params![status.as_str(), id],
            )?;
        }
        Ok(())
    }

    pub fn set_duration(&self, id: &str, duration_ms: i64, captured_ms: i64) -> Result<()> {
        let conn = self.inner.lock();
        conn.execute(
            "UPDATE recordings SET duration_ms = ?1, audio_captured_ms = ?2 WHERE id = ?3",
            params![duration_ms, captured_ms, id],
        )?;
        Ok(())
    }

    pub fn set_transcript(
        &self,
        id: &str,
        transcript: &str,
        provider: &str,
    ) -> Result<()> {
        let conn = self.inner.lock();
        conn.execute(
            r#"UPDATE recordings
               SET transcript = ?1, stt_provider = ?2, dirty = 1
               WHERE id = ?3"#,
            params![transcript, provider, id],
        )?;
        Ok(())
    }

    pub fn set_cleaned(
        &self,
        id: &str,
        cleaned: &str,
        provider: Option<&str>,
        used: bool,
        note: Option<&str>,
    ) -> Result<()> {
        self.set_alt(id, AltKind::Cleaned, cleaned, provider, used, note)
    }

    /// Generic setter for either the cleaned or drafted output column.
    /// Backwards-compat alias `set_cleaned` keeps the old call sites green
    /// while new code uses `set_alt(Drafted, ...)` to route to the right
    /// column for the new 3-version history UI.
    pub fn set_alt(
        &self,
        id: &str,
        kind: AltKind,
        text: &str,
        provider: Option<&str>,
        used: bool,
        note: Option<&str>,
    ) -> Result<()> {
        let conn = self.inner.lock();
        match kind {
            AltKind::Cleaned => conn.execute(
                r#"UPDATE recordings
                   SET cleaned_text = ?1, llm_provider = ?2,
                       clippy_used = ?3, clippy_note = ?4,
                       dirty = CASE WHEN COALESCE(remote, 0) = 1 THEN 0 ELSE 1 END
                   WHERE id = ?5"#,
                params![text, provider, used as i32, note, id],
            )?,
            AltKind::Drafted => conn.execute(
                r#"UPDATE recordings
                   SET drafted_text = ?1, llm_provider = ?2,
                       clippy_used = ?3, clippy_note = ?4,
                       dirty = CASE WHEN COALESCE(remote, 0) = 1 THEN 0 ELSE 1 END
                   WHERE id = ?5"#,
                params![text, provider, used as i32, note, id],
            )?,
            AltKind::MeetingNotes => conn.execute(
                r#"UPDATE recordings
                   SET meeting_notes_text = ?1, llm_provider = ?2,
                       clippy_used = ?3, clippy_note = ?4, is_meeting = 1,
                       dirty = CASE WHEN COALESCE(remote, 0) = 1 THEN 0 ELSE 1 END
                   WHERE id = ?5"#,
                params![text, provider, used as i32, note, id],
            )?,
        };
        Ok(())
    }

    pub fn set_meeting_metadata(
        &self,
        id: &str,
        is_meeting: bool,
        diarization_enabled: bool,
        turns_json: Option<&str>,
    ) -> Result<()> {
        let conn = self.inner.lock();
        conn.execute(
            r#"UPDATE recordings SET is_meeting = ?1, diarization_enabled = ?2,
               speaker_turns = ?3,
               dirty = CASE WHEN COALESCE(remote, 0) = 1 THEN 0 ELSE 1 END
               WHERE id = ?4"#,
            params![is_meeting as i32, diarization_enabled as i32, turns_json, id],
        )?;
        Ok(())
    }

    pub fn set_speaker_names(&self, id: &str, names_json: &str) -> Result<()> {
        let conn = self.inner.lock();
        conn.execute(
            "UPDATE recordings SET speaker_names = ?1, \
             dirty = CASE WHEN COALESCE(remote, 0) = 1 THEN 0 ELSE 1 END \
             WHERE id = ?2",
            params![names_json, id],
        )?;
        Ok(())
    }

    pub fn set_title(&self, id: &str, title: &str) -> Result<()> {
        let conn = self.inner.lock();
        conn.execute(
            "UPDATE recordings SET title = ?1, dirty = 1 WHERE id = ?2",
            params![title, id],
        )?;
        Ok(())
    }

    /// Persist the per-recording timing breakdown + event-log JSON. Written
    /// once near the end of the pipeline (and on the error path) so the (i)
    /// inspector can show "STT 19.2s / cleanup 0.9s / total 20.4s" plus the
    /// timeline. All-optional so partial data (e.g. STT timed out, no cleanup)
    /// still records what we know.
    pub fn set_timings(
        &self,
        id: &str,
        stt_ms: Option<i64>,
        cleanup_ms: Option<i64>,
        total_ms: Option<i64>,
        event_log: &str,
    ) -> Result<()> {
        let conn = self.inner.lock();
        conn.execute(
            r#"UPDATE recordings
               SET stt_ms = ?1, cleanup_ms = ?2, total_ms = ?3, event_log = ?4
               WHERE id = ?5"#,
            params![stt_ms, cleanup_ms, total_ms, event_log, id],
        )?;
        Ok(())
    }

    pub fn set_error(&self, id: &str, error: &str) -> Result<()> {
        let conn = self.inner.lock();
        conn.execute(
            r#"UPDATE recordings
               SET status = ?1, error = ?2
               WHERE id = ?3"#,
            params![Status::Error.as_str(), error, id],
        )?;
        Ok(())
    }

    pub fn bump_retry(&self, id: &str) -> Result<()> {
        let conn = self.inner.lock();
        conn.execute(
            "UPDATE recordings SET retry_count = retry_count + 1 WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<Option<Recording>> {
        let conn = self.inner.lock();
        let mut stmt = conn.prepare(SELECT_ALL_COLUMNS_BY_ID)?;
        let row = stmt
            .query_row(params![id], row_to_recording)
            .optional()?;
        Ok(row)
    }

    pub fn list_recent(&self, limit: i64) -> Result<Vec<Recording>> {
        let conn = self.inner.lock();
        let mut stmt = conn.prepare(&format!(
            "{SELECT_ALL_COLUMNS} ORDER BY created_at DESC LIMIT ?1"
        ))?;
        let rows = stmt
            .query_map(params![limit], row_to_recording)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Returns ids + audio_paths whose created_at is older than `older_than`.
    /// Caller deletes the rows + files; we don't touch disk here.
    pub fn list_purgeable(&self, older_than: DateTime<Utc>) -> Result<Vec<(String, PathBuf)>> {
        let conn = self.inner.lock();
        let mut stmt = conn.prepare(
            "SELECT id, audio_path FROM recordings WHERE created_at < ?1",
        )?;
        let rows = stmt
            .query_map(params![older_than.to_rfc3339()], |r| {
                Ok((r.get::<_, String>(0)?, PathBuf::from(r.get::<_, String>(1)?)))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let conn = self.inner.lock();
        conn.execute("DELETE FROM recordings WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Look up the recording id that owns a given audio file path. Used by
    /// the storage-cap GC sweep — when it picks an oldest audio file to
    /// delete on disk, it also needs the row id so it can drop the history
    /// row, otherwise History fills with rows pointing at missing files.
    pub fn find_by_audio_path(&self, audio_path: &Path) -> Result<Option<String>> {
        let conn = self.inner.lock();
        let mut stmt =
            conn.prepare("SELECT id FROM recordings WHERE audio_path = ?1 LIMIT 1")?;
        let mut rows = stmt
            .query_map(params![audio_path.to_string_lossy()], |r| {
                r.get::<_, String>(0)
            })?;
        match rows.next() {
            Some(Ok(id)) => Ok(Some(id)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    /// Increment the lifetime daily-stats rollup for one completed recording.
    /// `date` is the LOCAL calendar day ("YYYY-MM-DD"); `mode` decides whether
    /// it counts toward the light/cleanup bucket or the drafting bucket. Called
    /// exactly once per recording from the flow pipeline (not on retry — that
    /// would double-count an already-tallied session).
    pub fn record_session(
        &self,
        date: &str,
        words: i64,
        dictation_ms: i64,
        mode: ClippyMode,
    ) -> Result<()> {
        let (light_delta, draft_delta) = match mode {
            ClippyMode::Drafting => (0i64, 1i64),
            _ => (1i64, 0i64),
        };
        let conn = self.inner.lock();
        conn.execute(
            r#"INSERT INTO daily_stats
                 (date, sessions, words, dictation_ms, light_count, draft_count)
               VALUES (?1, 1, ?2, ?3, ?4, ?5)
               ON CONFLICT(date) DO UPDATE SET
                 sessions     = sessions + 1,
                 words        = words + ?2,
                 dictation_ms = dictation_ms + ?3,
                 light_count  = light_count + ?4,
                 draft_count  = draft_count + ?5"#,
            params![date, words, dictation_ms, light_delta, draft_delta],
        )?;
        Ok(())
    }

    /// Read the full lifetime stats rollup: every active day (ascending) plus
    /// totals. Cheap — one row per day.
    pub fn stats_summary(&self) -> Result<StatsSummary> {
        let conn = self.inner.lock();
        let mut stmt = conn.prepare(
            r#"SELECT date, sessions, words, dictation_ms, light_count, draft_count
               FROM daily_stats
               ORDER BY date ASC"#,
        )?;
        let days = stmt
            .query_map([], |r| {
                Ok(DailyStat {
                    date: r.get(0)?,
                    sessions: r.get(1)?,
                    words: r.get(2)?,
                    dictation_ms: r.get(3)?,
                    light_count: r.get(4)?,
                    draft_count: r.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut total_sessions = 0i64;
        let mut total_words = 0i64;
        let mut total_dictation_ms = 0i64;
        for d in &days {
            total_sessions += d.sessions;
            total_words += d.words;
            total_dictation_ms += d.dictation_ms;
        }
        let first_day = days.first().map(|d| d.date.clone());

        Ok(StatsSummary {
            days,
            total_sessions,
            total_words,
            total_dictation_ms,
            first_day,
        })
    }

    /// On app launch, mark any rows still in non-terminal states as error.
    /// They represent recordings that were mid-pipeline when the app was
    /// killed / crashed / force-quit. Without this they'd sit at
    /// `transcribing` or `cleaning` forever and the user couldn't tell
    /// they're stranded — the row would just show a stale status with
    /// no recovery affordance. Setting status=error lets the History UI
    /// surface them as failed and expose the Retry button.
    ///
    /// Returns the count of rows recovered, for logging.
    pub fn recover_stranded(&self) -> Result<usize> {
        let conn = self.inner.lock();
        let n = conn.execute(
            r#"UPDATE recordings
               SET status = 'error',
                   error  = COALESCE(error, 'Interrupted — app exited before this recording finished. Click Retry to resume.')
               WHERE status IN ('recording', 'transcribing', 'cleaning', 'injecting')"#,
            [],
        )?;
        Ok(n)
    }
}

const SELECT_ALL_COLUMNS_BY_ID: &str = r#"
SELECT id, created_at, audio_path, duration_ms, mode, status,
       transcript, cleaned_text, drafted_text, meeting_notes_text,
       speaker_turns, speaker_names, is_meeting, diarization_enabled,
       stt_provider, llm_provider,
       clippy_used, clippy_note, retry_count, error, title,
       stt_ms, cleanup_ms, total_ms, event_log, audio_captured_ms, source,
       platform, device_name, dirty, remote
FROM recordings WHERE id = ?1"#;

const SELECT_ALL_COLUMNS: &str = r#"
SELECT id, created_at, audio_path, duration_ms, mode, status,
       transcript, cleaned_text, drafted_text, meeting_notes_text,
       speaker_turns, speaker_names, is_meeting, diarization_enabled,
       stt_provider, llm_provider,
       clippy_used, clippy_note, retry_count, error, title,
       stt_ms, cleanup_ms, total_ms, event_log, audio_captured_ms, source,
       platform, device_name, dirty, remote
FROM recordings"#;

fn row_to_recording(row: &rusqlite::Row<'_>) -> rusqlite::Result<Recording> {
    let created_at_str: String = row.get(1)?;
    let created_at = DateTime::parse_from_rfc3339(&created_at_str)
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    let mode_s: String = row.get(4)?;
    let status_s: String = row.get(5)?;
    Ok(Recording {
        id: row.get(0)?,
        created_at,
        audio_path: PathBuf::from(row.get::<_, String>(2)?),
        duration_ms: row.get(3)?,
        mode: mode_parse(&mode_s),
        status: Status::parse(&status_s),
        transcript: row.get(6)?,
        cleaned_text: row.get(7)?,
        drafted_text: row.get(8)?,
        meeting_notes_text: row.get(9)?,
        speaker_turns: row.get(10)?,
        speaker_names: row.get(11)?,
        is_meeting: row.get::<_, i32>(12)? != 0,
        diarization_enabled: row.get::<_, i32>(13)? != 0,
        stt_provider: row.get(14)?,
        llm_provider: row.get(15)?,
        clippy_used: row.get::<_, i32>(16)? != 0,
        clippy_note: row.get(17)?,
        retry_count: row.get(18)?,
        error: row.get(19)?,
        title: row.get(20)?,
        stt_ms: row.get(21)?,
        cleanup_ms: row.get(22)?,
        total_ms: row.get(23)?,
        event_log: row.get(24)?,
        audio_captured_ms: row.get(25)?,
        source: row
            .get::<_, Option<String>>(26)?
            .unwrap_or_else(|| "mic".to_string()),
        platform: row
            .get::<_, Option<String>>(27)?
            .unwrap_or_else(|| "desktop".to_string()),
        device_name: row.get(28)?,
        dirty: row.get::<_, i32>(29)? != 0,
        remote: row.get::<_, Option<i32>>(30)?.unwrap_or(0) != 0,
    })
}

// ─── Sync support (accounts + cross-device sync, v3.0.0) ───────────────────
//
// See `wispr-fox-web/docs/SYNC_DESIGN.md` for the canonical protocol. History
// owns the local half: which rows are dirty (need pushing), applying rows
// pulled from the cloud, tombstones, per-id "don't resurrect on pull"
// exclusions, and a tiny key/value `sync_meta` table for the device id +
// pull cursor. All of this is completely inert (never touched) unless the
// user is signed in — `engine.rs` is the only caller.

/// A synced transcript row as it comes down from (or goes up to) the cloud
/// `notes` table. Intentionally a plain data bag — `engine.rs` maps this
/// to/from PostgREST JSON; History just knows how to apply/read it locally.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteNote {
    pub id: String,
    pub device_id: String,
    pub platform: String,
    pub device_name: Option<String>,
    pub title: Option<String>,
    pub transcript: Option<String>,
    pub cleaned_text: Option<String>,
    pub drafted_text: Option<String>,
    pub meeting_notes_text: Option<String>,
    pub speaker_turns: Option<String>,
    pub speaker_names: Option<String>,
    pub is_meeting: bool,
    pub diarization_enabled: bool,
    pub duration_ms: i64,
    pub stt_provider: Option<String>,
    pub llm_provider: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

impl History {
    /// Rows eligible for the next push: terminal `done` status AND locally
    /// dirty. Remote rows (pulled from another device) are never pushed back
    /// — they're excluded here so a round-trip echo never happens.
    pub fn list_dirty(&self) -> Result<Vec<Recording>> {
        let conn = self.inner.lock();
        let mut stmt = conn.prepare(&format!(
            "{SELECT_ALL_COLUMNS} WHERE status = 'done' AND dirty = 1 AND COALESCE(remote, 0) = 0"
        ))?;
        let rows = stmt
            .query_map([], row_to_recording)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Clear the dirty flag after a successful push.
    pub fn mark_clean(&self, ids: &[String]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let conn = self.inner.lock();
        let placeholders = vec!["?"; ids.len()].join(",");
        let sql = format!("UPDATE recordings SET dirty = 0 WHERE id IN ({placeholders})");
        let params: Vec<&dyn rusqlite::ToSql> =
            ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        conn.execute(&sql, params.as_slice())?;
        Ok(())
    }

    /// On first sign-in, every existing LOCALLY-ORIGINATED `done` row becomes
    /// eligible for the initial push — the user's whole pre-existing history
    /// goes up.
    ///
    /// The `remote = 0` filter is load-bearing and must stay in lockstep with
    /// `list_dirty`'s. Without it this marked pulled rows dirty too, and since
    /// `list_dirty` correctly refuses to push them they never got cleaned by
    /// `mark_clean` — leaving them stuck at `dirty = 1` forever. `upsert_remote`
    /// skips any locally-dirty row, so from that point on those rows silently
    /// ignored every further update from the cloud, **including tombstones**:
    /// deleting such a transcript on the device that owns it would never remove
    /// this device's copy. One sign-out/sign-in cycle was enough to trigger it.
    pub fn mark_all_done_dirty(&self) -> Result<usize> {
        let conn = self.inner.lock();
        let n = conn.execute(
            "UPDATE recordings SET dirty = 1 \
             WHERE status = 'done' AND COALESCE(remote, 0) = 0",
            [],
        )?;
        Ok(n)
    }

    /// Apply one row pulled from the cloud. Never overwrites a local row that
    /// has unpushed edits (`dirty = 1`) — that would silently discard work
    /// the next push hasn't sent up yet; the row is skipped this cycle and
    /// picked up again once it's been pushed clean. Inserts new remote rows
    /// with `remote = 1` and an empty `audio_path` (audio never syncs).
    /// Returns `true` if the local DB was changed.
    pub fn upsert_remote(&self, note: &RemoteNote) -> Result<bool> {
        let conn = self.inner.lock();
        let existing_dirty: Option<i32> = conn
            .query_row(
                "SELECT dirty FROM recordings WHERE id = ?1",
                params![note.id],
                |r| r.get(0),
            )
            .optional()?;
        if existing_dirty == Some(1) {
            tracing::debug!(id = %note.id, "sync: skipping pull for locally-dirty row");
            return Ok(false);
        }

        let dur = note.duration_ms;
        conn.execute(
            r#"INSERT INTO recordings
                 (id, created_at, audio_path, duration_ms, mode, status,
                  transcript, cleaned_text, drafted_text, meeting_notes_text,
                  speaker_turns, speaker_names, is_meeting, diarization_enabled,
                  stt_provider, llm_provider,
                  title, platform, device_name, dirty, remote, source)
               VALUES (?1, ?2, '', ?3, 'light', 'done',
                       ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                       ?12, ?13, ?14, ?15, ?16, 0, 1, 'mic')
               ON CONFLICT(id) DO UPDATE SET
                 transcript   = excluded.transcript,
                 cleaned_text = excluded.cleaned_text,
                 drafted_text = excluded.drafted_text,
                 meeting_notes_text = excluded.meeting_notes_text,
                 speaker_turns = excluded.speaker_turns,
                 speaker_names = excluded.speaker_names,
                 is_meeting = excluded.is_meeting,
                 diarization_enabled = excluded.diarization_enabled,
                 stt_provider = excluded.stt_provider,
                 llm_provider = excluded.llm_provider,
                 title        = excluded.title,
                 platform     = excluded.platform,
                 device_name  = excluded.device_name,
                 duration_ms  = excluded.duration_ms,
                 dirty        = 0,
                 remote       = 1"#,
            params![
                note.id,
                note.created_at,
                dur,
                note.transcript,
                note.cleaned_text,
                note.drafted_text,
                note.meeting_notes_text,
                note.speaker_turns,
                note.speaker_names,
                note.is_meeting as i32,
                note.diarization_enabled as i32,
                note.stt_provider,
                note.llm_provider,
                note.title,
                note.platform,
                note.device_name,
            ],
        )?;
        Ok(true)
    }

    /// Apply a cloud tombstone (`deleted_at` set): remove the local row and,
    /// if it happens to have a local audio file (it was a local row on THIS
    /// device before being deleted "everywhere" from elsewhere), delete that
    /// file too. No-op if the id isn't present locally.
    pub fn apply_tombstone(&self, id: &str) -> Result<()> {
        let audio_path = {
            let conn = self.inner.lock();
            conn.query_row(
                "SELECT audio_path FROM recordings WHERE id = ?1",
                params![id],
                |r| r.get::<_, String>(0),
            )
            .optional()?
        };
        if let Some(p) = audio_path {
            if !p.is_empty() {
                let _ = std::fs::remove_file(&p);
            }
        }
        self.delete(id)
    }

    /// Remove a recording's local audio file but keep its row. Was the old
    /// "voice files only" delete option; the ownership-scoped delete rework
    /// dropped audio-only deletion (a transcript and its audio die together),
    /// so nothing calls this now. Kept because it's a coherent piece of the
    /// History API and a future feature may want it.
    #[allow(dead_code)]
    pub fn clear_audio(&self, id: &str) -> Result<()> {
        let conn = self.inner.lock();
        let existing: Option<String> = conn
            .query_row(
                "SELECT audio_path FROM recordings WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(p) = existing {
            if !p.is_empty() {
                let _ = std::fs::remove_file(&p);
            }
        }
        conn.execute(
            "UPDATE recordings SET audio_path = '' WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    /// Ids the user chose to delete "this device only" while signed in — a
    /// pull must not resurrect them even though the cloud copy still exists
    /// for other devices. The ownership-scoped delete rework removed the
    /// "this device only" option, so nothing writes exclusions any more; the
    /// table and `exclusion_contains` (read on pull) are retained so entries
    /// legacy builds left behind still suppress resurrection.
    #[allow(dead_code)]
    pub fn exclusion_add(&self, id: &str) -> Result<()> {
        let conn = self.inner.lock();
        conn.execute(
            "INSERT OR IGNORE INTO sync_exclusions (id) VALUES (?1)",
            params![id],
        )?;
        Ok(())
    }

    pub fn exclusion_contains(&self, id: &str) -> Result<bool> {
        let conn = self.inner.lock();
        let found: Option<String> = conn
            .query_row(
                "SELECT id FROM sync_exclusions WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(found.is_some())
    }

    /// Tiny KV store for sync state — device id (generated once, persisted
    /// forever) and the pull cursor (max `updated_at` seen so far).
    pub fn meta_get(&self, key: &str) -> Result<Option<String>> {
        let conn = self.inner.lock();
        let v = conn
            .query_row(
                "SELECT value FROM sync_meta WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()?;
        Ok(v)
    }

    pub fn meta_set(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.inner.lock();
        conn.execute(
            r#"INSERT INTO sync_meta (key, value) VALUES (?1, ?2)
               ON CONFLICT(key) DO UPDATE SET value = excluded.value"#,
            params![key, value],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn schema_round_trip() {
        let tmp = TempDir::new().unwrap();
        let db = tmp.path().join("h.sqlite");
        let h = History::open(&db).unwrap();
        let id = h
            .insert_new(&PathBuf::from("clip.wav"), ClippyMode::Light, "Test Device")
            .unwrap();
        h.set_duration(&id, 1234, 1234).unwrap();
        h.set_transcript(&id, "hello world", "groq").unwrap();
        h.set_cleaned(&id, "Hello world.", Some("groq"), true, None)
            .unwrap();
        h.update_status(&id, Status::Done).unwrap();

        let r = h.get(&id).unwrap().unwrap();
        assert_eq!(r.duration_ms, 1234);
        assert_eq!(r.transcript.as_deref(), Some("hello world"));
        assert_eq!(r.cleaned_text.as_deref(), Some("Hello world."));
        assert_eq!(r.status, Status::Done);
        assert!(r.clippy_used);
    }
}
