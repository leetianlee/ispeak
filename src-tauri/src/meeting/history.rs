//! Phase 3.5 — persistent meeting history with FTS5 search.
//!
//! Stores completed transcripts in a SQLite database under the app data dir.
//! Metadata columns are queryable directly; full transcript bodies are stored
//! as JSON blobs and re-hydrated on read. A separate FTS5 virtual table indexes
//! the joined segment text + summary for free-text search.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::meeting::types::{Transcript, TranscriptSource};

const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS meetings (
    id              TEXT PRIMARY KEY,
    created_at      INTEGER NOT NULL,
    duration_secs   REAL    NOT NULL,
    source_kind     TEXT    NOT NULL,
    source_value    TEXT,
    summary         TEXT,
    action_items    TEXT    NOT NULL DEFAULT '[]',
    segments_json   TEXT    NOT NULL,
    partial         INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS meetings_created_idx ON meetings(created_at DESC);

CREATE VIRTUAL TABLE IF NOT EXISTS meetings_fts USING fts5(
    id UNINDEXED,
    body,
    tokenize = 'porter unicode61 remove_diacritics 2'
);
"#;

pub struct History {
    conn: Mutex<Connection>,
}

impl History {
    pub fn open(app_data_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(app_data_dir)
            .map_err(|e| AppError::Meeting(format!("create app data dir: {e}")))?;
        let db_path = app_data_dir.join("meetings.db");
        let conn = Connection::open(&db_path)
            .map_err(|e| AppError::Meeting(format!("open history db {}: {e}", db_path.display())))?;
        conn.execute_batch(SCHEMA_SQL)
            .map_err(|e| AppError::Meeting(format!("init history schema: {e}")))?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    /// In-memory ctor for tests.
    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()
            .map_err(|e| AppError::Meeting(format!("open in-mem db: {e}")))?;
        conn.execute_batch(SCHEMA_SQL)
            .map_err(|e| AppError::Meeting(format!("init schema: {e}")))?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn persist(&self, t: &Transcript) -> Result<()> {
        let (source_kind, source_value) = match &t.source {
            TranscriptSource::FileImport(p) => ("file_import", Some(p.display().to_string())),
            TranscriptSource::LiveCapture => ("live_capture", None),
        };
        let action_items = serde_json::to_string(&t.action_items)
            .map_err(|e| AppError::Meeting(format!("encode action_items: {e}")))?;
        let segments = serde_json::to_string(&t.segments)
            .map_err(|e| AppError::Meeting(format!("encode segments: {e}")))?;
        let body = fts_body(t);

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO meetings
                (id, created_at, duration_secs, source_kind, source_value,
                 summary, action_items, segments_json, partial)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                t.id.to_string(),
                t.created_at as i64,
                t.duration_secs as f64,
                source_kind,
                source_value,
                t.summary,
                action_items,
                segments,
                t.partial as i64,
            ],
        )
        .map_err(|e| AppError::Meeting(format!("insert meeting: {e}")))?;

        // FTS row: delete any prior row for this id, then insert fresh.
        conn.execute(
            "DELETE FROM meetings_fts WHERE id = ?1",
            params![t.id.to_string()],
        )
        .map_err(|e| AppError::Meeting(format!("fts delete: {e}")))?;
        conn.execute(
            "INSERT INTO meetings_fts (id, body) VALUES (?1, ?2)",
            params![t.id.to_string(), body],
        )
        .map_err(|e| AppError::Meeting(format!("fts insert: {e}")))?;

        Ok(())
    }

    /// List or search meetings. Empty `query` returns most-recent first.
    /// Non-empty `query` runs an FTS5 MATCH and orders by relevance then recency.
    pub fn list(&self, query: Option<&str>, limit: u32, offset: u32) -> Result<Vec<Transcript>> {
        let conn = self.conn.lock().unwrap();
        let limit = limit.clamp(1, 200) as i64;
        let offset = offset as i64;

        if let Some(q) = query.map(str::trim).filter(|q| !q.is_empty()) {
            let fts_q = sanitize_fts_query(q);
            let mut stmt = conn
                .prepare(
                    "SELECT m.id, m.created_at, m.duration_secs, m.source_kind, m.source_value,
                            m.summary, m.action_items, m.segments_json, m.partial
                     FROM meetings_fts f
                     JOIN meetings m ON m.id = f.id
                     WHERE meetings_fts MATCH ?1
                     ORDER BY rank, m.created_at DESC
                     LIMIT ?2 OFFSET ?3",
                )
                .map_err(map_db_err)?;
            let rows = stmt
                .query_map(params![fts_q, limit, offset], row_to_transcript)
                .map_err(map_db_err)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(map_db_err)?;
            Ok(rows)
        } else {
            let mut stmt = conn
                .prepare(
                    "SELECT id, created_at, duration_secs, source_kind, source_value,
                            summary, action_items, segments_json, partial
                     FROM meetings
                     ORDER BY created_at DESC
                     LIMIT ?1 OFFSET ?2",
                )
                .map_err(map_db_err)?;
            let rows = stmt
                .query_map(params![limit, offset], row_to_transcript)
                .map_err(map_db_err)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(map_db_err)?;
            Ok(rows)
        }
    }

    pub fn get(&self, id: Uuid) -> Result<Option<Transcript>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, created_at, duration_secs, source_kind, source_value,
                        summary, action_items, segments_json, partial
                 FROM meetings WHERE id = ?1",
            )
            .map_err(map_db_err)?;
        let row = stmt
            .query_row(params![id.to_string()], row_to_transcript)
            .optional()
            .map_err(map_db_err)?;
        Ok(row)
    }

    pub fn delete(&self, id: Uuid) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM meetings_fts WHERE id = ?1",
            params![id.to_string()],
        )
        .map_err(map_db_err)?;
        let n = conn
            .execute("DELETE FROM meetings WHERE id = ?1", params![id.to_string()])
            .map_err(map_db_err)?;
        Ok(n > 0)
    }

    pub fn count(&self) -> Result<u32> {
        let conn = self.conn.lock().unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM meetings", [], |r| r.get(0))
            .map_err(map_db_err)?;
        Ok(n.max(0) as u32)
    }
}

