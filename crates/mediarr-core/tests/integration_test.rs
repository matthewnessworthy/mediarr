//! Integration tests for mediarr-core: full pipeline round-trips.
//!
//! Tests the complete scan -> plan -> rename -> history -> undo cycle,
//! validating that all Phase 1 modules work together end-to-end.

use mediarr_core::config::Config;
use mediarr_core::history::HistoryDb;
use mediarr_core::renamer::{RenamePlan, RenamePlanEntry, Renamer};
use mediarr_core::scanner::Scanner;
use mediarr_core::types::{MediaType, RenameRecord, ScanFilter, ScanStatus};

use std::fs;
use tempfile::TempDir;

/// Helper: create a Config with output_dir set to given path.
fn config_with_output(output_dir: &std::path::Path) -> Config {
    let mut config = Config::default();
    config.general.output_dir = Some(output_dir.to_path_buf());
    config
}

#[test]
fn test_scan_rename_undo_roundtrip() {
    // 1. Setup: create temp directories
    let source_dir = TempDir::new().unwrap();
    let output_dir = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();

    // Create video files with identifiable content
    let video1 = source_dir
        .path()
        .join("The.Office.S02E03.720p.BluRay.x264-DEMAND.mkv");
    let video2 = source_dir
        .path()
        .join("Inception.2010.1080p.BluRay.x264-GROUP.mkv");
    fs::write(&video1, b"video1 content here").unwrap();
    fs::write(&video2, b"video2 content here").unwrap();

    // Also create a subtitle (will be discovered but not part of rename plan for simplicity)
    let _sub1 = source_dir
        .path()
        .join("The.Office.S02E03.720p.BluRay.x264-DEMAND.en.srt");
    fs::write(&_sub1, b"subtitle content").unwrap();

    // 2. Create Config with output_dir pointing to temp
    let config = config_with_output(output_dir.path());

    // 3. Scan
    let scanner = Scanner::new(config.clone());
    let results = scanner.scan_folder(source_dir.path()).unwrap();
    assert_eq!(results.len(), 2, "Should find 2 video files");

    // Verify media type detection
    let has_series = results
        .iter()
        .any(|r| r.media_info.media_type == MediaType::Series);
    let has_movie = results
        .iter()
        .any(|r| r.media_info.media_type == MediaType::Movie);
    assert!(has_series, "Should detect The Office as Series");
    assert!(has_movie, "Should detect Inception as Movie");

    // Verify subtitles discovered for the series file
    let series_result = results
        .iter()
        .find(|r| r.media_info.media_type == MediaType::Series)
        .unwrap();
    assert!(
        !series_result.subtitles.is_empty(),
        "Series file should have discovered subtitles"
    );

    // 4. Build RenamePlan from scan results (video files only)
    let plan_entries: Vec<RenamePlanEntry> = results
        .iter()
        .map(|r| RenamePlanEntry {
            source_path: r.source_path.clone(),
            dest_path: r.proposed_path.clone(),
        })
        .collect();
    let plan = RenamePlan {
        entries: plan_entries,
    };

    // 5. Dry run
    let renamer = Renamer::from_config(&config.general);
    let dry_results = renamer.dry_run(&plan);
    assert!(
        dry_results.iter().all(|r| r.success),
        "Dry run should show no conflicts: {:?}",
        dry_results
            .iter()
            .filter(|r| !r.success)
            .map(|r| r.error.as_ref())
            .collect::<Vec<_>>()
    );

    // 6. Execute rename
    let exec_results = renamer.execute(&plan);
    assert!(
        exec_results.iter().all(|r| r.success),
        "All renames should succeed: {:?}",
        exec_results
            .iter()
            .filter(|r| !r.success)
            .map(|r| r.error.as_ref())
            .collect::<Vec<_>>()
    );

    // Verify dest files exist
    for result in &exec_results {
        assert!(
            result.dest_path.exists(),
            "Dest should exist: {:?}",
            result.dest_path
        );
    }
    // Verify source files moved (no longer at original location)
    assert!(!video1.exists(), "Source video1 should be moved");
    assert!(!video2.exists(), "Source video2 should be moved");

    // 7. Record in history
    let db_path = db_dir.path().join("history.db");
    let db = HistoryDb::open(&db_path).unwrap();
    let batch_id = HistoryDb::generate_batch_id();
    let timestamp = chrono::Utc::now().to_rfc3339();

    let records: Vec<RenameRecord> = exec_results
        .iter()
        .zip(results.iter())
        .map(|(exec, scan)| {
            let metadata = fs::metadata(&exec.dest_path).unwrap();
            RenameRecord {
                batch_id: batch_id.clone(),
                timestamp: timestamp.clone(),
                source_path: exec.source_path.clone(),
                dest_path: exec.dest_path.clone(),
                media_info: scan.media_info.clone(),
                file_size: metadata.len(),
                file_mtime: format!("{:?}", metadata.modified().unwrap()),
            }
        })
        .collect();
    db.record_batch(&records).unwrap();

    // 8. Verify history
    let batches = db.list_batches(None).unwrap();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].file_count, 2);

    // 9. Check undo eligible
    let eligibility = db.check_undo_eligible(&batch_id).unwrap();
    assert!(
        eligibility.eligible,
        "Batch should be eligible for undo: {:?}",
        eligibility.ineligible_reasons
    );

    // 10. Execute undo
    let undo_results = db.execute_undo(&batch_id).unwrap();
    assert!(
        undo_results.iter().all(|r| r.success),
        "All undos should succeed: {:?}",
        undo_results
            .iter()
            .filter(|r| !r.success)
            .map(|r| r.error.as_ref())
            .collect::<Vec<_>>()
    );

    // 11. Verify files restored to original locations
    assert!(video1.exists(), "Source video1 should be restored");
    assert!(video2.exists(), "Source video2 should be restored");

    // Verify content preserved
    assert_eq!(
        fs::read(&video1).unwrap(),
        b"video1 content here",
        "video1 content should be preserved"
    );
    assert_eq!(
        fs::read(&video2).unwrap(),
        b"video2 content here",
        "video2 content should be preserved"
    );

    // 12. Verify batch removed from history
    let batches_after = db.list_batches(None).unwrap();
    assert_eq!(
        batches_after.len(),
        0,
        "Batch should be removed after undo"
    );
}

