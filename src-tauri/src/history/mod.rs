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
    pub stt_provider: Option<String>,
    pub llm_provider: Option<String>,
    pub clippy_used: bool,
    pub clippy_note: Option<String>,
    pub retry_count: i32,
    pub error: Option<String>,
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
              stt_provider  TEXT,
              llm_provider  TEXT,
              clippy_used   INTEGER NOT NULL DEFAULT 0,
              clippy_note   TEXT,
              retry_count   INTEGER NOT NULL DEFAULT 0,
              error         TEXT
            );
            CREATE INDEX IF NOT EXISTS recordings_created_at_idx
              ON recordings(created_at);
            "#,
        )?;
        Ok(Self { inner: Arc::new(Mutex::new(conn)) })
    }

    pub fn insert_new(
        &self,
        audio_path: &Path,
        mode: ClippyMode,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let conn = self.inner.lock();
        conn.execute(
            r#"INSERT INTO recordings
               (id, created_at, audio_path, mode, status)
               VALUES (?1, ?2, ?3, ?4, ?5)"#,
            params![
                id,
                now,
                audio_path.to_string_lossy(),
                mode_str(mode),
                Status::Recording.as_str(),
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
        if !matches!(status, Status::Error) {
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

    pub fn set_duration(&self, id: &str, duration_ms: i64) -> Result<()> {
        let conn = self.inner.lock();
        conn.execute(
            "UPDATE recordings SET duration_ms = ?1 WHERE id = ?2",
            params![duration_ms, id],
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
               SET transcript = ?1, stt_provider = ?2
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
        let conn = self.inner.lock();
        conn.execute(
            r#"UPDATE recordings
               SET cleaned_text = ?1, llm_provider = ?2,
                   clippy_used = ?3, clippy_note = ?4
               WHERE id = ?5"#,
            params![cleaned, provider, used as i32, note, id],
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
        let mut stmt = conn.prepare(
            r#"SELECT id, created_at, audio_path, duration_ms, mode, status,
                      transcript, cleaned_text, stt_provider, llm_provider,
                      clippy_used, clippy_note, retry_count, error
               FROM recordings
               ORDER BY created_at DESC
               LIMIT ?1"#,
        )?;
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
}

const SELECT_ALL_COLUMNS_BY_ID: &str = r#"
SELECT id, created_at, audio_path, duration_ms, mode, status,
       transcript, cleaned_text, stt_provider, llm_provider,
       clippy_used, clippy_note, retry_count, error
FROM recordings WHERE id = ?1"#;

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
        stt_provider: row.get(8)?,
        llm_provider: row.get(9)?,
        clippy_used: row.get::<_, i32>(10)? != 0,
        clippy_note: row.get(11)?,
        retry_count: row.get(12)?,
        error: row.get(13)?,
    })
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
            .insert_new(&PathBuf::from("clip.wav"), ClippyMode::Light)
            .unwrap();
        h.set_duration(&id, 1234).unwrap();
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