fn fts_body(t: &Transcript) -> String {
    let mut s = String::new();
    if let Some(summary) = &t.summary {
        s.push_str(summary);
        s.push('\n');
    }
    for item in &t.action_items {
        s.push_str(item);
        s.push('\n');
    }
    for seg in &t.segments {
        s.push_str(&seg.text);
        s.push('\n');
    }
    s
}

/// FTS5 MATCH treats some characters as syntax (e.g. `-`, `"`, `:`). Wrap each
/// whitespace-separated word in double quotes so the user's query is interpreted
/// literally as a phrase AND across tokens, not as FTS operators.
fn sanitize_fts_query(q: &str) -> String {
    let parts: Vec<String> = q
        .split_whitespace()
        .filter(|p| !p.is_empty())
        .map(|p| {
            // Escape internal double quotes by doubling them, per FTS5 grammar.
            let escaped = p.replace('"', "\"\"");
            format!("\"{}\"", escaped)
        })
        .collect();
    if parts.is_empty() {
        "\"\"".to_string()
    } else {
        parts.join(" ")
    }
}

fn row_to_transcript(row: &rusqlite::Row<'_>) -> rusqlite::Result<Transcript> {
    let id_str: String = row.get(0)?;
    let created_at: i64 = row.get(1)?;
    let duration_secs: f64 = row.get(2)?;
    let source_kind: String = row.get(3)?;
    let source_value: Option<String> = row.get(4)?;
    let summary: Option<String> = row.get(5)?;
    let action_items_json: String = row.get(6)?;
    let segments_json: String = row.get(7)?;
    let partial: i64 = row.get(8)?;

    let id = Uuid::parse_str(&id_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let source = match (source_kind.as_str(), source_value) {
        ("file_import", Some(v)) => TranscriptSource::FileImport(PathBuf::from(v)),
        ("file_import", None) => TranscriptSource::FileImport(PathBuf::new()),
        ("live_capture", _) => TranscriptSource::LiveCapture,
        (other, _) => {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unknown source_kind: {other}"),
                )),
            ));
        }
    };
    let action_items: Vec<String> = serde_json::from_str(&action_items_json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let segments = serde_json::from_str(&segments_json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })?;

    Ok(Transcript {
        id,
        created_at: created_at as u64,
        duration_secs: duration_secs as f32,
        source,
        segments,
        summary,
        action_items,
        partial: partial != 0,
    })
}

