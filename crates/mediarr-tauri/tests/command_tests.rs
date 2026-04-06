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
use mediarr_core::{RenamePlan, RenamePlanEntry, RenameRecord, Renamer};

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

    assert!(
        results.is_empty(),
        "Empty directory should produce no results"
    );
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
    assert!(
        source_file.exists(),
        "Source file should still exist after dry run"
    );
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
        settings: None,
    });

    // list_watchers command just returns config.watchers.clone()
    assert_eq!(config.watchers.len(), 1);
    assert_eq!(config.watchers[0].path, PathBuf::from("/tmp/test"));
    assert_eq!(config.watchers[0].mode, mediarr_core::WatcherMode::Auto);
    assert!(config.watchers[0].active);
}

// ---------------------------------------------------------------------------
// Watcher E2E tests (mirror the full Tauri command flow)
// ---------------------------------------------------------------------------

/// Simulate the start_watcher Tauri command flow:
/// spawn a watcher thread, wait for init signal, drop a file, verify processing.
#[test]
fn watcher_e2e_auto_mode_start_process_stop() {
    use std::sync::Arc;

    let watch_dir = TempDir::new().unwrap();
    let output_dir = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("e2e.db");

    let mut config = config_with_output(output_dir.path());
    config.watchers.push(mediarr_core::WatcherConfig {
        path: watch_dir.path().to_path_buf(),
        mode: mediarr_core::WatcherMode::Auto,
        active: false,
        debounce_seconds: 1,
        settings: None,
    });

    // -- Simulate start_watcher command --

    let watcher_config = config
        .watchers
        .iter()
        .find(|w| w.path == watch_dir.path())
        .cloned()
        .expect("watcher config should exist");

    let all_events = Arc::new(std::sync::Mutex::new(Vec::<(String, String)>::new()));
    let all_events_clone = all_events.clone();

    let on_event_callback: Box<dyn Fn(&mediarr_core::WatcherEvent) + Send> =
        Box::new(move |event: &mediarr_core::WatcherEvent| {
            let action = event.action.to_string();
            let detail = event.detail.clone().unwrap_or_default();
            eprintln!("[watcher-event] action={action} detail={detail}");
            all_events_clone.lock().unwrap().push((action, detail));
        });

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let (init_tx, init_rx) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);

    let config_clone = config.clone();
    let db_path_clone = db_path.clone();
    let watch_path = watcher_config.path.clone();
    let mode = watcher_config.mode;
    let debounce = watcher_config.debounce_seconds;

    // Spawn watcher thread (same pattern as Tauri start_watcher command)
    let watch_path_thread = watch_path.clone();
    let thread_handle = std::thread::Builder::new()
        .name("test-watcher-e2e".to_string())
        .spawn(move || {
            let db = mediarr_core::HistoryDb::open(&db_path_clone)
                .expect("open history db");
            let mut watcher = mediarr_core::WatcherManager::new(config_clone, db);
            watcher.set_on_event(on_event_callback);

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build tokio runtime");

            if let Err(e) = rt.block_on(watcher.run_with_init_signal(
                &watch_path_thread,
                mode,
                debounce,
                shutdown_rx,
                init_tx,
            )) {
                eprintln!("watcher exited with error: {e}");
            }
        })
        .expect("spawn watcher thread");

    // Wait for init signal (same as Tauri command)
    let init_result = init_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("should receive init signal");
    assert!(
        init_result.is_ok(),
        "watcher should initialize successfully: {:?}",
        init_result
    );

    // -- Simulate user dropping a file --
    let video = watch_path.join("Inception.2010.1080p.BluRay.x264-GROUP.mkv");
    fs::write(&video, b"video data").unwrap();

    // Wait for debounce (1s) + processing
    std::thread::sleep(std::time::Duration::from_secs(4));

    // -- Simulate stop_watcher command --
    let _ = shutdown_tx.send(true);
    thread_handle.join().expect("watcher thread should not panic");

    // -- Verify results --
    let events = all_events.lock().unwrap().clone();
    eprintln!("[test] all events: {events:?}");
    assert!(!events.is_empty(), "expected at least 1 watcher event");

    // At least one event should be 'renamed'
    let has_renamed = events.iter().any(|(a, _)| a == "renamed");
    assert!(
        has_renamed,
        "expected at least one 'renamed' event, got: {events:?}"
    );

    // Source file should be moved
    assert!(!video.exists(), "source file should have been moved");

    // Verify events in database (same as list_watcher_events command)
    let db = mediarr_core::HistoryDb::open(&db_path).expect("open history db");
    let events = db
        .list_watcher_events(Some(watch_dir.path()), None)
        .expect("list events");
    assert!(!events.is_empty(), "should have logged events to database");
    assert_eq!(
        events[0].action,
        mediarr_core::WatcherAction::Renamed,
        "first event should be 'renamed'"
    );

    // Verify history batch was recorded
    let batches = db.list_batches(None).expect("list batches");
    assert!(!batches.is_empty(), "should have recorded a history batch");
}

