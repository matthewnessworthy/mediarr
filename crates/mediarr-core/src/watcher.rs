//! Filesystem watcher for automatic media file processing.
//!
//! Monitors folders for new video files using notify-debouncer-full,
//! then either auto-renames or queues for review based on configuration.
//! Uses a channel bridge pattern to connect notify's sync callbacks to
//! tokio's async runtime.

use std::path::{Path, PathBuf};
use std::time::Duration;

use notify::RecursiveMode;
use notify_debouncer_full::new_debouncer;
use tracing::{debug, error, info, warn};

use crate::config::Config;
use crate::error::{MediError, Result};
use crate::history::HistoryDb;
use crate::renamer::{RenamePlan, RenamePlanEntry, Renamer};
use crate::scanner::Scanner;
use crate::types::{
    RenameRecord, ReviewQueueEntry, ReviewStatus, WatcherAction, WatcherEvent, WatcherMode,
};

/// Default maximum activity events kept per watch path (per D-07).
const DEFAULT_MAX_EVENTS: usize = 500;

/// Video file extensions recognised by the watcher.
const VIDEO_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "avi", "m4v", "mov", "wmv", "ts", "flv", "webm",
];

/// Check whether a path has a video file extension.
pub(crate) fn is_video_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| VIDEO_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Manages filesystem watching for a single folder.
///
/// Bridges notify's synchronous callbacks to tokio's async runtime using
/// a channel bridge pattern. Processes debounced filesystem events by
/// either auto-renaming or queuing for review.
pub struct WatcherManager {
    config: Config,
    scanner: Scanner,
    renamer: Renamer,
    history: HistoryDb,
    max_activity_events: usize,
}

impl WatcherManager {
    /// Create a new WatcherManager from the application config and history database.
    pub fn new(config: Config, history: HistoryDb) -> Self {
        let scanner = Scanner::new(config.clone());
        let renamer = Renamer::from_config(&config.general);
        Self {
            config,
            scanner,
            renamer,
            history,
            max_activity_events: DEFAULT_MAX_EVENTS,
        }
    }

    /// Run the watcher event loop for a given path and mode.
    ///
    /// Uses a channel bridge to connect notify-debouncer-full's sync
    /// callbacks to tokio's async select loop. Runs until the shutdown
    /// signal is received.
    ///
    /// # Arguments
    ///
    /// * `watch_path` - Directory to watch recursively
    /// * `mode` - Auto (rename immediately) or Review (queue for user)
    /// * `debounce_seconds` - Debounce duration for filesystem events
    /// * `shutdown` - Watch receiver; when true, the loop exits
    pub async fn run(
        &self,
        watch_path: &Path,
        mode: WatcherMode,
        debounce_seconds: u64,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> Result<()> {
        info!(path = %watch_path.display(), ?mode, debounce_seconds, "starting watcher");

        // Sync channel for notify callback -> bridge thread
        let (sync_tx, sync_rx) = std::sync::mpsc::channel();

        // Async channel for bridge thread -> tokio event loop
        let (async_tx, mut async_rx) =
            tokio::sync::mpsc::unbounded_channel::<Vec<notify_debouncer_full::DebouncedEvent>>();

        // Create debouncer with sync callback
        let mut debouncer = new_debouncer(
            Duration::from_secs(debounce_seconds),
            None,
            move |result: std::result::Result<Vec<notify_debouncer_full::DebouncedEvent>, Vec<notify::Error>>| {
                match result {
                    Ok(events) => {
                        if let Err(e) = sync_tx.send(events) {
                            error!("failed to send debounced events to bridge: {e}");
                        }
                    }
                    Err(errors) => {
                        for e in errors {
                            warn!("notify error: {e}");
                        }
                    }
                }
            },
        )
        .map_err(|e| MediError::Watcher(format!("failed to create debouncer: {e}")))?;

        // Start watching the path
        debouncer
            .watch(watch_path, RecursiveMode::Recursive)
            .map_err(|e| MediError::Watcher(format!("failed to watch path: {e}")))?;

        // Bridge thread: forward from sync channel to async channel
        let bridge_async_tx = async_tx.clone();
        std::thread::spawn(move || {
            while let Ok(events) = sync_rx.recv() {
                if bridge_async_tx.send(events).is_err() {
                    break; // async receiver dropped, stop bridge
                }
            }
        });

        // Async event loop
        let watch_path_owned = watch_path.to_path_buf();
        loop {
            tokio::select! {
                Some(events) = async_rx.recv() => {
                    self.process_debounced_events(&events, &watch_path_owned, mode);
                }
                Ok(()) = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!(path = %watch_path_owned.display(), "shutdown signal received, stopping watcher");
                        break;
                    }
                }
            }
        }

