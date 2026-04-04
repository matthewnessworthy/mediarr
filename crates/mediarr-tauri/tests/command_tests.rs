//! Integration tests for Tauri command handlers.
//!
//! Tests exercise the same logic paths as the Tauri commands by constructing
//! AppState directly and calling mediarr-core APIs the same way command
//! handlers do. Commands that don't require State (preview_template,
//! validate_template) are called directly.

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use mediarr_core::{
    Config, HistoryDb, MediaInfo, MediaType, ParseConfidence, Scanner, TemplateEngine,
};
use mediarr_core::{RenamePlan, RenamePlanEntry, Renamer, RenameRecord};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a default Config with output_dir pointing to the given path.
fn config_with_output(output_dir: &Path) -> Config {
    let mut config = Config::default();
    config.general.output_dir = Some(output_dir.to_path_buf());
    config
}

/// Create a test MediaInfo for a movie.
fn test_media_info() -> MediaInfo {
    MediaInfo {
        title: "Test Movie".to_string(),
        media_type: MediaType::Movie,
        year: Some(2024),
        season: None,
        episodes: vec![],
        resolution: Some("1080p".to_string()),
        video_codec: Some("x264".to_string()),
        audio_codec: None,
        source: Some("BluRay".to_string()),
        release_group: Some("GROUP".to_string()),
        container: "mkv".to_string(),
        language: None,
        confidence: ParseConfidence::High,
    }
}

/// Create a test MediaInfo for a series episode.
fn test_series_info() -> MediaInfo {
    MediaInfo {
        title: "The Office".to_string(),
        media_type: MediaType::Series,
        year: None,
        season: Some(2),
        episodes: vec![3],
        resolution: Some("720p".to_string()),
        video_codec: Some("x264".to_string()),
        audio_codec: None,
        source: Some("BluRay".to_string()),
        release_group: Some("DEMAND".to_string()),
        container: "mkv".to_string(),
        language: None,
        confidence: ParseConfidence::High,
    }
}

// ---------------------------------------------------------------------------
// Scan tests (mirrors scan_folder command)
// ---------------------------------------------------------------------------

#[test]
fn scan_folder_returns_results() {
    let source_dir = TempDir::new().unwrap();
    let output_dir = TempDir::new().unwrap();

    // Create a recognisable media file
    fs::write(
        source_dir
            .path()
            .join("The.Office.S02E03.720p.BluRay.x264-DEMAND.mkv"),
        b"fake video content",
    )
    .unwrap();

    let config = config_with_output(output_dir.path());
    let scanner = Scanner::new(config);
    let results = scanner.scan_folder(source_dir.path()).unwrap();

    assert_eq!(results.len(), 1, "Should find exactly 1 video file");
    assert!(
        results[0].media_info.title.contains("Office"),
        "Title should contain 'Office', got: {}",
        results[0].media_info.title
    );
    assert_eq!(
        results[0].media_info.media_type,
        MediaType::Series,
        "Should detect as Series"
    );
}

#[test]
fn scan_folder_empty_dir_returns_empty() {
    let source_dir = TempDir::new().unwrap();
    let output_dir = TempDir::new().unwrap();

    let config = config_with_output(output_dir.path());
    let scanner = Scanner::new(config);
    let results = scanner.scan_folder(source_dir.path()).unwrap();

    assert!(results.is_empty(), "Empty directory should produce no results");
}

// ---------------------------------------------------------------------------
// Dry-run rename tests (mirrors dry_run_renames command)
// ---------------------------------------------------------------------------

#[test]
fn dry_run_renames_validates_without_touching_fs() {
    let source_dir = TempDir::new().unwrap();
    let output_dir = TempDir::new().unwrap();

    let source_file = source_dir.path().join("test_video.mkv");
    fs::write(&source_file, b"video bytes").unwrap();

    let dest_file = output_dir.path().join("Renamed Movie.mkv");

    let config = config_with_output(output_dir.path());
    let renamer = Renamer::from_config(&config.general);
    let plan = RenamePlan {
        entries: vec![RenamePlanEntry {
            source_path: source_file.clone(),
            dest_path: dest_file.clone(),
        }],
    };

    let results = renamer.dry_run(&plan);

    assert_eq!(results.len(), 1);
    assert!(results[0].success, "Dry run should succeed");
    assert!(source_file.exists(), "Source file should still exist after dry run");
    assert!(
        !dest_file.exists(),
        "Dest file should NOT exist after dry run"
    );
}

// ---------------------------------------------------------------------------
// Execute rename tests (mirrors execute_renames command)
// ---------------------------------------------------------------------------

