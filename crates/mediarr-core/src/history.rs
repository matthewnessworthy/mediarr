//! Rename history storage and undo support.
//!
//! Records every rename operation in an SQLite database, supports querying
//! historical batches, checking undo eligibility, and executing undo to
//! reverse rename operations.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};
use tracing::{debug, info, warn};

use crate::error::{MediError, Result};
use crate::fs_util;
use crate::types::{
    BatchSummary, MediaInfo, RenameRecord, RenameResult, ReviewQueueEntry, ReviewStatus,
    UndoEligibility, UndoIssue, WatcherAction, WatcherEvent,
};

/// SQL schema for the rename history table.
///
/// Uses `CREATE TABLE IF NOT EXISTS` for safe auto-migration on first open.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS rename_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    batch_id TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    source_path TEXT NOT NULL,
    dest_path TEXT NOT NULL,
    media_info TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    file_mtime TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_batch_id ON rename_history(batch_id);
CREATE INDEX IF NOT EXISTS idx_timestamp ON rename_history(timestamp);
";

/// SQL schema for watcher-related tables.
///
/// Creates `watcher_events` for activity logging and `review_queue` for
/// the queue-for-review workflow. Both use `CREATE TABLE IF NOT EXISTS`.
const WATCHER_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS watcher_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    watch_path TEXT NOT NULL,
    filename TEXT NOT NULL,
    action TEXT NOT NULL,
    detail TEXT,
    batch_id TEXT
);
CREATE INDEX IF NOT EXISTS idx_watcher_events_watch_path ON watcher_events(watch_path);
CREATE INDEX IF NOT EXISTS idx_watcher_events_timestamp ON watcher_events(timestamp);
CREATE INDEX IF NOT EXISTS idx_watcher_events_path_time ON watcher_events(watch_path, timestamp DESC);

CREATE TABLE IF NOT EXISTS review_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    watch_path TEXT NOT NULL,
    source_path TEXT NOT NULL,
    proposed_path TEXT NOT NULL,
    media_info TEXT NOT NULL,
    subtitles TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
);
CREATE INDEX IF NOT EXISTS idx_review_queue_watch_path ON review_queue(watch_path);
CREATE INDEX IF NOT EXISTS idx_review_queue_status ON review_queue(status);
";

/// SQLite-backed rename history database.
///
/// Provides recording, querying, undo eligibility checking, and undo
/// execution for rename batches. Each batch groups files renamed in a
/// single operation, identified by a UUID v4 batch ID.
pub struct HistoryDb {
    conn: Connection,
}

impl HistoryDb {
    /// Open (or create) the history database at the given path.
    ///
    /// Creates parent directories if they do not exist and executes the
    /// schema migration on first open.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path)?;

        // Enable WAL mode for concurrent CLI/GUI access and set busy timeout
        // to avoid "database is locked" errors when both processes access the DB.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;

        conn.execute_batch(SCHEMA)?;
        conn.execute_batch(WATCHER_SCHEMA)?;