        // Debouncer drops here, stopping the watch automatically
        info!(path = %watch_path_owned.display(), "watcher stopped");
        Ok(())
    }

    /// Process a batch of debounced filesystem events.
    ///
    /// Filters for create/rename-to events on video files, then delegates
    /// to [`process_single_file`] for each.
    fn process_debounced_events(
        &self,
        events: &[notify_debouncer_full::DebouncedEvent],
        watch_path: &Path,
        mode: WatcherMode,
    ) {
        for event in events {
            // Only process file creation and rename-to events
            let dominated = matches!(
                event.kind,
                notify::EventKind::Create(_)
                    | notify::EventKind::Modify(notify::event::ModifyKind::Name(
                        notify::event::RenameMode::To
                    ))
            );
            if !dominated {
                continue;
            }

            for path in &event.paths {
                if !is_video_file(path) {
                    debug!(path = %path.display(), "skipping non-video file");
                    continue;
                }

                if let Err(e) = self.process_single_file(path, watch_path, mode) {
                    warn!(
                        path = %path.display(),
                        error = %e,
                        "error processing watcher event"
                    );
                }
            }
        }
    }

    /// Process a single video file event.
    ///
    /// In auto mode: scan, rename, record batch, log event.
    /// In review mode: scan, queue for review, log event.
    /// Errors are logged as watcher events rather than propagated (the
    /// watcher loop must not crash on individual file failures).
    fn process_single_file(
        &self,
        path: &Path,
        watch_path: &Path,
        mode: WatcherMode,
    ) -> Result<()> {
        let timestamp = chrono::Utc::now().to_rfc3339();
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        info!(filename = %filename, ?mode, "processing watcher event");

        // Scan the file
        let scan_result = match self.scanner.scan_file(path) {
            Ok(r) => r,
            Err(e) => {
                // Log error event but don't crash the watcher
                let event = WatcherEvent {
                    id: None,
                    timestamp,
                    watch_path: watch_path.to_path_buf(),
                    filename,
                    action: WatcherAction::Error,
                    detail: Some(format!("scan failed: {e}")),
                    batch_id: None,
                };
                self.history.log_watcher_event(&event)?;
                self.history.prune_watcher_events(watch_path, self.max_activity_events)?;
                return Ok(());
            }
        };

        match mode {
            WatcherMode::Auto => {
                // Build rename plan: video + any discovered subtitles
                let mut entries = vec![RenamePlanEntry {
                    source_path: scan_result.source_path.clone(),
                    dest_path: scan_result.proposed_path.clone(),
                }];

                // Add subtitle rename entries
                for sub in &scan_result.subtitles {
                    entries.push(RenamePlanEntry {
                        source_path: sub.source_path.clone(),
                        dest_path: sub.proposed_path.clone(),
                    });
                }

                let plan = RenamePlan { entries };
                let results = self.renamer.execute(&plan);

                // Check if all results are successful
                let all_success = results.iter().all(|r| r.success);

                if all_success {
                    // Record batch in history
                    let batch_id = HistoryDb::generate_batch_id();

                    let records: Vec<RenameRecord> = results
                        .iter()
                        .filter(|r| r.success)
                        .map(|r| {
                            let file_size = std::fs::metadata(&r.dest_path)
                                .map(|m| m.len())
                                .unwrap_or(0);
                            let file_mtime = std::fs::metadata(&r.dest_path)
                                .and_then(|m| m.modified())
                                .ok()
                                .and_then(|t| {
                                    let dt: chrono::DateTime<chrono::Utc> = t.into();
                                    Some(dt.to_rfc3339())
                                })
                                .unwrap_or_default();

                            RenameRecord {
                                batch_id: batch_id.clone(),
                                timestamp: timestamp.clone(),
                                source_path: r.source_path.clone(),
                                dest_path: r.dest_path.clone(),
                                media_info: scan_result.media_info.clone(),
                                file_size,
                                file_mtime,
                            }
                        })
                        .collect();

                    if let Err(e) = self.history.record_batch(&records) {
                        warn!(error = %e, "failed to record rename batch in history");
                    }

                    // Log watcher event
                    let event = WatcherEvent {
                        id: None,
                        timestamp: timestamp.clone(),
                        watch_path: watch_path.to_path_buf(),
                        filename,
                        action: WatcherAction::Renamed,
                        detail: Some(format!("{}", scan_result.proposed_path.display())),
                        batch_id: Some(batch_id),
                    };
                    self.history.log_watcher_event(&event)?;
                } else {
                    // Some renames failed
                    let errors: Vec<String> = results
                        .iter()
                        .filter(|r| !r.success)
                        .filter_map(|r| r.error.clone())
                        .collect();
                    let detail = errors.join("; ");

                    let event = WatcherEvent {
                        id: None,
                        timestamp: timestamp.clone(),
                        watch_path: watch_path.to_path_buf(),
                        filename,
                        action: WatcherAction::Error,
                        detail: Some(format!("rename failed: {detail}")),
                        batch_id: None,
                    };
                    self.history.log_watcher_event(&event)?;
                }

                // Prune old events
                self.history.prune_watcher_events(watch_path, self.max_activity_events)?;
            }

            WatcherMode::Review => {
                // Serialize media info and subtitles to JSON
                let media_info_json = serde_json::to_string(&scan_result.media_info)
                    .unwrap_or_else(|_| "{}".to_string());
                let subtitles_json = serde_json::to_string(&scan_result.subtitles)
                    .unwrap_or_else(|_| "[]".to_string());

                // Create review queue entry
                let entry = ReviewQueueEntry {
                    id: None,
                    timestamp: timestamp.clone(),
                    watch_path: watch_path.to_path_buf(),
                    source_path: scan_result.source_path.clone(),
                    proposed_path: scan_result.proposed_path.clone(),
                    media_info_json,
                    subtitles_json,
                    status: ReviewStatus::Pending,
                };

                self.history.add_to_review_queue(&entry)?;

                // Log watcher event
                let event = WatcherEvent {
                    id: None,
                    timestamp: timestamp.clone(),
                    watch_path: watch_path.to_path_buf(),
                    filename,
                    action: WatcherAction::Queued,
                    detail: Some(format!("{}", scan_result.proposed_path.display())),
                    batch_id: None,
                };
                self.history.log_watcher_event(&event)?;

                // Prune old events
                self.history.prune_watcher_events(watch_path, self.max_activity_events)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::history::HistoryDb;
    use crate::types::{ReviewStatus, WatcherAction};
    use std::fs;
    use tempfile::TempDir;

    /// Helper: create a Config with output_dir set.
    fn test_config(output_dir: &Path) -> Config {
        let mut config = Config::default();
        config.general.output_dir = Some(output_dir.to_path_buf());
        config
    }

    /// Helper: create a WatcherManager with temp dirs and in-memory history.
    fn setup_watcher(output_dir: &Path, db_path: &Path) -> WatcherManager {
        let config = test_config(output_dir);
        let history = HistoryDb::open(db_path).expect("open history db");
        WatcherManager::new(config, history)
    }

    // -----------------------------------------------------------------------
    // Test 1: WatcherManager::new creates an instance
    // -----------------------------------------------------------------------

    #[test]
    fn new_creates_instance_from_config_and_history() {
        let tmp = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");

        let watcher = setup_watcher(output.path(), &db_path);
        assert_eq!(watcher.max_activity_events, DEFAULT_MAX_EVENTS);
    }

    // -----------------------------------------------------------------------
    // Test 2: process_single_file in auto mode renames file
    // -----------------------------------------------------------------------

    #[test]
    fn auto_mode_renames_valid_video_file() {
        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();
        let db_dir = TempDir::new().unwrap();
        let db_path = db_dir.path().join("test.db");

        let video = source.path().join("Inception.2010.1080p.BluRay.x264-GROUP.mkv");
        fs::write(&video, b"video data").unwrap();

        let watcher = setup_watcher(output.path(), &db_path);
        let result = watcher.process_single_file(&video, source.path(), WatcherMode::Auto);
        assert!(result.is_ok(), "process_single_file should succeed: {:?}", result);

        // Verify the file was moved (source gone, something exists in output)
        assert!(!video.exists(), "source file should have been moved");

        // Verify a watcher event was logged
        let events = watcher
            .history
            .list_watcher_events(Some(source.path()), None)
            .unwrap();
        assert!(!events.is_empty(), "should have logged at least one event");
        assert_eq!(events[0].action, WatcherAction::Renamed);
    }

    // -----------------------------------------------------------------------
    // Test 3: process_single_file in review mode queues without renaming
    // -----------------------------------------------------------------------

    #[test]
    fn review_mode_queues_without_renaming() {
        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();
        let db_dir = TempDir::new().unwrap();
        let db_path = db_dir.path().join("test.db");

        let video = source.path().join("Inception.2010.1080p.BluRay.x264-GROUP.mkv");
        fs::write(&video, b"video data").unwrap();

        let watcher = setup_watcher(output.path(), &db_path);
        let result = watcher.process_single_file(&video, source.path(), WatcherMode::Review);
        assert!(result.is_ok(), "process_single_file should succeed: {:?}", result);

        // Source file should still exist (not renamed)
        assert!(video.exists(), "source file should NOT be moved in review mode");

        // Verify entry added to review queue
        let queue = watcher
            .history
            .list_review_queue(Some(source.path()), Some(ReviewStatus::Pending))
            .unwrap();
        assert!(!queue.is_empty(), "review queue should have an entry");

        // Verify watcher event logged
        let events = watcher
            .history
            .list_watcher_events(Some(source.path()), None)
            .unwrap();
        assert!(!events.is_empty(), "should have logged a watcher event");
        assert_eq!(events[0].action, WatcherAction::Queued);
    }

    // -----------------------------------------------------------------------
    // Test 4: non-video file is ignored
    // -----------------------------------------------------------------------

    #[test]
    fn non_video_file_is_ignored_by_process_single_file() {
        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();
        let db_dir = TempDir::new().unwrap();
        let db_path = db_dir.path().join("test.db");

        let txt_file = source.path().join("readme.txt");
        fs::write(&txt_file, b"text content").unwrap();

        let watcher = setup_watcher(output.path(), &db_path);
        // scan_file will reject non-video files, so this logs an error event
        let result = watcher.process_single_file(&txt_file, source.path(), WatcherMode::Auto);
        // Should not crash
        assert!(result.is_ok());

        // The txt file should still exist (not moved)
        assert!(txt_file.exists());
    }

    // -----------------------------------------------------------------------
    // Test 5: process_single_file logs WatcherEvent for both modes
    // -----------------------------------------------------------------------

    #[test]
    fn logs_watcher_event_for_auto_mode() {
        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();
        let db_dir = TempDir::new().unwrap();
        let db_path = db_dir.path().join("test.db");

        let video = source.path().join("The.Office.S02E03.720p.BluRay.x264-DEMAND.mkv");
        fs::write(&video, b"series data").unwrap();

        let watcher = setup_watcher(output.path(), &db_path);
        watcher
            .process_single_file(&video, source.path(), WatcherMode::Auto)
            .unwrap();

        let events = watcher
            .history
            .list_watcher_events(Some(source.path()), None)
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, WatcherAction::Renamed);
        assert!(events[0].detail.is_some());
    }

    #[test]
    fn logs_watcher_event_for_review_mode() {
        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();
        let db_dir = TempDir::new().unwrap();
        let db_path = db_dir.path().join("test.db");

        let video = source.path().join("The.Office.S02E03.720p.BluRay.x264-DEMAND.mkv");
        fs::write(&video, b"series data").unwrap();

        let watcher = setup_watcher(output.path(), &db_path);
        watcher
            .process_single_file(&video, source.path(), WatcherMode::Review)
            .unwrap();

        let events = watcher
            .history
            .list_watcher_events(Some(source.path()), None)
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, WatcherAction::Queued);
    }

    // -----------------------------------------------------------------------
    // Test 6: auto mode prunes old events after logging
    // -----------------------------------------------------------------------

    #[test]
    fn auto_mode_prunes_events_after_logging() {
        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();
        let db_dir = TempDir::new().unwrap();
        let db_path = db_dir.path().join("test.db");

        let config = test_config(output.path());
        let history = HistoryDb::open(&db_path).unwrap();

        // Set very low max events to force pruning
        let mut watcher = WatcherManager::new(config, history);
        watcher.max_activity_events = 2;

        // Process 3 files in auto mode
        for i in 1..=3 {
            let video = source.path().join(format!("Movie.{}.2020.mkv", 2000 + i));
            fs::write(&video, format!("video data {i}").as_bytes()).unwrap();
            watcher
                .process_single_file(&video, source.path(), WatcherMode::Auto)
                .unwrap();
        }

        // Should only have max_activity_events (2) events, not 3
        let events = watcher
            .history
            .list_watcher_events(Some(source.path()), None)
            .unwrap();
        assert!(
            events.len() <= 2,
            "events should be pruned to max_activity_events (2), got {}",
            events.len()
        );
    }

    // -----------------------------------------------------------------------
    // Test 7: is_video_file correctly identifies extensions
    // -----------------------------------------------------------------------

    #[test]
    fn is_video_file_accepts_video_extensions() {
        let video_exts = ["mkv", "mp4", "avi", "m4v", "mov", "wmv", "ts", "flv", "webm"];
        for ext in &video_exts {
            let path = PathBuf::from(format!("file.{ext}"));
            assert!(
                is_video_file(&path),
                "{ext} should be recognised as video"
            );
        }
    }

    #[test]
    fn is_video_file_rejects_non_video_extensions() {
        let non_video = ["txt", "srt", "nfo", "jpg", "png", "sub", "idx", "ass"];
        for ext in &non_video {
            let path = PathBuf::from(format!("file.{ext}"));
            assert!(
                !is_video_file(&path),
                "{ext} should NOT be recognised as video"
            );
        }
    }

    #[test]
    fn is_video_file_case_insensitive() {
        assert!(is_video_file(Path::new("file.MKV")));
        assert!(is_video_file(Path::new("file.Mp4")));
        assert!(is_video_file(Path::new("file.AVI")));
    }

    #[test]
    fn is_video_file_rejects_no_extension() {
        assert!(!is_video_file(Path::new("noextension")));
    }
}