/// Simulate the review mode watcher flow + approve command.
#[test]
fn watcher_e2e_review_mode_queue_and_approve() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    let watch_dir = TempDir::new().unwrap();
    let output_dir = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("e2e-review.db");

    let mut config = config_with_output(output_dir.path());
    config.watchers.push(mediarr_core::WatcherConfig {
        path: watch_dir.path().to_path_buf(),
        mode: mediarr_core::WatcherMode::Review,
        active: false,
        debounce_seconds: 1,
        settings: None,
    });

    let event_count = Arc::new(AtomicUsize::new(0));
    let event_count_clone = event_count.clone();

    let on_event_callback: Box<dyn Fn(&mediarr_core::WatcherEvent) + Send> =
        Box::new(move |_event| {
            event_count_clone.fetch_add(1, Ordering::SeqCst);
        });

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let (init_tx, init_rx) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);

    let config_clone = config.clone();
    let db_path_clone = db_path.clone();
    let watch_path = watch_dir.path().to_path_buf();

    let watch_path_thread = watch_path.clone();
    let thread_handle = std::thread::Builder::new()
        .name("test-watcher-review".to_string())
        .spawn(move || {
            let db = mediarr_core::HistoryDb::open(&db_path_clone).expect("open db");
            let mut watcher = mediarr_core::WatcherManager::new(config_clone, db);
            watcher.set_on_event(on_event_callback);

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build runtime");

            let _ = rt.block_on(watcher.run_with_init_signal(
                &watch_path_thread,
                mediarr_core::WatcherMode::Review,
                1,
                shutdown_rx,
                init_tx,
            ));
        })
        .expect("spawn thread");

    let init_result = init_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("should receive init signal");
    assert!(init_result.is_ok(), "init should succeed: {:?}", init_result);

    // Drop a video file
    let video = watch_path.join("The.Office.S02E03.720p.BluRay.x264-DEMAND.mkv");
    fs::write(&video, b"series data").unwrap();

    // Wait for debounce + processing
    std::thread::sleep(std::time::Duration::from_secs(4));

    // Stop watcher
    let _ = shutdown_tx.send(true);
    thread_handle.join().expect("thread should not panic");

    // -- Verify review queue (mirrors list_review_queue command) --
    let db = mediarr_core::HistoryDb::open(&db_path).expect("open db");
    let queue = db
        .list_review_queue(Some(watch_dir.path()), Some(mediarr_core::ReviewStatus::Pending))
        .expect("list review queue");
    assert!(!queue.is_empty(), "review queue should have at least 1 entry");

    let entry = &queue[0];
    assert!(entry.id.is_some(), "entry should have an id");
    assert!(
        entry.source_path.exists(),
        "source file should still exist (review mode doesn't rename)"
    );

    // -- Simulate approve_review_entry command --
    let entry_id = entry.id.unwrap();

    // Parse subtitle entries from JSON (same as approve command)
    let mut plan_entries = vec![mediarr_core::RenamePlanEntry {
        source_path: entry.source_path.clone(),
        dest_path: entry.proposed_path.clone(),
    }];

    if let Ok(subtitles) =
        serde_json::from_str::<Vec<mediarr_core::SubtitleMatch>>(&entry.subtitles_json)
    {
        for sub in &subtitles {
            plan_entries.push(mediarr_core::RenamePlanEntry {
                source_path: sub.source_path.clone(),
                dest_path: sub.proposed_path.clone(),
            });
        }
    }

    let plan = mediarr_core::RenamePlan {
        entries: plan_entries,
    };

    let renamer = mediarr_core::Renamer::from_config(&config.general);
    let results = renamer.execute(&plan);
    assert!(
        results.iter().all(|r| r.success),
        "all renames should succeed: {:?}",
        results
    );

    // Record history batch
    let batch_id = mediarr_core::HistoryDb::generate_batch_id();
    let timestamp = chrono::Utc::now().to_rfc3339();
    let media_info: mediarr_core::MediaInfo =
        serde_json::from_str(&entry.media_info_json)
            .expect("media_info_json should deserialize");

    let records: Vec<mediarr_core::RenameRecord> = results
        .iter()
        .filter(|r| r.success)
        .map(|r| {
            let file_size = fs::metadata(&r.dest_path).map(|m| m.len()).unwrap_or(0);
            mediarr_core::RenameRecord {
                batch_id: batch_id.clone(),
                timestamp: timestamp.clone(),
                source_path: r.source_path.clone(),
                dest_path: r.dest_path.clone(),
                media_info: media_info.clone(),
                file_size,
                file_mtime: String::new(),
            }
        })
        .collect();

    db.record_batch(&records).expect("record batch");
    db.update_review_status(entry_id, mediarr_core::ReviewStatus::Approved)
        .expect("update review status");

    // Verify: source gone, dest exists, status updated
    assert!(!video.exists(), "source should be moved after approve");

    let updated_queue = db
        .list_review_queue(Some(watch_dir.path()), Some(mediarr_core::ReviewStatus::Pending))
        .expect("list queue");
    assert!(
        updated_queue.is_empty(),
        "pending queue should be empty after approve"
    );

    let batches = db.list_batches(None).expect("list batches");
    assert!(!batches.is_empty(), "history should have a batch");
}