        debug!(?path, "opened history database");
        Ok(Self { conn })
    }

    /// Generate a new batch ID using UUID v4.
    pub fn generate_batch_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    /// Record a batch of rename operations atomically.
    ///
    /// All entries are inserted within a single transaction. If any insert
    /// fails, the entire batch is rolled back. Path storage uses
    /// [`fs_util::path_to_utf8`] to ensure lossless round-tripping --
    /// non-UTF-8 paths will return an error rather than silently corrupting
    /// stored data.
    pub fn record_batch(&self, entries: &[RenameRecord]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;

        for entry in entries {
            let source_str = fs_util::path_to_utf8(&entry.source_path)?;
            let dest_str = fs_util::path_to_utf8(&entry.dest_path)?;
            let media_json = serde_json::to_string(&entry.media_info)?;

            tx.execute(
                "INSERT INTO rename_history (batch_id, timestamp, source_path, dest_path, media_info, file_size, file_mtime)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    entry.batch_id,
                    entry.timestamp,
                    source_str,
                    dest_str,
                    media_json,
                    entry.file_size as i64,
                    entry.file_mtime,
                ],
            )?;
        }

        tx.commit()?;
        info!(count = entries.len(), "recorded rename batch");
        Ok(())
    }

    /// Record successful rename results to history with a single call.
    ///
    /// Generates a batch ID and timestamp, reads dest file metadata for
    /// file_size and mtime, and looks up `MediaInfo` from the provided map
    /// (keyed by source path string). Skips failed results and results
    /// whose dest metadata cannot be read. Returns the batch ID on success,
    /// or an empty string if no successful results exist.
    pub fn record_rename_results(
        &self,
        results: &[RenameResult],
        media_info_map: &std::collections::HashMap<String, MediaInfo>,
    ) -> Result<String> {
        let succeeded: Vec<_> = results.iter().filter(|r| r.success).collect();
        if succeeded.is_empty() {
            return Ok(String::new());
        }

        let batch_id = Self::generate_batch_id();
        let timestamp = chrono::Utc::now().to_rfc3339();

        let records: Vec<RenameRecord> = succeeded
            .iter()
            .filter_map(|r| {
                let meta = std::fs::metadata(&r.dest_path).ok()?;
                let file_mtime = meta
                    .modified()
                    .ok()
                    .map(|t| {
                        let dt: chrono::DateTime<chrono::Utc> = t.into();
                        dt.to_rfc3339()
                    })
                    .unwrap_or_default();

                let source_key = r.source_path.to_string_lossy().to_string();
                let info = media_info_map.get(&source_key).cloned().unwrap_or_default();

                Some(RenameRecord {
                    batch_id: batch_id.clone(),
                    timestamp: timestamp.clone(),
                    source_path: r.source_path.clone(),
                    dest_path: r.dest_path.clone(),
                    media_info: info,
                    file_size: meta.len(),
                    file_mtime,
                })
            })
            .collect();

        if records.is_empty() {
            return Ok(String::new());
        }

        self.record_batch(&records)?;
        Ok(batch_id)
    }

    /// List recent rename batches in reverse chronological order.
    ///
    /// Returns batch summaries with file counts. The `entries` field of each
    /// summary is empty -- use [`get_batch`] to retrieve full details.
    pub fn list_batches(&self, limit: Option<usize>) -> Result<Vec<BatchSummary>> {
        let query = match limit {
            Some(n) => format!(
                "SELECT batch_id, MIN(timestamp) as ts, COUNT(*) as cnt \
                 FROM rename_history GROUP BY batch_id ORDER BY ts DESC LIMIT {}",
                n
            ),
            None => "SELECT batch_id, MIN(timestamp) as ts, COUNT(*) as cnt \
                     FROM rename_history GROUP BY batch_id ORDER BY ts DESC"
                .to_string(),
        };

        let mut stmt = self.conn.prepare(&query)?;
        let batches = stmt
            .query_map([], |row| {
                Ok(BatchSummary {
                    batch_id: row.get(0)?,
                    timestamp: row.get(1)?,
                    file_count: row.get::<_, i64>(2)? as usize,
                    entries: Vec::new(),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(batches)
    }

    /// Get all rename records for a specific batch.
    ///
    /// Records are returned in insertion order (by row ID).
    pub fn get_batch(&self, batch_id: &str) -> Result<Vec<RenameRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT batch_id, timestamp, source_path, dest_path, media_info, file_size, file_mtime \
             FROM rename_history WHERE batch_id = ?1 ORDER BY id",
        )?;

        let records = stmt
            .query_map(params![batch_id], |row| {
                let media_json: String = row.get(4)?;
                let media_info: MediaInfo = serde_json::from_str(&media_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        4,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

                Ok(RenameRecord {
                    batch_id: row.get(0)?,
                    timestamp: row.get(1)?,
                    source_path: PathBuf::from(row.get::<_, String>(2)?),
                    dest_path: PathBuf::from(row.get::<_, String>(3)?),
                    media_info,
                    file_size: row.get::<_, i64>(5)? as u64,
                    file_mtime: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(records)
    }

    /// Check whether a batch is eligible for undo.
    ///
    /// A batch is eligible only if ALL entries pass these checks:
    /// - The destination file still exists
    /// - The source location is not occupied by another file
    /// - The file at the destination has the same size as when it was renamed
    pub fn check_undo_eligible(&self, batch_id: &str) -> Result<UndoEligibility> {
        let entries = self.get_batch(batch_id)?;
        let mut issues = Vec::new();

        for entry in &entries {
            if !entry.dest_path.exists() {
                issues.push(UndoIssue {
                    dest_path: entry.dest_path.clone(),
                    reason: "destination file missing".to_string(),
                });
                continue;
            }

            if entry.source_path.exists() {
                issues.push(UndoIssue {
                    dest_path: entry.dest_path.clone(),
                    reason: "source location occupied".to_string(),
                });
                continue;
            }

            match std::fs::metadata(&entry.dest_path) {
                Ok(meta) => {
                    if meta.len() != entry.file_size {
                        issues.push(UndoIssue {
                            dest_path: entry.dest_path.clone(),
                            reason: "file modified since rename".to_string(),
                        });
                    }
                }
                Err(e) => {
                    warn!(?e, path = ?entry.dest_path, "failed to read file metadata");
                    issues.push(UndoIssue {
                        dest_path: entry.dest_path.clone(),
                        reason: format!("cannot read file metadata: {e}"),
                    });
                }
            }
        }

        Ok(UndoEligibility {
            eligible: issues.is_empty(),
            batch_id: batch_id.to_string(),
            ineligible_reasons: issues,
        })
    }

    /// Execute an undo operation for a batch.
    ///
    /// Checks eligibility first, then moves each file from its destination
    /// back to its source path using [`fs_util::safe_move`]. Files are
    /// processed in reverse order. On success, the batch is deleted from
    /// the database.
    pub fn execute_undo(&self, batch_id: &str) -> Result<Vec<RenameResult>> {
        let eligibility = self.check_undo_eligible(batch_id)?;
        if !eligibility.eligible {
            let reasons: Vec<String> = eligibility
                .ineligible_reasons
                .iter()
                .map(|i| format!("{}: {}", i.dest_path.display(), i.reason))
                .collect();
            return Err(MediError::UndoNotEligible {
                batch_id: batch_id.to_string(),
                reason: reasons.join("; "),
            });
        }

        let mut entries = self.get_batch(batch_id)?;
        entries.reverse();

        let mut results = Vec::new();

        // Use a transaction so partially-successful undos are atomic:
        // each successfully-moved entry is deleted individually, while
        // failed entries remain in the DB for retry.
        let tx = self.conn.unchecked_transaction()?;

        for entry in &entries {
            // Create parent directory for source path if needed
            if let Some(parent) = entry.source_path.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    results.push(RenameResult {
                        source_path: entry.dest_path.clone(),
                        dest_path: entry.source_path.clone(),
                        success: false,
                        error: Some(format!("failed to create parent dir: {e}")),
                    });
                    continue;
                }
            }

            match fs_util::safe_move(&entry.dest_path, &entry.source_path) {
                Ok(()) => {
                    debug!(
                        from = ?entry.dest_path,
                        to = ?entry.source_path,
                        "undo: moved file back"
                    );
                    // Delete this entry from DB immediately — it was successfully undone
                    tx.execute(
                        "DELETE FROM rename_history WHERE batch_id = ?1 AND dest_path = ?2",
                        params![batch_id, entry.dest_path.to_string_lossy()],
                    )?;
                    results.push(RenameResult {
                        source_path: entry.dest_path.clone(),
                        dest_path: entry.source_path.clone(),
                        success: true,
                        error: None,
                    });
                }
                Err(e) => {
                    warn!(?e, "undo: failed to move file back");
                    results.push(RenameResult {
                        source_path: entry.dest_path.clone(),
                        dest_path: entry.source_path.clone(),
                        success: false,
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        tx.commit()?;

        if results.iter().all(|r| r.success) {
            info!(batch_id, "undo complete, all entries removed from history");
        } else {
            let failed = results.iter().filter(|r| !r.success).count();
            warn!(batch_id, failed, "partial undo — failed entries remain in history for retry");
        }

        Ok(results)
    }

    /// Clear all rename history entries from the database.
    ///
    /// Deletes every row in the `rename_history` table. This is irreversible.
    pub fn clear_history(&self) -> Result<usize> {
        let deleted = self.conn.execute("DELETE FROM rename_history", [])?;
        info!(deleted, "cleared rename history");
        Ok(deleted)
    }

    // -----------------------------------------------------------------------
    // Watcher event methods
    // -----------------------------------------------------------------------

    /// Log a watcher event to the database.
    pub fn log_watcher_event(&self, event: &WatcherEvent) -> Result<()> {
        let watch_path_str = crate::fs_util::path_to_utf8(&event.watch_path)?;
        let action_str = event.action.to_string();

        self.conn.execute(
            "INSERT INTO watcher_events (timestamp, watch_path, filename, action, detail, batch_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                event.timestamp,
                watch_path_str,
                event.filename,
                action_str,
                event.detail,
                event.batch_id,
            ],
        )?;

        debug!(filename = %event.filename, action = %action_str, "logged watcher event");
        Ok(())
    }

    /// List watcher events, optionally filtered by watch path.
    ///
    /// Returns events in reverse chronological order (newest first).
    pub fn list_watcher_events(
        &self,
        watch_path: Option<&Path>,
        limit: Option<usize>,
    ) -> Result<Vec<WatcherEvent>> {
        let (query, watch_path_str);

        if let Some(wp) = watch_path {
            watch_path_str = crate::fs_util::path_to_utf8(wp)?;
            query = match limit {
                Some(n) => format!(
                    "SELECT id, timestamp, watch_path, filename, action, detail, batch_id \
                     FROM watcher_events WHERE watch_path = ?1 \
                     ORDER BY timestamp DESC LIMIT {n}"
                ),
                None => "SELECT id, timestamp, watch_path, filename, action, detail, batch_id \
                         FROM watcher_events WHERE watch_path = ?1 \
                         ORDER BY timestamp DESC"
                    .to_string(),
            };

            let mut stmt = self.conn.prepare(&query)?;
            let events = stmt
                .query_map(params![watch_path_str], Self::map_watcher_event_row)?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(events)
        } else {
            query = match limit {
                Some(n) => format!(
                    "SELECT id, timestamp, watch_path, filename, action, detail, batch_id \
                     FROM watcher_events ORDER BY timestamp DESC LIMIT {n}"
                ),
                None => "SELECT id, timestamp, watch_path, filename, action, detail, batch_id \
                         FROM watcher_events ORDER BY timestamp DESC"
                    .to_string(),
            };

            let mut stmt = self.conn.prepare(&query)?;
            let events = stmt
                .query_map([], Self::map_watcher_event_row)?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(events)
        }
    }

    /// Prune watcher events for a given watch path, keeping only the most
    /// recent `max_events` entries (per D-07).
    ///
    /// Returns the number of rows deleted.
    pub fn prune_watcher_events(&self, watch_path: &Path, max_events: usize) -> Result<usize> {
        let watch_path_str = crate::fs_util::path_to_utf8(watch_path)?;

        let deleted = self.conn.execute(
            "DELETE FROM watcher_events WHERE watch_path = ?1 AND id NOT IN (
                SELECT id FROM watcher_events WHERE watch_path = ?1
                ORDER BY timestamp DESC LIMIT ?2
            )",
            params![watch_path_str, max_events as i64],
        )?;

        if deleted > 0 {
            debug!(watch_path = %watch_path_str, deleted, "pruned watcher events");
        }
        Ok(deleted)
    }

    /// Map a SQLite row to a WatcherEvent.
    fn map_watcher_event_row(row: &rusqlite::Row) -> rusqlite::Result<WatcherEvent> {
        let action_str: String = row.get(4)?;
        let action = match action_str.as_str() {
            "renamed" => WatcherAction::Renamed,
            "queued" => WatcherAction::Queued,
            _ => WatcherAction::Error,
        };

        Ok(WatcherEvent {
            id: Some(row.get(0)?),
            timestamp: row.get(1)?,
            watch_path: PathBuf::from(row.get::<_, String>(2)?),
            filename: row.get(3)?,
            action,
            detail: row.get(5)?,
            batch_id: row.get(6)?,
        })
    }

    // -----------------------------------------------------------------------
    // Review queue methods
    // -----------------------------------------------------------------------

    /// Add an entry to the review queue.
    ///
    /// Returns the new row ID.
    pub fn add_to_review_queue(&self, entry: &ReviewQueueEntry) -> Result<i64> {
        let watch_path_str = crate::fs_util::path_to_utf8(&entry.watch_path)?;
        let source_path_str = crate::fs_util::path_to_utf8(&entry.source_path)?;
        let proposed_path_str = crate::fs_util::path_to_utf8(&entry.proposed_path)?;
        let status_str = entry.status.to_string();

        self.conn.execute(
            "INSERT INTO review_queue (timestamp, watch_path, source_path, proposed_path, media_info, subtitles, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                entry.timestamp,
                watch_path_str,
                source_path_str,
                proposed_path_str,
                entry.media_info_json,
                entry.subtitles_json,
                status_str,
            ],
        )?;

        let id = self.conn.last_insert_rowid();
        debug!(id, source = %source_path_str, "added to review queue");
        Ok(id)
    }

    /// List review queue entries, optionally filtered by watch path and/or status.
    pub fn list_review_queue(
        &self,
        watch_path: Option<&Path>,
        status: Option<ReviewStatus>,
    ) -> Result<Vec<ReviewQueueEntry>> {
        let mut conditions = Vec::new();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut param_idx = 1;

        if let Some(wp) = watch_path {
            let wp_str = crate::fs_util::path_to_utf8(wp)?;
            conditions.push(format!("watch_path = ?{param_idx}"));
            param_values.push(Box::new(wp_str));
            param_idx += 1;
        }

        if let Some(st) = status {
            conditions.push(format!("status = ?{param_idx}"));
            param_values.push(Box::new(st.to_string()));
            // param_idx is unused after this but kept for clarity
            let _ = param_idx;
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };

        let query = format!(
            "SELECT id, timestamp, watch_path, source_path, proposed_path, media_info, subtitles, status \
             FROM review_queue{where_clause} ORDER BY timestamp DESC"
        );

        let mut stmt = self.conn.prepare(&query)?;
        let params: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let entries = stmt
            .query_map(params.as_slice(), Self::map_review_queue_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(entries)
    }

    /// Update the status of a review queue entry.
    pub fn update_review_status(&self, id: i64, status: ReviewStatus) -> Result<()> {
        let status_str = status.to_string();
        self.conn.execute(
            "UPDATE review_queue SET status = ?1 WHERE id = ?2",
            params![status_str, id],
        )?;
        debug!(id, status = %status_str, "updated review queue status");
        Ok(())
    }

    /// Remove a review queue entry by ID.
    pub fn remove_review_entry(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM review_queue WHERE id = ?1", params![id])?;
        debug!(id, "removed review queue entry");
        Ok(())
    }

    /// Map a SQLite row to a ReviewQueueEntry.
    fn map_review_queue_row(row: &rusqlite::Row) -> rusqlite::Result<ReviewQueueEntry> {
        let status_str: String = row.get(7)?;
        let status = match status_str.as_str() {
            "approved" => ReviewStatus::Approved,
            "rejected" => ReviewStatus::Rejected,
            _ => ReviewStatus::Pending,
        };

        Ok(ReviewQueueEntry {
            id: Some(row.get(0)?),
            timestamp: row.get(1)?,
            watch_path: PathBuf::from(row.get::<_, String>(2)?),
            source_path: PathBuf::from(row.get::<_, String>(3)?),
            proposed_path: PathBuf::from(row.get::<_, String>(4)?),
            media_info_json: row.get(5)?,
            subtitles_json: row.get(6)?,
            status,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{MediaInfo, MediaType, ParseConfidence};
    use tempfile::TempDir;

    fn test_media_info() -> MediaInfo {
        MediaInfo {
            title: "Test Movie".to_string(),
            media_type: MediaType::Movie,
            year: Some(2024),
            season: None,
            episodes: Vec::new(),
            resolution: Some("1080p".to_string()),
            video_codec: Some("x265".to_string()),
            audio_codec: Some("AAC".to_string()),
            source: Some("BluRay".to_string()),
            release_group: Some("TestGroup".to_string()),
            container: "mkv".to_string(),
            language: Some("en".to_string()),
            confidence: ParseConfidence::High,
        }
    }

    fn make_record(batch_id: &str, source: &Path, dest: &Path, size: u64) -> RenameRecord {
        RenameRecord {
            batch_id: batch_id.to_string(),
            timestamp: "2024-01-15T10:30:00Z".to_string(),
            source_path: source.to_path_buf(),
            dest_path: dest.to_path_buf(),
            media_info: test_media_info(),
            file_size: size,
            file_mtime: "2024-01-15T09:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_open_creates_schema() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");

        let db = HistoryDb::open(&db_path).unwrap();

        // Verify tables exist by querying them
        let mut stmt = db
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='rename_history'")
            .unwrap();
        let tables: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(tables, vec!["rename_history"]);

        // Verify indexes exist
        let mut stmt = db
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'")
            .unwrap();
        let indexes: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(indexes.contains(&"idx_batch_id".to_string()));
        assert!(indexes.contains(&"idx_timestamp".to_string()));
    }

    #[test]
    fn test_record_batch_inserts_atomically() {
        let dir = TempDir::new().unwrap();
        let db = HistoryDb::open(&dir.path().join("test.db")).unwrap();

        let batch_id = HistoryDb::generate_batch_id();
        let entries = vec![
            make_record(
                &batch_id,
                Path::new("/src/a.mkv"),
                Path::new("/dst/a.mkv"),
                1000,
            ),
            make_record(
                &batch_id,
                Path::new("/src/b.mkv"),
                Path::new("/dst/b.mkv"),
                2000,
            ),
        ];

        db.record_batch(&entries).unwrap();

        // Verify both rows inserted
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM rename_history", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_record_then_list_batches() {
        let dir = TempDir::new().unwrap();
        let db = HistoryDb::open(&dir.path().join("test.db")).unwrap();

        let batch_id = HistoryDb::generate_batch_id();
        let entries = vec![
            make_record(
                &batch_id,
                Path::new("/src/a.mkv"),
                Path::new("/dst/a.mkv"),
                1000,
            ),
            make_record(
                &batch_id,
                Path::new("/src/b.mkv"),
                Path::new("/dst/b.mkv"),
                2000,
            ),
            make_record(
                &batch_id,
                Path::new("/src/c.mkv"),
                Path::new("/dst/c.mkv"),
                3000,
            ),
        ];

        db.record_batch(&entries).unwrap();

        let batches = db.list_batches(None).unwrap();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].batch_id, batch_id);
        assert_eq!(batches[0].file_count, 3);
    }

    #[test]
    fn test_list_batches_reverse_chronological() {
        let dir = TempDir::new().unwrap();
        let db = HistoryDb::open(&dir.path().join("test.db")).unwrap();

        // Insert older batch
        let batch_old = "batch-old".to_string();
        let mut old_record = make_record(
            &batch_old,
            Path::new("/src/old.mkv"),
            Path::new("/dst/old.mkv"),
            100,
        );
        old_record.timestamp = "2024-01-01T00:00:00Z".to_string();
        db.record_batch(&[old_record]).unwrap();

        // Insert newer batch
        let batch_new = "batch-new".to_string();
        let mut new_record = make_record(
            &batch_new,
            Path::new("/src/new.mkv"),
            Path::new("/dst/new.mkv"),
            200,
        );
        new_record.timestamp = "2024-06-15T00:00:00Z".to_string();
        db.record_batch(&[new_record]).unwrap();

        let batches = db.list_batches(None).unwrap();
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].batch_id, "batch-new");
        assert_eq!(batches[1].batch_id, "batch-old");
    }

    #[test]
    fn test_list_batches_with_limit() {
        let dir = TempDir::new().unwrap();
        let db = HistoryDb::open(&dir.path().join("test.db")).unwrap();

        for i in 0..5 {
            let batch_id = format!("batch-{i}");
            let mut record = make_record(
                &batch_id,
                Path::new(&format!("/src/{i}.mkv")),
                Path::new(&format!("/dst/{i}.mkv")),
                100,
            );
            record.timestamp = format!("2024-01-{:02}T00:00:00Z", i + 1);
            db.record_batch(&[record]).unwrap();
        }

        let batches = db.list_batches(Some(3)).unwrap();
        assert_eq!(batches.len(), 3);
    }

    #[test]
    fn test_get_batch_returns_entries() {
        let dir = TempDir::new().unwrap();
        let db = HistoryDb::open(&dir.path().join("test.db")).unwrap();

        let batch_id = HistoryDb::generate_batch_id();
        let entries = vec![
            make_record(
                &batch_id,
                Path::new("/src/a.mkv"),
                Path::new("/dst/a.mkv"),
                1000,
            ),
            make_record(
                &batch_id,
                Path::new("/src/b.mkv"),
                Path::new("/dst/b.mkv"),
                2000,
            ),
        ];

        db.record_batch(&entries).unwrap();

        let retrieved = db.get_batch(&batch_id).unwrap();
        assert_eq!(retrieved.len(), 2);
        assert_eq!(retrieved[0].source_path, PathBuf::from("/src/a.mkv"));
        assert_eq!(retrieved[1].source_path, PathBuf::from("/src/b.mkv"));
    }

    #[test]
    fn test_check_undo_eligible_all_ok() {
        let dir = TempDir::new().unwrap();
        let db = HistoryDb::open(&dir.path().join("test.db")).unwrap();

        // Create dest files on disk
        let dest_file = dir.path().join("dest.mkv");
        std::fs::write(&dest_file, "file content").unwrap();

        let source_path = dir.path().join("source.mkv");
        let batch_id = HistoryDb::generate_batch_id();
        let record = make_record(
            &batch_id,
            &source_path,
            &dest_file,
            std::fs::metadata(&dest_file).unwrap().len(),
        );

        db.record_batch(&[record]).unwrap();

        let eligibility = db.check_undo_eligible(&batch_id).unwrap();
        assert!(eligibility.eligible);
        assert!(eligibility.ineligible_reasons.is_empty());
    }

    #[test]
    fn test_check_undo_eligible_dest_missing() {
        let dir = TempDir::new().unwrap();
        let db = HistoryDb::open(&dir.path().join("test.db")).unwrap();

        let batch_id = HistoryDb::generate_batch_id();
        let record = make_record(
            &batch_id,
            &dir.path().join("source.mkv"),
            &dir.path().join("nonexistent_dest.mkv"),
            1000,
        );

        db.record_batch(&[record]).unwrap();

        let eligibility = db.check_undo_eligible(&batch_id).unwrap();
        assert!(!eligibility.eligible);
        assert_eq!(eligibility.ineligible_reasons.len(), 1);
        assert_eq!(
            eligibility.ineligible_reasons[0].reason,
            "destination file missing"
        );
    }

    #[test]
    fn test_check_undo_eligible_source_occupied() {
        let dir = TempDir::new().unwrap();
        let db = HistoryDb::open(&dir.path().join("test.db")).unwrap();

        let source_file = dir.path().join("source.mkv");
        let dest_file = dir.path().join("dest.mkv");

        // Both source and dest exist
        std::fs::write(&source_file, "occupying").unwrap();
        std::fs::write(&dest_file, "file content").unwrap();

        let batch_id = HistoryDb::generate_batch_id();
        let record = make_record(
            &batch_id,
            &source_file,
            &dest_file,
            std::fs::metadata(&dest_file).unwrap().len(),
        );

        db.record_batch(&[record]).unwrap();

        let eligibility = db.check_undo_eligible(&batch_id).unwrap();
        assert!(!eligibility.eligible);
        assert_eq!(eligibility.ineligible_reasons.len(), 1);
        assert_eq!(
            eligibility.ineligible_reasons[0].reason,
            "source location occupied"
        );
    }

    #[test]
    fn test_execute_undo_moves_files_back() {
        let dir = TempDir::new().unwrap();
        let db = HistoryDb::open(&dir.path().join("test.db")).unwrap();

        let source_dir = dir.path().join("source_dir");
        let dest_dir = dir.path().join("dest_dir");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::create_dir_all(&dest_dir).unwrap();

        let source_path = source_dir.join("movie.mkv");
        let dest_path = dest_dir.join("movie.mkv");

        // Simulate: file was moved from source to dest
        std::fs::write(&dest_path, "movie data").unwrap();
        let file_size = std::fs::metadata(&dest_path).unwrap().len();

        let batch_id = HistoryDb::generate_batch_id();
        let record = make_record(&batch_id, &source_path, &dest_path, file_size);

        db.record_batch(&[record]).unwrap();

        let results = db.execute_undo(&batch_id).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].success);

        // File should be back at source
        assert!(source_path.exists());
        assert!(!dest_path.exists());
        assert_eq!(std::fs::read_to_string(&source_path).unwrap(), "movie data");

        // Batch should be removed from database
        let batches = db.list_batches(None).unwrap();
        assert!(batches.is_empty());
    }

    #[test]
    fn test_execute_undo_fails_when_ineligible() {
        let dir = TempDir::new().unwrap();
        let db = HistoryDb::open(&dir.path().join("test.db")).unwrap();

        let batch_id = HistoryDb::generate_batch_id();
        let record = make_record(
            &batch_id,
            &dir.path().join("source.mkv"),
            &dir.path().join("nonexistent.mkv"),
            1000,
        );

        db.record_batch(&[record]).unwrap();

        let result = db.execute_undo(&batch_id);
        assert!(result.is_err());
        match result.unwrap_err() {
            MediError::UndoNotEligible { batch_id: bid, .. } => {
                assert_eq!(bid, batch_id);
            }
            other => panic!("expected UndoNotEligible, got: {other:?}"),
        }
    }

    /// Regression test for R003: partial undo should delete successfully-moved
    /// entries from DB while keeping failed entries for retry.
    #[test]
    fn test_partial_undo_removes_only_successful_entries_from_db() {
        let dir = TempDir::new().unwrap();
        let db = HistoryDb::open(&dir.path().join("test.db")).unwrap();

        let source_dir = dir.path().join("source_dir");
        let dest_dir = dir.path().join("dest_dir");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::create_dir_all(&dest_dir).unwrap();

        // Entry 1: will succeed — normal source path
        let source1 = source_dir.join("movie1.mkv");
        let dest1 = dest_dir.join("movie1.mkv");
        std::fs::write(&dest1, "movie1").unwrap();

        // Entry 2: will fail — source parent is blocked by a regular file
        // so create_dir_all fails when trying to create the parent dir
        let blocker = dir.path().join("blocker_file");
        std::fs::write(&blocker, "I am a file, not a directory").unwrap();
        let source2 = blocker.join("subdir").join("movie2.mkv");
        let dest2 = dest_dir.join("movie2.mkv");
        std::fs::write(&dest2, "movie2").unwrap();

        let batch_id = HistoryDb::generate_batch_id();
        let record1 = make_record(
            &batch_id,
            &source1,
            &dest1,
            std::fs::metadata(&dest1).unwrap().len(),
        );
        let record2 = make_record(
            &batch_id,
            &source2,
            &dest2,
            std::fs::metadata(&dest2).unwrap().len(),
        );

        db.record_batch(&[record1, record2]).unwrap();

        // Verify batch has 2 entries
        let entries_before = db.get_batch(&batch_id).unwrap();
        assert_eq!(entries_before.len(), 2);

        let results = db.execute_undo(&batch_id).unwrap();
        assert_eq!(results.len(), 2);

        // Entry 1 should succeed, entry 2 should fail
        let successes: Vec<_> = results.iter().filter(|r| r.success).collect();
        let failures: Vec<_> = results.iter().filter(|r| !r.success).collect();
        assert_eq!(successes.len(), 1, "one entry should have been undone");
        assert_eq!(failures.len(), 1, "one entry should have failed");

        // Movie1 should be back at source
        assert!(source1.exists());

        // DB should only contain the failed entry
        let remaining = db.get_batch(&batch_id).unwrap();
        assert_eq!(
            remaining.len(),
            1,
            "only the failed entry should remain in DB"
        );
        assert_eq!(remaining[0].dest_path, dest2);
    }

    #[test]
    fn test_media_info_json_round_trip() {
        let dir = TempDir::new().unwrap();
        let db = HistoryDb::open(&dir.path().join("test.db")).unwrap();

        let batch_id = HistoryDb::generate_batch_id();
        let original_info = test_media_info();
        let record = make_record(
            &batch_id,
            Path::new("/src/a.mkv"),
            Path::new("/dst/a.mkv"),
            100,
        );

        db.record_batch(&[record]).unwrap();

        let retrieved = db.get_batch(&batch_id).unwrap();
        assert_eq!(retrieved.len(), 1);

        let stored_info = &retrieved[0].media_info;
        assert_eq!(stored_info.title, original_info.title);
        assert_eq!(stored_info.media_type, original_info.media_type);
        assert_eq!(stored_info.year, original_info.year);
        assert_eq!(stored_info.resolution, original_info.resolution);
        assert_eq!(stored_info.video_codec, original_info.video_codec);
        assert_eq!(stored_info.release_group, original_info.release_group);
        assert_eq!(stored_info.container, original_info.container);
    }

    #[test]
    fn test_generate_batch_id_is_uuid_v4() {
        let id = HistoryDb::generate_batch_id();
        // UUID v4 format: 8-4-4-4-12 hex chars
        assert_eq!(id.len(), 36);
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.len(), 5);
        assert_eq!(parts[0].len(), 8);
        assert_eq!(parts[1].len(), 4);
        assert_eq!(parts[2].len(), 4);
        assert_eq!(parts[3].len(), 4);
        assert_eq!(parts[4].len(), 12);
        // Version nibble should be 4
        assert!(parts[2].starts_with('4'));
    }

    // -----------------------------------------------------------------------
    // Watcher tables schema
    // -----------------------------------------------------------------------

    #[test]
    fn test_open_creates_watcher_tables() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db = HistoryDb::open(&db_path).unwrap();

        // Verify watcher_events table exists
        let mut stmt = db
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap();
        let tables: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(
            tables.contains(&"watcher_events".to_string()),
            "watcher_events table should exist"
        );
        assert!(
            tables.contains(&"review_queue".to_string()),
            "review_queue table should exist"
        );
        assert!(
            tables.contains(&"rename_history".to_string()),
            "rename_history should still exist"
        );

        // Verify watcher indexes exist
        let mut stmt = db
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='index'")
            .unwrap();
        let indexes: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(indexes.contains(&"idx_watcher_events_watch_path".to_string()));
        assert!(indexes.contains(&"idx_watcher_events_timestamp".to_string()));
        assert!(indexes.contains(&"idx_review_queue_watch_path".to_string()));
        assert!(indexes.contains(&"idx_review_queue_status".to_string()));
    }

    // -----------------------------------------------------------------------
    // Watcher event CRUD
    // -----------------------------------------------------------------------

    fn make_watcher_event(
        watch_path: &Path,
        filename: &str,
        action: WatcherAction,
    ) -> WatcherEvent {
        WatcherEvent {
            id: None,
            timestamp: "2024-06-15T10:00:00Z".to_string(),
            watch_path: watch_path.to_path_buf(),
            filename: filename.to_string(),
            action,
            detail: Some("test detail".to_string()),
            batch_id: None,
        }
    }

    #[test]
    fn test_log_and_list_watcher_events() {
        let dir = TempDir::new().unwrap();
        let db = HistoryDb::open(&dir.path().join("test.db")).unwrap();

        let wp = Path::new("/watch/movies");
        let event = make_watcher_event(wp, "Movie.2024.mkv", WatcherAction::Renamed);

        db.log_watcher_event(&event).unwrap();

        let events = db.list_watcher_events(Some(wp), None).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].filename, "Movie.2024.mkv");
        assert_eq!(events[0].action, WatcherAction::Renamed);
        assert_eq!(events[0].watch_path, wp.to_path_buf());
        assert!(events[0].id.is_some());
    }

    #[test]
    fn test_list_watcher_events_filters_by_watch_path() {
        let dir = TempDir::new().unwrap();
        let db = HistoryDb::open(&dir.path().join("test.db")).unwrap();

        let wp1 = Path::new("/watch/movies");
        let wp2 = Path::new("/watch/series");

        db.log_watcher_event(&make_watcher_event(
            wp1,
            "movie.mkv",
            WatcherAction::Renamed,
        ))
        .unwrap();
        db.log_watcher_event(&make_watcher_event(
            wp2,
            "series.mkv",
            WatcherAction::Queued,
        ))
        .unwrap();

        let events_wp1 = db.list_watcher_events(Some(wp1), None).unwrap();
        assert_eq!(events_wp1.len(), 1);
        assert_eq!(events_wp1[0].filename, "movie.mkv");

        let events_all = db.list_watcher_events(None, None).unwrap();
        assert_eq!(events_all.len(), 2);
    }

    #[test]
    fn test_prune_watcher_events_keeps_max() {
        let dir = TempDir::new().unwrap();
        let db = HistoryDb::open(&dir.path().join("test.db")).unwrap();

        let wp = Path::new("/watch/movies");

        for i in 0..5 {
            let mut event = make_watcher_event(wp, &format!("file{i}.mkv"), WatcherAction::Renamed);
            event.timestamp = format!("2024-06-15T10:0{i}:00Z");
            db.log_watcher_event(&event).unwrap();
        }

        let events_before = db.list_watcher_events(Some(wp), None).unwrap();
        assert_eq!(events_before.len(), 5);

        let deleted = db.prune_watcher_events(wp, 2).unwrap();
        assert_eq!(deleted, 3);

        let events_after = db.list_watcher_events(Some(wp), None).unwrap();
        assert_eq!(events_after.len(), 2);
    }

    // -----------------------------------------------------------------------
    // Review queue CRUD
    // -----------------------------------------------------------------------

    fn make_review_entry(watch_path: &Path, source: &str, proposed: &str) -> ReviewQueueEntry {
        ReviewQueueEntry {
            id: None,
            timestamp: "2024-06-15T10:00:00Z".to_string(),
            watch_path: watch_path.to_path_buf(),
            source_path: PathBuf::from(source),
            proposed_path: PathBuf::from(proposed),
            media_info_json: r#"{"title":"Test"}"#.to_string(),
            subtitles_json: "[]".to_string(),
            status: ReviewStatus::Pending,
        }
    }

    #[test]
    fn test_add_and_list_review_queue() {
        let dir = TempDir::new().unwrap();
        let db = HistoryDb::open(&dir.path().join("test.db")).unwrap();

        let wp = Path::new("/watch/movies");
        let entry = make_review_entry(wp, "/src/movie.mkv", "/dst/movie.mkv");

        let id = db.add_to_review_queue(&entry).unwrap();
        assert!(id > 0);

        let entries = db.list_review_queue(None, None).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source_path, PathBuf::from("/src/movie.mkv"));
        assert_eq!(entries[0].status, ReviewStatus::Pending);
    }

    #[test]
    fn test_update_review_status() {
        let dir = TempDir::new().unwrap();
        let db = HistoryDb::open(&dir.path().join("test.db")).unwrap();

        let wp = Path::new("/watch/movies");
        let entry = make_review_entry(wp, "/src/movie.mkv", "/dst/movie.mkv");
        let id = db.add_to_review_queue(&entry).unwrap();

        db.update_review_status(id, ReviewStatus::Approved).unwrap();

        let entries = db.list_review_queue(None, None).unwrap();
        assert_eq!(entries[0].status, ReviewStatus::Approved);
    }

    #[test]
    fn test_list_review_queue_filters() {
        let dir = TempDir::new().unwrap();
        let db = HistoryDb::open(&dir.path().join("test.db")).unwrap();

        let wp1 = Path::new("/watch/movies");
        let wp2 = Path::new("/watch/series");

        db.add_to_review_queue(&make_review_entry(wp1, "/src/movie.mkv", "/dst/movie.mkv"))
            .unwrap();
        let id2 = db
            .add_to_review_queue(&make_review_entry(
                wp2,
                "/src/series.mkv",
                "/dst/series.mkv",
            ))
            .unwrap();

        // Approve second entry
        db.update_review_status(id2, ReviewStatus::Approved)
            .unwrap();

        // Filter by watch_path
        let movies = db.list_review_queue(Some(wp1), None).unwrap();
        assert_eq!(movies.len(), 1);
        assert_eq!(movies[0].source_path, PathBuf::from("/src/movie.mkv"));

        // Filter by status
        let pending = db
            .list_review_queue(None, Some(ReviewStatus::Pending))
            .unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].source_path, PathBuf::from("/src/movie.mkv"));

        let approved = db
            .list_review_queue(None, Some(ReviewStatus::Approved))
            .unwrap();
        assert_eq!(approved.len(), 1);
        assert_eq!(approved[0].source_path, PathBuf::from("/src/series.mkv"));

        // Filter by both watch_path and status
        let wp1_pending = db
            .list_review_queue(Some(wp1), Some(ReviewStatus::Pending))
            .unwrap();
        assert_eq!(wp1_pending.len(), 1);

        let wp2_pending = db
            .list_review_queue(Some(wp2), Some(ReviewStatus::Pending))
            .unwrap();
        assert_eq!(wp2_pending.len(), 0);
    }

    #[test]
    fn test_remove_review_entry() {
        let dir = TempDir::new().unwrap();
        let db = HistoryDb::open(&dir.path().join("test.db")).unwrap();

        let wp = Path::new("/watch/movies");
        let id = db
            .add_to_review_queue(&make_review_entry(wp, "/src/movie.mkv", "/dst/movie.mkv"))
            .unwrap();

        db.remove_review_entry(id).unwrap();

        let entries = db.list_review_queue(None, None).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_wal_mode_enabled() {
        let dir = tempfile::TempDir::new().unwrap();
        let db = HistoryDb::open(&dir.path().join("test.db")).unwrap();
        let mode: String = db
            .conn
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
        assert_eq!(mode, "wal", "Journal mode should be WAL");
    }

    #[test]
    fn test_composite_index_exists() {
        let dir = tempfile::TempDir::new().unwrap();
        let db = HistoryDb::open(&dir.path().join("test.db")).unwrap();
        let mut stmt = db
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='index'")
            .unwrap();
        let indexes: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(
            indexes.contains(&"idx_watcher_events_path_time".to_string()),
            "Composite index idx_watcher_events_path_time should exist"
        );
    }
}