fn map_db_err(e: rusqlite::Error) -> AppError {
    AppError::Meeting(format!("history db: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meeting::types::{Segment, SpeakerLabel};

    fn fixture(id: Uuid, when: u64, text: &str) -> Transcript {
        Transcript {
            id,
            created_at: when,
            duration_secs: 60.0,
            source: TranscriptSource::FileImport(PathBuf::from("/tmp/m.wav")),
            segments: vec![Segment {
                start: 0.0,
                end: 5.0,
                speaker: SpeakerLabel::Other,
                text: text.into(),
            }],
            summary: Some(format!("summary of {text}")),
            action_items: vec!["check email".into()],
            partial: false,
        }
    }

    #[test]
    fn roundtrip_persist_and_get() {
        let h = History::open_in_memory().unwrap();
        let id = Uuid::new_v4();
        let t = fixture(id, 1000, "hello world");
        h.persist(&t).unwrap();
        let got = h.get(id).unwrap().unwrap();
        assert_eq!(got.id, id);
        assert_eq!(got.segments[0].text, "hello world");
        assert_eq!(got.summary.as_deref(), Some("summary of hello world"));
        assert_eq!(got.action_items, vec!["check email".to_string()]);
    }

    #[test]
    fn list_orders_by_recency() {
        let h = History::open_in_memory().unwrap();
        h.persist(&fixture(Uuid::new_v4(), 1, "old")).unwrap();
        h.persist(&fixture(Uuid::new_v4(), 100, "new")).unwrap();
        let rows = h.list(None, 10, 0).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].segments[0].text, "new");
        assert_eq!(rows[1].segments[0].text, "old");
    }

    #[test]
    fn search_returns_only_matching_rows() {
        let h = History::open_in_memory().unwrap();
        h.persist(&fixture(Uuid::new_v4(), 1, "discussing quarterly budget")).unwrap();
        h.persist(&fixture(Uuid::new_v4(), 2, "talking about migration plan")).unwrap();
        let rows = h.list(Some("budget"), 10, 0).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].segments[0].text.contains("budget"));
    }

    #[test]
    fn search_strips_fts_operators() {
        let h = History::open_in_memory().unwrap();
        h.persist(&fixture(Uuid::new_v4(), 1, "fixing the bug")).unwrap();
        // "-bug" without sanitisation is a column-exclusion operator and would crash.
        let rows = h.list(Some("-bug"), 10, 0).unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn delete_removes_from_both_tables() {
        let h = History::open_in_memory().unwrap();
        let id = Uuid::new_v4();
        h.persist(&fixture(id, 1, "remove me")).unwrap();
        assert!(h.delete(id).unwrap());
        assert!(h.get(id).unwrap().is_none());
        let rows = h.list(Some("remove"), 10, 0).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn count_reflects_inserts() {
        let h = History::open_in_memory().unwrap();
        assert_eq!(h.count().unwrap(), 0);
        h.persist(&fixture(Uuid::new_v4(), 1, "a")).unwrap();
        h.persist(&fixture(Uuid::new_v4(), 2, "b")).unwrap();
        assert_eq!(h.count().unwrap(), 2);
    }
}