#[test]
fn execute_renames_moves_files() {
    let source_dir = TempDir::new().unwrap();
    let output_dir = TempDir::new().unwrap();

    let source_file = source_dir.path().join("movie.mkv");
    let content = b"known video content for verification";
    fs::write(&source_file, content).unwrap();

    let dest_file = output_dir.path().join("Movie Renamed.mkv");

    let config = config_with_output(output_dir.path());
    let renamer = Renamer::from_config(&config.general);
    let plan = RenamePlan {
        entries: vec![RenamePlanEntry {
            source_path: source_file.clone(),
            dest_path: dest_file.clone(),
        }],
    };

    let results = renamer.execute(&plan);

    assert_eq!(results.len(), 1);
    assert!(results[0].success, "Rename should succeed");
    assert!(
        !source_file.exists(),
        "Source file should no longer exist at original path"
    );
    assert!(dest_file.exists(), "Dest file should exist");
    assert_eq!(
        fs::read(&dest_file).unwrap(),
        content,
        "Dest file should contain the original content"
    );
}

#[test]
fn execute_renames_records_history() {
    let source_dir = TempDir::new().unwrap();
    let output_dir = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();

    let source_file = source_dir.path().join("movie.mkv");
    fs::write(&source_file, b"content").unwrap();

    let dest_file = output_dir.path().join("Movie.mkv");

    let config = config_with_output(output_dir.path());
    let renamer = Renamer::from_config(&config.general);
    let plan = RenamePlan {
        entries: vec![RenamePlanEntry {
            source_path: source_file.clone(),
            dest_path: dest_file.clone(),
        }],
    };

    let results = renamer.execute(&plan);
    assert!(results[0].success);

    // Record in history (same pattern as the Tauri command handler)
    let db_path = db_dir.path().join("history.db");
    let db = HistoryDb::open(&db_path).unwrap();
    let batch_id = HistoryDb::generate_batch_id();
    let timestamp = chrono::Utc::now().to_rfc3339();

    let metadata = fs::metadata(&dest_file).unwrap();
    let records = vec![RenameRecord {
        batch_id: batch_id.clone(),
        timestamp,
        source_path: source_file,
        dest_path: dest_file,
        media_info: test_media_info(),
        file_size: metadata.len(),
        file_mtime: String::new(),
    }];

    db.record_batch(&records).unwrap();

    let batches = db.list_batches(None).unwrap();
    assert_eq!(batches.len(), 1, "Should have 1 batch");
    assert_eq!(batches[0].file_count, 1, "Batch should have 1 file");
}

// ---------------------------------------------------------------------------
// History tests (mirrors list_batches, check_undo, execute_undo commands)
// ---------------------------------------------------------------------------

#[test]
fn list_batches_returns_empty_initially() {
    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("history.db");
    let db = HistoryDb::open(&db_path).unwrap();

    let batches = db.list_batches(None).unwrap();
    assert!(batches.is_empty(), "Fresh database should have no batches");
}

#[test]
fn check_undo_nonexistent_batch() {
    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("history.db");
    let db = HistoryDb::open(&db_path).unwrap();

    let result = db.check_undo_eligible("nonexistent-batch-id");

    // API returns Ok with eligible=true and empty issues for nonexistent batch
    // (vacuously true -- no entries means no ineligible entries). This is expected
    // because get_batch returns an empty vec for unknown IDs.
    match result {
        Ok(eligibility) => {
            assert_eq!(
                eligibility.batch_id, "nonexistent-batch-id",
                "Should echo back the batch ID"
            );
            assert!(
                eligibility.ineligible_reasons.is_empty(),
                "No entries means no ineligible reasons"
            );
        }
        Err(_) => {
            // Returning an error for nonexistent batch is also acceptable
        }
    }
}

#[test]
fn execute_undo_after_rename() {
    let source_dir = TempDir::new().unwrap();
    let output_dir = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();

    let source_file = source_dir.path().join("original.mkv");
    let content = b"undo test content";
    fs::write(&source_file, content).unwrap();

    let dest_file = output_dir.path().join("Renamed.mkv");

    // Execute rename
    let config = config_with_output(output_dir.path());
    let renamer = Renamer::from_config(&config.general);
    let plan = RenamePlan {
        entries: vec![RenamePlanEntry {
            source_path: source_file.clone(),
            dest_path: dest_file.clone(),
        }],
    };
    let results = renamer.execute(&plan);
    assert!(results[0].success);
    assert!(dest_file.exists());
    assert!(!source_file.exists());

    // Record in history
    let db_path = db_dir.path().join("history.db");
    let db = HistoryDb::open(&db_path).unwrap();
    let batch_id = HistoryDb::generate_batch_id();
    let timestamp = chrono::Utc::now().to_rfc3339();
    let metadata = fs::metadata(&dest_file).unwrap();

    let records = vec![RenameRecord {
        batch_id: batch_id.clone(),
        timestamp,
        source_path: source_file.clone(),
        dest_path: dest_file.clone(),
        media_info: test_media_info(),
        file_size: metadata.len(),
        file_mtime: String::new(),
    }];
    db.record_batch(&records).unwrap();

    // Execute undo
    let undo_results = db.execute_undo(&batch_id).unwrap();
    assert!(
        undo_results.iter().all(|r| r.success),
        "All undos should succeed"
    );

    // Verify file moved back
    assert!(
        source_file.exists(),
        "Source file should be restored after undo"
    );
    assert_eq!(
        fs::read(&source_file).unwrap(),
        content,
        "Restored file should have original content"
    );
}

