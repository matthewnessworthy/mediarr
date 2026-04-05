//! Rename execution engine for Mediarr.
//!
//! Takes a rename plan (list of source->dest pairs) and executes it safely.
//! Supports dry-run mode, three conflict resolution strategies, and batch
//! execution that stops on first failure. Uses shared `safe_move` from
//! `fs_util` for cross-filesystem EXDEV handling.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use tracing::{debug, info, warn};

use crate::config::GeneralConfig;
use crate::error::{MediError, Result};
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
    pub fn dry_run(&self, plan: &RenamePlan) -> Vec<RenameResult> {
        let mut results = Vec::with_capacity(plan.entries.len());
        let mut seen_dests: HashSet<PathBuf> = HashSet::new();

        for entry in &plan.entries {
            let conflict = entry.dest_path.exists() || seen_dests.contains(&entry.dest_path);

            if conflict {
                match self.conflict_strategy {
                    ConflictStrategy::Skip => {
                        debug!(
                            source = %entry.source_path.display(),
                            dest = %entry.dest_path.display(),
                            "dry_run: skipping conflict"
                        );
                        results.push(RenameResult {
                            source_path: entry.source_path.clone(),
                            dest_path: entry.dest_path.clone(),
                            success: false,
                            error: Some("skipped: target already exists".into()),
                        });
                    }
                    ConflictStrategy::Overwrite => {
                        debug!(
                            source = %entry.source_path.display(),
                            dest = %entry.dest_path.display(),
                            "dry_run: would overwrite"
                        );
                        seen_dests.insert(entry.dest_path.clone());
                        results.push(RenameResult {
                            source_path: entry.source_path.clone(),
                            dest_path: entry.dest_path.clone(),
                            success: true,
                            error: None,
                        });
                    }
                    ConflictStrategy::NumericSuffix => {
                        match resolve_numeric_suffix(&entry.dest_path) {
                            Ok(suffixed) => {
                                debug!(
                                    source = %entry.source_path.display(),
                                    dest = %suffixed.display(),
                                    "dry_run: would use numeric suffix"
                                );
                                seen_dests.insert(suffixed.clone());
                                results.push(RenameResult {
                                    source_path: entry.source_path.clone(),
                                    dest_path: suffixed,
                                    success: true,
                                    error: None,
                                });
                            }
                            Err(e) => {
                                results.push(RenameResult {
                                    source_path: entry.source_path.clone(),
                                    dest_path: entry.dest_path.clone(),
                                    success: false,
                                    error: Some(e.to_string()),
                                });
                            }
                        }
                    }
                }
            } else {
                seen_dests.insert(entry.dest_path.clone());
                results.push(RenameResult {
                    source_path: entry.source_path.clone(),
                    dest_path: entry.dest_path.clone(),
                    success: true,
                    error: None,
                });
            }
        }

        info!(entries = plan.entries.len(), "dry_run complete");
        results
    }

    /// Execute a rename plan, moving or copying files.
    ///
    /// Processes entries in order. On failure, stops immediately and returns
    /// results for all completed entries plus the failed one. Remaining entries
    /// are not attempted (RENM-05: stop on failure).
    pub fn execute(&self, plan: &RenamePlan) -> Vec<RenameResult> {
        let mut results = Vec::with_capacity(plan.entries.len());

        for entry in &plan.entries {
            // Determine effective destination (may change due to conflict resolution)
            let effective_dest = if entry.dest_path.exists() {
                match self.conflict_strategy {
                    ConflictStrategy::Skip => {
                        info!(
                            source = %entry.source_path.display(),
                            dest = %entry.dest_path.display(),
                            "skipping: target already exists"
                        );
                        results.push(RenameResult {
                            source_path: entry.source_path.clone(),
                            dest_path: entry.dest_path.clone(),
                            success: false,
                            error: Some("skipped: target already exists".into()),
                        });
                        continue;
                    }
                    ConflictStrategy::Overwrite => {
                        debug!(
                            dest = %entry.dest_path.display(),
                            "overwriting existing target"
                        );
                        // Remove existing file before move/copy
                        if let Err(e) = std::fs::remove_file(&entry.dest_path) {
                            warn!(
                                dest = %entry.dest_path.display(),
                                error = %e,
                                "failed to remove existing target for overwrite"
                            );
                            results.push(RenameResult {
                                source_path: entry.source_path.clone(),
                                dest_path: entry.dest_path.clone(),
                                success: false,
                                error: Some(format!("overwrite failed: {e}")),
                            });
                            break;
                        }
                        entry.dest_path.clone()
                    }
                    ConflictStrategy::NumericSuffix => {
                        match resolve_numeric_suffix(&entry.dest_path) {
                            Ok(suffixed) => {
                                debug!(
                                    original = %entry.dest_path.display(),
                                    suffixed = %suffixed.display(),
                                    "using numeric suffix to avoid conflict"
                                );
                                suffixed
                            }
                            Err(e) => {
                                results.push(RenameResult {
                                    source_path: entry.source_path.clone(),
                                    dest_path: entry.dest_path.clone(),
                                    success: false,
                                    error: Some(e.to_string()),
                                });
                                continue;
                            }
                        }
                    }
                }
            } else {
                entry.dest_path.clone()
            };

            // Create target directories if configured
            if self.create_directories {
                if let Some(parent) = effective_dest.parent() {
                    if !parent.as_os_str().is_empty() {
                        if let Err(e) = std::fs::create_dir_all(parent) {
                            results.push(RenameResult {
                                source_path: entry.source_path.clone(),
                                dest_path: effective_dest,
                                success: false,
                                error: Some(format!("failed to create directory: {e}")),
                            });
                            break;
                        }
                    }
                }
            }

            // Perform the operation
            match self.operation {
                RenameOperation::Move => {
                    match crate::fs_util::safe_move(&entry.source_path, &effective_dest) {
                        Ok(()) => {
                            info!(
                                source = %entry.source_path.display(),
                                dest = %effective_dest.display(),
                                "moved file successfully"
                            );
                            results.push(RenameResult {
                                source_path: entry.source_path.clone(),
                                dest_path: effective_dest,
                                success: true,
                                error: None,
                            });
                        }
                        Err(e) => {
                            warn!(
                                source = %entry.source_path.display(),
                                dest = %effective_dest.display(),
                                error = %e,
                                "move failed"
                            );
                            results.push(RenameResult {
                                source_path: entry.source_path.clone(),
                                dest_path: effective_dest,
                                success: false,
                                error: Some(format!("{e}")),
                            });
                            break; // Stop on first failure (RENM-05)
                        }
                    }
                }
                RenameOperation::Copy => {
                    match std::fs::copy(&entry.source_path, &effective_dest) {
                        Ok(_) => {
                            // Verify sizes match
                            let src_size = match std::fs::metadata(&entry.source_path) {
                                Ok(m) => m.len(),
                                Err(e) => {
                                    results.push(RenameResult {
                                        source_path: entry.source_path.clone(),
                                        dest_path: effective_dest,
                                        success: false,
                                        error: Some(format!("copy verify failed: {e}")),
                                    });
                                    break;
                                }
                            };
                            let dest_size = match std::fs::metadata(&effective_dest) {
                                Ok(m) => m.len(),
                                Err(e) => {
                                    results.push(RenameResult {
                                        source_path: entry.source_path.clone(),
                                        dest_path: effective_dest,
                                        success: false,
                                        error: Some(format!("copy verify failed: {e}")),
                                    });
                                    break;
                                }
                            };

                            if src_size != dest_size {
                                // Remove the bad copy
                                let _ = std::fs::remove_file(&effective_dest);
                                results.push(RenameResult {
                                    source_path: entry.source_path.clone(),
                                    dest_path: effective_dest,
                                    success: false,
                                    error: Some("copy verification failed: size mismatch".into()),
                                });
                                break;
                            }

                            info!(
                                source = %entry.source_path.display(),
                                dest = %effective_dest.display(),
                                "copied file successfully"
                            );
                            results.push(RenameResult {
                                source_path: entry.source_path.clone(),
                                dest_path: effective_dest,
                                success: true,
                                error: None,
                            });
                        }
                        Err(e) => {
                            warn!(
                                source = %entry.source_path.display(),
                                dest = %effective_dest.display(),
                                error = %e,
                                "copy failed"
                            );
                            results.push(RenameResult {
                                source_path: entry.source_path.clone(),
                                dest_path: effective_dest,
                                success: false,
                                error: Some(format!("{e}")),
                            });
                            break; // Stop on first failure (RENM-05)
                        }
                    }
                }
            }
        }

        info!(
            total = plan.entries.len(),
            completed = results.len(),
            "execute complete"
        );
        results
    }
}