#[test]
fn test_scan_detects_conflicts() {
    let source = TempDir::new().unwrap();
    let output = TempDir::new().unwrap();

    // Create two video files in different subdirectories that will produce
    // the same output path (identical filenames -> identical template output)
    let dir_a = source.path().join("dir_a");
    let dir_b = source.path().join("dir_b");
    fs::create_dir_all(&dir_a).unwrap();
    fs::create_dir_all(&dir_b).unwrap();

    fs::write(
        dir_a.join("Inception.2010.1080p.BluRay.x264-GROUP.mkv"),
        b"copy1",
    )
    .unwrap();
    fs::write(
        dir_b.join("Inception.2010.1080p.BluRay.x264-GROUP.mkv"),
        b"copy2",
    )
    .unwrap();

    let scanner = Scanner::new(config_with_output(output.path()));
    let results = scanner.scan_folder(source.path()).unwrap();

    // Both should be flagged as Conflict with "duplicate target path" reason
    let conflicts: Vec<_> = results
        .iter()
        .filter(|r| r.status == ScanStatus::Conflict)
        .collect();

    assert!(
        conflicts.len() >= 2,
        "Both duplicate entries should be marked Conflict, got {} conflicts out of {} results",
        conflicts.len(),
        results.len()
    );

    for conflict in &conflicts {
        assert!(
            conflict
                .ambiguity_reason
                .as_ref()
                .unwrap()
                .contains("duplicate target path"),
            "Conflict reason should mention 'duplicate target path', got: {:?}",
            conflict.ambiguity_reason
        );
    }
}

#[test]
fn test_scan_filter_by_media_type() {
    let source = TempDir::new().unwrap();
    let output = TempDir::new().unwrap();

    // Mix of movie and series files
    fs::write(
        source
            .path()
            .join("The.Office.S02E03.720p.BluRay.x264-DEMAND.mkv"),
        b"series",
    )
    .unwrap();
    fs::write(
        source
            .path()
            .join("Inception.2010.1080p.BluRay.x264-GROUP.mkv"),
        b"movie",
    )
    .unwrap();
    fs::write(
        source
            .path()
            .join("Breaking.Bad.S01E01.720p.mkv"),
        b"series2",
    )
    .unwrap();

    let scanner = Scanner::new(config_with_output(output.path()));
    let results = scanner.scan_folder(source.path()).unwrap();

    // Filter by Movie
    let movies = Scanner::filter_results(
        &results,
        &ScanFilter {
            media_type: Some(MediaType::Movie),
            ..ScanFilter::default()
        },
    );
    assert_eq!(movies.len(), 1, "Should find exactly 1 movie");
    assert_eq!(movies[0].media_info.media_type, MediaType::Movie);

    // Filter by Series
    let series = Scanner::filter_results(
        &results,
        &ScanFilter {
            media_type: Some(MediaType::Series),
            ..ScanFilter::default()
        },
    );
    assert_eq!(series.len(), 2, "Should find exactly 2 series");
    assert!(series
        .iter()
        .all(|r| r.media_info.media_type == MediaType::Series));

    // No filter: returns all
    let all = Scanner::filter_results(&results, &ScanFilter::default());
    assert_eq!(all.len(), results.len(), "No filter should return all results");
}