// ---------------------------------------------------------------------------
// Config tests (mirrors get_config, update_config commands)
// ---------------------------------------------------------------------------

#[test]
fn get_config_returns_default() {
    let config = Config::default();

    assert_eq!(
        config.general.operation,
        mediarr_core::RenameOperation::Move,
        "Default operation should be Move"
    );
    assert!(
        !config.templates.movie.is_empty(),
        "Default movie template should be populated"
    );
    assert!(
        !config.templates.series.is_empty(),
        "Default series template should be populated"
    );
}

#[test]
fn update_config_persists() {
    let config_dir = TempDir::new().unwrap();
    let config_path = config_dir.path().join("config.toml");

    let mut config = Config::default();
    config.general.create_directories = false;

    // Save (same pattern as update_config command)
    config.save(&config_path).unwrap();

    // Load back
    let loaded = Config::load(&config_path).unwrap();
    assert!(
        !loaded.general.create_directories,
        "Loaded config should reflect the saved change"
    );
}

// ---------------------------------------------------------------------------
// Template tests (mirrors preview_template, validate_template commands)
// ---------------------------------------------------------------------------

#[test]
fn preview_template_renders_correctly() {
    let engine = TemplateEngine::new();
    let info = test_media_info();

    // Same call as preview_template command
    let result = engine.render("{title} ({year}).{ext}", &info).unwrap();
    let rendered = result.to_string_lossy();

    assert!(
        rendered.contains("Test Movie"),
        "Rendered should contain title, got: {}",
        rendered
    );
    assert!(
        rendered.contains("2024"),
        "Rendered should contain year, got: {}",
        rendered
    );
    assert!(
        rendered.contains("mkv"),
        "Rendered should contain extension, got: {}",
        rendered
    );
}

#[test]
fn preview_template_renders_series() {
    let engine = TemplateEngine::new();
    let info = test_series_info();

    let result = engine
        .render("{title} - S{season:02}E{episode:02}.{ext}", &info)
        .unwrap();
    let rendered = result.to_string_lossy();

    assert!(
        rendered.contains("The Office"),
        "Should contain series title"
    );
    assert!(rendered.contains("S02E03"), "Should contain season/episode");
    assert!(rendered.contains("mkv"), "Should contain extension");
}

#[test]
fn validate_template_returns_warnings_for_missing_required() {
    let engine = TemplateEngine::new();

    // Template missing required variables (year, ext) for Movie type
    let warnings = engine.validate("{title}", &MediaType::Movie);

    assert!(
        !warnings.is_empty(),
        "Template missing required variables should produce warnings"
    );
    assert!(
        warnings.iter().any(|w| w.variable == "year"),
        "Should warn about missing year variable, got: {:?}",
        warnings
    );
    assert!(
        warnings.iter().any(|w| w.variable == "ext"),
        "Should warn about missing ext variable, got: {:?}",
        warnings
    );
}

#[test]
fn validate_template_valid_returns_no_warnings() {
    let engine = TemplateEngine::new();

    let warnings = engine.validate("{title} ({year}).{ext}", &MediaType::Movie);

    assert!(
        warnings.is_empty(),
        "Valid template should produce no warnings, got: {:?}",
        warnings
    );
}

// ---------------------------------------------------------------------------
// Watcher config tests (mirrors list_watchers command)
// ---------------------------------------------------------------------------

#[test]
fn list_watchers_returns_configured() {
    let mut config = Config::default();
    config.watchers.push(mediarr_core::WatcherConfig {
        path: PathBuf::from("/tmp/test"),
        mode: mediarr_core::WatcherMode::Auto,
        active: true,
        debounce_seconds: 5,
    });

    // list_watchers command just returns config.watchers.clone()
    assert_eq!(config.watchers.len(), 1);
    assert_eq!(config.watchers[0].path, PathBuf::from("/tmp/test"));
    assert_eq!(config.watchers[0].mode, mediarr_core::WatcherMode::Auto);
    assert!(config.watchers[0].active);
}