/// Find the next available numeric-suffixed path to avoid conflicts.
///
/// Given a path like `/dest/Movie.mkv`, tries `/dest/Movie (1).mkv`,
/// `/dest/Movie (2).mkv`, etc. up to 99. Returns the first non-existing path.
///
/// Returns `MediError::ConflictResolutionExhausted` if all 99 suffixes are taken.
fn resolve_numeric_suffix(dest: &Path) -> Result<PathBuf> {
    let parent = dest.parent().unwrap_or_else(|| Path::new(""));
    let stem = dest.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = dest.extension().and_then(|e| e.to_str());

    for i in 1..=99 {
        let filename = match ext {
            Some(e) => format!("{stem} ({i}).{e}"),
            None => format!("{stem} ({i})"),
        };
        let candidate = parent.join(&filename);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    // Exhausted all 99 suffixes -- return error instead of silent overwrite
    Err(MediError::ConflictResolutionExhausted {
        path: dest.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
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

        let renamer = Renamer::new(RenameOperation::Move, ConflictStrategy::Skip, true);

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

        let renamer = Renamer::new(RenameOperation::Move, ConflictStrategy::Skip, true);

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

        let renamer = Renamer::new(RenameOperation::Move, ConflictStrategy::Skip, true);

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

        let renamer = Renamer::new(RenameOperation::Move, ConflictStrategy::NumericSuffix, true);

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
        let dest = dir.path().join("renamed-movie.mkv");

        let renamer = Renamer::new(RenameOperation::Move, ConflictStrategy::Skip, true);

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

        let renamer = Renamer::new(RenameOperation::Move, ConflictStrategy::Skip, true);

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

        let renamer = Renamer::new(RenameOperation::Move, ConflictStrategy::Skip, true);

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

        let renamer = Renamer::new(RenameOperation::Copy, ConflictStrategy::Skip, true);

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

        let renamer = Renamer::new(RenameOperation::Copy, ConflictStrategy::Skip, true);

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

        let renamer = Renamer::new(RenameOperation::Move, ConflictStrategy::Skip, true);

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

        let renamer = Renamer::new(RenameOperation::Move, ConflictStrategy::Overwrite, true);

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
        let src = dir.path().join("new-movie.mkv");
        std::fs::write(&src, b"new data").unwrap();
        let dest = dir.path().join("target.mkv");
        std::fs::write(&dest, b"existing data").unwrap();

        let renamer = Renamer::new(RenameOperation::Move, ConflictStrategy::NumericSuffix, true);

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
        let suffixed = dir.path().join("target (1).mkv");
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

        let renamer = Renamer::new(RenameOperation::Move, ConflictStrategy::NumericSuffix, true);

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

        let renamer = Renamer::new(RenameOperation::Move, ConflictStrategy::Skip, true);

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
        let src = dir.path().join("original-movie.mkv");
        let content = b"important video data";
        std::fs::write(&src, content).unwrap();
        let dest = dir.path().join("renamed-movie.mkv");

        let renamer = Renamer::new(RenameOperation::Move, ConflictStrategy::Skip, true);

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

        let resolved = resolve_numeric_suffix(&dest).unwrap();
        assert_eq!(resolved, dir.path().join("Movie (1).mkv"));
    }

    #[test]
    fn resolve_suffix_skips_existing_suffixes() {
        let dir = TempDir::new().unwrap();
        let dest = dir.path().join("Movie.mkv");
        std::fs::write(&dest, b"existing").unwrap();
        std::fs::write(dir.path().join("Movie (1).mkv"), b"copy1").unwrap();
        std::fs::write(dir.path().join("Movie (2).mkv"), b"copy2").unwrap();

        let resolved = resolve_numeric_suffix(&dest).unwrap();
        assert_eq!(resolved, dir.path().join("Movie (3).mkv"));
    }

    #[test]
    fn resolve_numeric_suffix_errors_when_exhausted() {
        let dir = TempDir::new().unwrap();
        let base = dir.path().join("Movie.mkv");
        std::fs::write(&base, b"base").unwrap();

        // Create all 99 suffix files
        for i in 1..=99 {
            let suffixed = dir.path().join(format!("Movie ({i}).mkv"));
            std::fs::write(&suffixed, b"taken").unwrap();
        }

        let result = resolve_numeric_suffix(&base);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("exhausted"),
            "Error should mention exhaustion: {err}"
        );
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