/// Test watcher events are correctly serialized for Tauri IPC.
/// Verifies the serde round-trip that happens when events go through
/// app.emit() -> frontend listen().
#[test]
fn watcher_event_serializes_for_tauri_ipc() {
    let event = mediarr_core::WatcherEvent {
        id: Some(42),
        timestamp: "2026-04-06T12:00:00Z".to_string(),
        watch_path: PathBuf::from("/Users/test/media"),
        filename: "Inception.2010.mkv".to_string(),
        action: mediarr_core::WatcherAction::Renamed,
        detail: Some("/Users/test/output/Inception (2010)/Inception.mkv".to_string()),
        batch_id: Some("batch-123".to_string()),
    };

    // Serialize (what Tauri app.emit does)
    let json = serde_json::to_string(&event).expect("should serialize");

    // Verify JSON structure matches TypeScript interface expectations
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    assert_eq!(parsed["id"], 42);
    assert_eq!(parsed["timestamp"], "2026-04-06T12:00:00Z");
    assert_eq!(parsed["watch_path"], "/Users/test/media");
    assert_eq!(parsed["filename"], "Inception.2010.mkv");
    assert_eq!(parsed["action"], "renamed");
    assert_eq!(
        parsed["detail"],
        "/Users/test/output/Inception (2010)/Inception.mkv"
    );
    assert_eq!(parsed["batch_id"], "batch-123");

    // Deserialize back (proves round-trip)
    let restored: mediarr_core::WatcherEvent =
        serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(restored.filename, "Inception.2010.mkv");
    assert_eq!(restored.action, mediarr_core::WatcherAction::Renamed);
}

/// Test ReviewQueueEntry serializes correctly for frontend IPC.
#[test]
fn review_queue_entry_serializes_for_tauri_ipc() {
    let entry = mediarr_core::ReviewQueueEntry {
        id: Some(1),
        timestamp: "2026-04-06T12:00:00Z".to_string(),
        watch_path: PathBuf::from("/Users/test/media"),
        source_path: PathBuf::from("/Users/test/media/Movie.2024.mkv"),
        proposed_path: PathBuf::from("/Users/test/output/Movie (2024)/Movie.mkv"),
        media_info_json: r#"{"title":"Movie","media_type":"Movie"}"#.to_string(),
        subtitles_json: "[]".to_string(),
        status: mediarr_core::ReviewStatus::Pending,
    };

    let json = serde_json::to_string(&entry).expect("should serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");

    // Verify field names match TypeScript interface (snake_case, not camelCase)
    assert!(parsed.get("id").is_some());
    assert!(parsed.get("timestamp").is_some());
    assert!(parsed.get("watch_path").is_some());
    assert!(parsed.get("source_path").is_some());
    assert!(parsed.get("proposed_path").is_some());
    assert!(parsed.get("media_info_json").is_some());
    assert!(parsed.get("subtitles_json").is_some());
    assert!(parsed.get("status").is_some());
    assert_eq!(parsed["status"], "pending");
}

/// Test WatcherConfig serializes correctly for frontend IPC.
#[test]
fn watcher_config_serializes_for_tauri_ipc() {
    let wc = mediarr_core::WatcherConfig {
        path: PathBuf::from("/Users/test/downloads"),
        mode: mediarr_core::WatcherMode::Review,
        active: true,
        debounce_seconds: 10,
        settings: None,
    };

    let json = serde_json::to_string(&wc).expect("should serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");

    // Frontend expects these exact field names
    assert_eq!(parsed["path"], "/Users/test/downloads");
    assert_eq!(parsed["mode"], "review");
    assert_eq!(parsed["active"], true);
    assert_eq!(parsed["debounce_seconds"], 10);
}
