//! Rename execution engine for Mediarr.
//!
//! Takes a rename plan (list of source->dest pairs) and executes it safely.
//! Supports dry-run mode, three conflict resolution strategies, and batch
//! execution that stops on first failure. Uses shared `safe_move` from
//! `fs_util` for cross-filesystem EXDEV handling.

use std::path::{Path, PathBuf};

use tracing::{debug, info, warn};

use crate::config::GeneralConfig;
use crate::error::Result;
use crate::types::{ConflictStrategy, RenameOperation, RenameResult};

/// A single entry in a rename plan: source -> dest.
#[derive(Debug, Clone)]
pub struct RenamePlanEntry {
    /// Original file path.
    pub source_path: PathBuf,
    /// Proposed destination path.
    pub dest_path: PathBuf,
}

/// A collection of rename operations to execute.
#[derive(Debug, Clone)]
pub struct RenamePlan {
    /// Ordered list of rename entries.
    pub entries: Vec<RenamePlanEntry>,
}

/// Rename execution engine.
///
/// Configurable with operation type (move/copy), conflict strategy, and
/// directory creation. Provides both dry-run (validation only) and execute
/// (actually rename files) modes.
pub struct Renamer {
    /// Whether to move or copy files.
    pub operation: RenameOperation,
    /// How to handle target path conflicts.
    pub conflict_strategy: ConflictStrategy,
    /// Whether to create target directories that don't exist.
    pub create_directories: bool,
}

impl Renamer {
    /// Create a new Renamer with explicit settings.
    pub fn new(
        operation: RenameOperation,
        conflict_strategy: ConflictStrategy,
        create_directories: bool,
    ) -> Self {
        Self {
            operation,
            conflict_strategy,
            create_directories,
        }
    }

    /// Create a Renamer from the general config section.
    pub fn from_config(config: &GeneralConfig) -> Self {
        Self::new(
            config.operation,
            config.conflict_strategy,
            config.create_directories,
        )
    }

    /// Validate a rename plan without touching the filesystem.
    ///
    /// Checks for conflicts (existing targets, duplicate destinations within
    /// the plan) and applies the configured conflict strategy to each entry.
    /// Returns a `RenameResult` for each entry showing what *would* happen.
    pub fn dry_run(&self, _plan: &RenamePlan) -> Vec<RenameResult> {
        todo!("dry_run not yet implemented")
    }

    /// Execute a rename plan, moving or copying files.
    ///
    /// Processes entries in order. On failure, stops immediately and returns
    /// results for all completed entries plus the failed one. Remaining entries
    /// are not attempted (RENM-05: stop on failure).
    pub fn execute(&self, _plan: &RenamePlan) -> Vec<RenameResult> {
        todo!("execute not yet implemented")
    }
}

