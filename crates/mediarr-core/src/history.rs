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
use crate::types::{BatchSummary, MediaInfo, RenameRecord, RenameResult, UndoEligibility, UndoIssue};

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
        conn.execute_batch(SCHEMA)?;

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
                let media_info: MediaInfo = serde_json::from_str(&media_json)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                        4,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    ))?;

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

        // If all moves succeeded, remove batch from database
        if results.iter().all(|r| r.success) {
            self.conn.execute(
                "DELETE FROM rename_history WHERE batch_id = ?1",
                params![batch_id],
            )?;
            info!(batch_id, "undo complete, batch removed from history");
        }

        Ok(results)
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
        let tables: Vec<String> = stmt.query_map([], |row| row.get(0)).unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(tables, vec!["rename_history"]);

        // Verify indexes exist
        let mut stmt = db
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'")
            .unwrap();
        let indexes: Vec<String> = stmt.query_map([], |row| row.get(0)).unwrap()
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
            make_record(&batch_id, Path::new("/src/a.mkv"), Path::new("/dst/a.mkv"), 1000),
            make_record(&batch_id, Path::new("/src/b.mkv"), Path::new("/dst/b.mkv"), 2000),
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
            make_record(&batch_id, Path::new("/src/a.mkv"), Path::new("/dst/a.mkv"), 1000),
            make_record(&batch_id, Path::new("/src/b.mkv"), Path::new("/dst/b.mkv"), 2000),
            make_record(&batch_id, Path::new("/src/c.mkv"), Path::new("/dst/c.mkv"), 3000),
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
        let mut old_record =
            make_record(&batch_old, Path::new("/src/old.mkv"), Path::new("/dst/old.mkv"), 100);
        old_record.timestamp = "2024-01-01T00:00:00Z".to_string();
        db.record_batch(&[old_record]).unwrap();

        // Insert newer batch
        let batch_new = "batch-new".to_string();
        let mut new_record =
            make_record(&batch_new, Path::new("/src/new.mkv"), Path::new("/dst/new.mkv"), 200);
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
            make_record(&batch_id, Path::new("/src/a.mkv"), Path::new("/dst/a.mkv"), 1000),
            make_record(&batch_id, Path::new("/src/b.mkv"), Path::new("/dst/b.mkv"), 2000),
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
        assert_eq!(eligibility.ineligible_reasons[0].reason, "destination file missing");
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
        assert_eq!(eligibility.ineligible_reasons[0].reason, "source location occupied");
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

    #[test]
    fn test_media_info_json_round_trip() {
        let dir = TempDir::new().unwrap();
        let db = HistoryDb::open(&dir.path().join("test.db")).unwrap();

        let batch_id = HistoryDb::generate_batch_id();
        let original_info = test_media_info();
        let record = make_record(&batch_id, Path::new("/src/a.mkv"), Path::new("/dst/a.mkv"), 100);

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
}