/// Find the next available numeric-suffixed path to avoid conflicts.
///
/// Given a path like `/dest/Movie.mkv`, tries `/dest/Movie (1).mkv`,
/// `/dest/Movie (2).mkv`, etc. up to 99. Returns the first non-existing path.
fn resolve_numeric_suffix(_dest: &Path) -> PathBuf {
    todo!("resolve_numeric_suffix not yet implemented")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use tempfile::TempDir;

    // -----------------------------------------------------------------------
    // Dry-run tests
    // -----------------------------------------------------------------------

    #[test]
    fn dry_run_valid_plan_returns_ok_without_creating_files() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("movie.mkv");
        std::fs::write(&src, b"video content").unwrap();

        let dest = dir.path().join("output").join("Movie.mkv");

        let renamer = Renamer::new(
            RenameOperation::Move,
            ConflictStrategy::Skip,
            true,
        );

        let plan = RenamePlan {
            entries: vec![RenamePlanEntry {
                source_path: src,
                dest_path: dest.clone(),
            }],
        };

        let results = renamer.dry_run(&plan);
        assert_eq!(results.len(), 1);
        assert!(results[0].success);
        // Dry run must NOT create the output directory or file
        assert!(!dest.exists());
        assert!(!dest.parent().unwrap().exists());
    }

    #[test]
    fn dry_run_detects_conflict_when_target_exists() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("movie.mkv");
        std::fs::write(&src, b"source").unwrap();
        let dest = dir.path().join("existing.mkv");
        std::fs::write(&dest, b"existing").unwrap();

        let renamer = Renamer::new(
            RenameOperation::Move,
            ConflictStrategy::Skip,
            true,
        );

        let plan = RenamePlan {
            entries: vec![RenamePlanEntry {
                source_path: src,
                dest_path: dest,
            }],
        };

        let results = renamer.dry_run(&plan);
        assert_eq!(results.len(), 1);
        assert!(!results[0].success);
        assert!(results[0].error.as_ref().unwrap().contains("skipped"));
    }

    #[test]
    fn dry_run_detects_duplicate_targets_in_plan() {
        let dir = TempDir::new().unwrap();
        let src1 = dir.path().join("movie1.mkv");
        let src2 = dir.path().join("movie2.mkv");
        std::fs::write(&src1, b"vid1").unwrap();
        std::fs::write(&src2, b"vid2").unwrap();

        let shared_dest = dir.path().join("output.mkv");

        let renamer = Renamer::new(
            RenameOperation::Move,
            ConflictStrategy::Skip,
            true,
        );

        let plan = RenamePlan {
            entries: vec![
                RenamePlanEntry {
                    source_path: src1,
                    dest_path: shared_dest.clone(),
                },
                RenamePlanEntry {
                    source_path: src2,
                    dest_path: shared_dest,
                },
            ],
        };

        let results = renamer.dry_run(&plan);
        assert_eq!(results.len(), 2);
        // First should succeed, second should be skipped (duplicate)
        assert!(results[0].success);
        assert!(!results[1].success);
        assert!(results[1].error.as_ref().unwrap().contains("skipped"));
    }

    #[test]
    fn dry_run_with_numeric_suffix_computes_suffixed_path() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("movie.mkv");
        std::fs::write(&src, b"source").unwrap();
        let dest = dir.path().join("existing.mkv");
        std::fs::write(&dest, b"existing").unwrap();

        let renamer = Renamer::new(
            RenameOperation::Move,
            ConflictStrategy::NumericSuffix,
            true,
        );

        let plan = RenamePlan {
            entries: vec![RenamePlanEntry {
                source_path: src,
                dest_path: dest.clone(),
            }],
        };

        let results = renamer.dry_run(&plan);
        assert_eq!(results.len(), 1);
        assert!(results[0].success);
        // Should have resolved to "existing (1).mkv"
        let expected_suffix = dir.path().join("existing (1).mkv");
        assert_eq!(results[0].dest_path, expected_suffix);
    }

    // -----------------------------------------------------------------------
    // Execute - Move tests
    // -----------------------------------------------------------------------

    #[test]
    fn execute_moves_file_to_destination() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("movie.mkv");
        std::fs::write(&src, b"video data").unwrap();
        let dest = dir.path().join("Movie.mkv");

        let renamer = Renamer::new(
            RenameOperation::Move,
            ConflictStrategy::Skip,
            true,
        );

        let plan = RenamePlan {
            entries: vec![RenamePlanEntry {
                source_path: src.clone(),
                dest_path: dest.clone(),
            }],
        };

        let results = renamer.execute(&plan);
        assert_eq!(results.len(), 1);
        assert!(results[0].success);
        assert!(!src.exists(), "source should no longer exist after Move");
        assert!(dest.exists(), "dest should exist after Move");
        assert_eq!(std::fs::read(&dest).unwrap(), b"video data");
    }

    #[test]
    fn execute_creates_target_directory() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("movie.mkv");
        std::fs::write(&src, b"content").unwrap();
        let dest = dir.path().join("nested").join("deep").join("Movie.mkv");

        let renamer = Renamer::new(
            RenameOperation::Move,
            ConflictStrategy::Skip,
            true,
        );

        let plan = RenamePlan {
            entries: vec![RenamePlanEntry {
                source_path: src,
                dest_path: dest.clone(),
            }],
        };

        let results = renamer.execute(&plan);
        assert_eq!(results.len(), 1);
        assert!(results[0].success);
        assert!(dest.exists());
    }

    #[test]
    fn execute_source_gone_after_move() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("source.mkv");
        std::fs::write(&src, b"data").unwrap();
        let dest = dir.path().join("dest.mkv");

        let renamer = Renamer::new(
            RenameOperation::Move,
            ConflictStrategy::Skip,
            true,
        );

        let plan = RenamePlan {
            entries: vec![RenamePlanEntry {
                source_path: src.clone(),
                dest_path: dest,
            }],
        };

        renamer.execute(&plan);
        assert!(!src.exists(), "source file must not exist after move");
    }

    // -----------------------------------------------------------------------
    // Execute - Copy tests
    // -----------------------------------------------------------------------

    #[test]
    fn execute_copy_leaves_source_in_place() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("movie.mkv");
        std::fs::write(&src, b"video data").unwrap();
        let dest = dir.path().join("Copy.mkv");

        let renamer = Renamer::new(
            RenameOperation::Copy,
            ConflictStrategy::Skip,
            true,
        );

        let plan = RenamePlan {
            entries: vec![RenamePlanEntry {
                source_path: src.clone(),
                dest_path: dest.clone(),
            }],
        };

        let results = renamer.execute(&plan);
        assert_eq!(results.len(), 1);
        assert!(results[0].success);
        assert!(src.exists(), "source must still exist after Copy");
        assert!(dest.exists(), "dest must exist after Copy");
        assert_eq!(std::fs::read(&dest).unwrap(), b"video data");
    }

    #[test]
    fn execute_copy_verifies_file_sizes_match() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("movie.mkv");
        std::fs::write(&src, b"video data for size check").unwrap();
        let dest = dir.path().join("Copied.mkv");

        let renamer = Renamer::new(
            RenameOperation::Copy,
            ConflictStrategy::Skip,
            true,
        );

        let plan = RenamePlan {
            entries: vec![RenamePlanEntry {
                source_path: src.clone(),
                dest_path: dest.clone(),
            }],
        };

        let results = renamer.execute(&plan);
        assert!(results[0].success);
        // Verify sizes match
        let src_size = std::fs::metadata(&src).unwrap().len();
        let dest_size = std::fs::metadata(&dest).unwrap().len();
        assert_eq!(src_size, dest_size);
    }

    // -----------------------------------------------------------------------
    // Conflict resolution tests
    // -----------------------------------------------------------------------

    #[test]
    fn conflict_skip_leaves_existing_file_untouched() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("movie.mkv");
        std::fs::write(&src, b"new content").unwrap();
        let dest = dir.path().join("existing.mkv");
        std::fs::write(&dest, b"original content").unwrap();

        let renamer = Renamer::new(
            RenameOperation::Move,
            ConflictStrategy::Skip,
            true,
        );

        let plan = RenamePlan {
            entries: vec![RenamePlanEntry {
                source_path: src.clone(),
                dest_path: dest.clone(),
            }],
        };

        let results = renamer.execute(&plan);
        assert_eq!(results.len(), 1);
        assert!(!results[0].success);
        assert!(results[0].error.as_ref().unwrap().contains("skipped"));
        // Existing file content unchanged
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "original content");
        // Source still exists (was not moved)
        assert!(src.exists());
    }

    #[test]
    fn conflict_overwrite_replaces_existing_file() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("movie.mkv");
        std::fs::write(&src, b"new content").unwrap();
        let dest = dir.path().join("existing.mkv");
        std::fs::write(&dest, b"old content").unwrap();

        let renamer = Renamer::new(
            RenameOperation::Move,
            ConflictStrategy::Overwrite,
            true,
        );

        let plan = RenamePlan {
            entries: vec![RenamePlanEntry {
                source_path: src,
                dest_path: dest.clone(),
            }],
        };

        let results = renamer.execute(&plan);
        assert_eq!(results.len(), 1);
        assert!(results[0].success);
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "new content");
    }

    #[test]
    fn conflict_numeric_suffix_appends_number() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("movie.mkv");
        std::fs::write(&src, b"new data").unwrap();
        let dest = dir.path().join("Movie.mkv");
        std::fs::write(&dest, b"existing data").unwrap();

        let renamer = Renamer::new(
            RenameOperation::Move,
            ConflictStrategy::NumericSuffix,
            true,
        );

        let plan = RenamePlan {
            entries: vec![RenamePlanEntry {
                source_path: src,
                dest_path: dest.clone(),
            }],
        };

        let results = renamer.execute(&plan);
        assert_eq!(results.len(), 1);
        assert!(results[0].success);
        // Original still intact
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "existing data");
        // New file at suffixed path
        let suffixed = dir.path().join("Movie (1).mkv");
        assert!(suffixed.exists());
        assert_eq!(std::fs::read_to_string(&suffixed).unwrap(), "new data");
        assert_eq!(results[0].dest_path, suffixed);
    }

    #[test]
    fn conflict_numeric_suffix_increments_past_existing() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("movie.mkv");
        std::fs::write(&src, b"data").unwrap();
        let dest = dir.path().join("Movie.mkv");
        std::fs::write(&dest, b"original").unwrap();
        // Also create (1) so it has to go to (2)
        let suffix1 = dir.path().join("Movie (1).mkv");
        std::fs::write(&suffix1, b"first copy").unwrap();

        let renamer = Renamer::new(
            RenameOperation::Move,
            ConflictStrategy::NumericSuffix,
            true,
        );

        let plan = RenamePlan {
            entries: vec![RenamePlanEntry {
                source_path: src,
                dest_path: dest,
            }],
        };

        let results = renamer.execute(&plan);
        assert!(results[0].success);
        let suffix2 = dir.path().join("Movie (2).mkv");
        assert!(suffix2.exists());
        assert_eq!(results[0].dest_path, suffix2);
    }

    // -----------------------------------------------------------------------
    // Batch tests
    // -----------------------------------------------------------------------

    #[test]
    fn batch_stops_on_first_failure() {
        let dir = TempDir::new().unwrap();
        let src1 = dir.path().join("file1.mkv");
        std::fs::write(&src1, b"data1").unwrap();
        // src2 does not exist -> will cause failure
        let src2 = dir.path().join("nonexistent.mkv");
        let src3 = dir.path().join("file3.mkv");
        std::fs::write(&src3, b"data3").unwrap();

        let renamer = Renamer::new(
            RenameOperation::Move,
            ConflictStrategy::Skip,
            true,
        );

        let plan = RenamePlan {
            entries: vec![
                RenamePlanEntry {
                    source_path: src1,
                    dest_path: dir.path().join("dest1.mkv"),
                },
                RenamePlanEntry {
                    source_path: src2,
                    dest_path: dir.path().join("dest2.mkv"),
                },
                RenamePlanEntry {
                    source_path: src3,
                    dest_path: dir.path().join("dest3.mkv"),
                },
            ],
        };

        let results = renamer.execute(&plan);
        // Should have 2 results: first success, second failure. Third not attempted.
        assert_eq!(results.len(), 2);
        assert!(results[0].success);
        assert!(!results[1].success);
        // Third file should not have been moved
        assert!(!dir.path().join("dest3.mkv").exists());
    }

    #[test]
    fn source_files_never_deleted_only_moved() {
        // This test verifies Move uses rename/safe_move semantics, not delete
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("movie.mkv");
        let content = b"important video data";
        std::fs::write(&src, content).unwrap();
        let dest = dir.path().join("Movie.mkv");

        let renamer = Renamer::new(
            RenameOperation::Move,
            ConflictStrategy::Skip,
            true,
        );

        let plan = RenamePlan {
            entries: vec![RenamePlanEntry {
                source_path: src.clone(),
                dest_path: dest.clone(),
            }],
        };

        let results = renamer.execute(&plan);
        assert!(results[0].success);
        // Data preserved at destination
        assert_eq!(std::fs::read(&dest).unwrap(), content);
        // Source is gone (moved, not deleted)
        assert!(!src.exists());
    }

    // -----------------------------------------------------------------------
    // resolve_numeric_suffix tests
    // -----------------------------------------------------------------------

    #[test]
    fn resolve_suffix_returns_first_available() {
        let dir = TempDir::new().unwrap();
        let dest = dir.path().join("Movie.mkv");
        std::fs::write(&dest, b"existing").unwrap();

        let resolved = resolve_numeric_suffix(&dest);
        assert_eq!(resolved, dir.path().join("Movie (1).mkv"));
    }

    #[test]
    fn resolve_suffix_skips_existing_suffixes() {
        let dir = TempDir::new().unwrap();
        let dest = dir.path().join("Movie.mkv");
        std::fs::write(&dest, b"existing").unwrap();
        std::fs::write(dir.path().join("Movie (1).mkv"), b"copy1").unwrap();
        std::fs::write(dir.path().join("Movie (2).mkv"), b"copy2").unwrap();

        let resolved = resolve_numeric_suffix(&dest);
        assert_eq!(resolved, dir.path().join("Movie (3).mkv"));
    }

    // -----------------------------------------------------------------------
    // from_config test
    // -----------------------------------------------------------------------

    #[test]
    fn from_config_reads_settings() {
        let config = GeneralConfig {
            output_dir: None,
            operation: RenameOperation::Copy,
            conflict_strategy: ConflictStrategy::NumericSuffix,
            create_directories: false,
        };

        let renamer = Renamer::from_config(&config);
        assert_eq!(renamer.operation, RenameOperation::Copy);
        assert_eq!(renamer.conflict_strategy, ConflictStrategy::NumericSuffix);
        assert!(!renamer.create_directories);
    }
}
