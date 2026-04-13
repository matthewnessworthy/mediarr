//! Filesystem watcher for automatic media file processing.
//!
//! Monitors folders for new video files using notify-debouncer-full,
//! then either auto-renames or queues for review based on configuration.
//! Uses a channel bridge pattern to connect notify's sync callbacks to
//! tokio's async runtime.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use notify::RecursiveMode;
use notify_debouncer_full::new_debouncer;
use tracing::{debug, error, info, warn};

use crate::config::Config;
use crate::error::{MediError, Result};
use crate::history::HistoryDb;
use crate::renamer::{RenamePlan, RenamePlanEntry, Renamer};
use crate::scanner::Scanner;
use crate::types::{
    MediaInfo, ReviewQueueEntry, ReviewStatus, WatcherAction, WatcherEvent, WatcherMode,
};

/// Default maximum activity events kept per watch path (per D-07).
const DEFAULT_MAX_EVENTS: usize = 500;

/// How long a processed path stays in the deduplication cache.
/// Events for the same canonical path within this window are ignored.
const DEDUP_WINDOW: Duration = Duration::from_secs(30);

use crate::fs_util::is_video_file;

/// Callback type invoked after each watcher event is logged to SQLite.
type EventCallback = Box<dyn Fn(&WatcherEvent) + Send>;

/// Manages filesystem watching for a single folder.
///
/// Bridges notify's synchronous callbacks to tokio's async runtime using
/// a channel bridge pattern. Processes debounced filesystem events by
/// either auto-renaming or queuing for review.
pub struct WatcherManager {
    scanner: Scanner,
    renamer: Renamer,
    history: HistoryDb,
    max_activity_events: usize,
    on_event: Option<EventCallback>,
    /// Recently-processed paths — prevents duplicate processing when the OS
    /// fires multiple filesystem events for a single logical file arrival.
    /// Maps canonical path -> time it was processed. Entries older than
    /// `DEDUP_WINDOW` are pruned on each event batch.
    recently_processed: HashMap<PathBuf, Instant>,
}

impl WatcherManager {
    /// Create a new WatcherManager from the application config and history database.
    pub fn new(config: Config, history: HistoryDb) -> Self {
        let scanner = Scanner::new(config.clone());
        let renamer = Renamer::from_config(&config.general);
        Self {
            scanner,
            renamer,
            history,
            max_activity_events: DEFAULT_MAX_EVENTS,
            on_event: None,
            recently_processed: HashMap::new(),
        }
    }

    /// Set an optional callback invoked after each watcher event is logged to SQLite.
    /// The callback runs in the watcher thread context.
    pub fn set_on_event(&mut self, callback: EventCallback) {
        self.on_event = Some(callback);
    }

    /// Notify the on_event callback if one is set.
    fn notify_event(&self, event: &WatcherEvent) {
        if let Some(ref cb) = self.on_event {
            cb(event);
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
        &mut self,
        watch_path: &Path,
        mode: WatcherMode,
        debounce_seconds: u64,
        shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> Result<()> {
        self.run_inner(watch_path, mode, debounce_seconds, shutdown, None)
            .await
    }

    /// Like [`run`], but sends a signal on `init_tx` once the debouncer is
    /// watching and the event loop is about to start. If initialization fails,
    /// the error is sent on the channel instead. This lets the caller (e.g.
    /// a Tauri command) detect and report early failures rather than having
    /// the thread die silently.
    pub async fn run_with_init_signal(
        &mut self,
        watch_path: &Path,
        mode: WatcherMode,
        debounce_seconds: u64,
        shutdown: tokio::sync::watch::Receiver<bool>,
        init_tx: std::sync::mpsc::SyncSender<std::result::Result<(), String>>,
    ) -> Result<()> {
        self.run_inner(watch_path, mode, debounce_seconds, shutdown, Some(init_tx))
            .await
    }

    /// Internal implementation shared by [`run`] and [`run_with_init_signal`].
    async fn run_inner(
        &mut self,
        watch_path: &Path,
        mode: WatcherMode,
        debounce_seconds: u64,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
        init_tx: Option<std::sync::mpsc::SyncSender<std::result::Result<(), String>>>,
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
            move |result: std::result::Result<
                Vec<notify_debouncer_full::DebouncedEvent>,
                Vec<notify::Error>,
            >| {
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
        .map_err(|e| {
            let msg = format!("failed to create debouncer: {e}");
            if let Some(ref tx) = init_tx {
                let _ = tx.send(Err(msg.clone()));
            }
            MediError::Watcher(msg)
        })?;

        // Start watching the path
        debouncer
            .watch(watch_path, RecursiveMode::Recursive)
            .map_err(|e| {
                let msg = format!("failed to watch path: {e}");
                if let Some(ref tx) = init_tx {
                    let _ = tx.send(Err(msg.clone()));
                }
                MediError::Watcher(msg)
            })?;

        // Bridge thread: forward from sync channel to async channel.
        // Named for easier debugging in thread dumps and profilers.
        let bridge_async_tx = async_tx.clone();
        std::thread::Builder::new()
            .name("mediarr-watcher-bridge".to_string())
            .spawn(move || {
                while let Ok(events) = sync_rx.recv() {
                    if bridge_async_tx.send(events).is_err() {
                        break; // async receiver dropped, stop bridge
                    }
                }
            })
            .map_err(|e| {
                let msg = format!("failed to spawn bridge thread: {e}");
                if let Some(ref tx) = init_tx {
                    let _ = tx.send(Err(msg.clone()));
                }
                MediError::Watcher(msg)
            })?;

        // Signal successful initialization — watcher is watching and event
        // loop is about to start.
        if let Some(tx) = init_tx {
            let _ = tx.send(Ok(()));
        }
        info!(path = %watch_path.display(), "watcher initialized and running");

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
    /// Filters for events that indicate a new file has appeared in the watched
    /// directory, then delegates to [`process_single_file`] for each.
    ///
    /// Accepted event kinds:
    /// - `Create(_)` — new file written (e.g. `cp`, download)
    /// - `Modify(Name(RenameMode::To))` — rename destination (Linux/inotify)
    /// - `Modify(Name(RenameMode::Both))` — matched rename pair (debouncer
    ///   stitched from+to); destination is `paths.last()`
    /// - `Modify(Name(RenameMode::Any))` — unmatched rename on macOS FSEvents;
    ///   the debouncer stores these only when the path exists (i.e. "move in")
    fn process_debounced_events(
        &mut self,
        events: &[notify_debouncer_full::DebouncedEvent],
        watch_path: &Path,
        mode: WatcherMode,
    ) {
        use notify::event::{ModifyKind, RenameMode};
        use notify::EventKind;

        // Prune expired entries from the deduplication cache.
        let now = Instant::now();
        self.recently_processed
            .retain(|_, processed_at| now.duration_since(*processed_at) < DEDUP_WINDOW);

        for event in events {
            // Determine which path to process based on event kind.
            // For rename-both events the destination is the last path;
            // for all other accepted events the paths slice itself is fine.
            let target_paths: &[std::path::PathBuf] = match &event.kind {
                EventKind::Create(_) => &event.paths,

                EventKind::Modify(ModifyKind::Name(RenameMode::To | RenameMode::Any)) => {
                    &event.paths
                }

                EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
                    // paths = [old, new]; we only care about the destination
                    if let Some(dest) = event.paths.last() {
                        std::slice::from_ref(dest)
                    } else {
                        continue;
                    }
                }

                _ => continue,
            };

            for path in target_paths {
                if !is_video_file(path) {
                    debug!(path = %path.display(), "skipping non-video file");
                    continue;
                }

                // Canonicalize the path for deduplication. On macOS, notify
                // resolves symlinks (e.g. /var -> /private/var) so the event
                // path may differ from what the user originally provided.
                // Fall back to the raw path if canonicalization fails (e.g.
                // the file was already moved by a prior event in this batch).
                let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
                let raw = path.to_path_buf();

                if self.recently_processed.contains_key(&canonical)
                    || self.recently_processed.contains_key(&raw)
                {
                    debug!(path = %path.display(), "skipping recently-processed file (dedup)");
                    continue;
                }

                // Also skip if the file no longer exists — it was likely
                // already processed and moved by a prior event.
                if !path.exists() {
                    debug!(path = %path.display(), "skipping non-existent file (already moved)");
                    continue;
                }

                // Mark as processed BEFORE processing, so concurrent events
                // for the same path in this batch are also skipped.
                // Insert both canonical and raw paths so dedup catches either form.
                if canonical != raw {
                    self.recently_processed.insert(raw, now);
                }
                self.recently_processed.insert(canonical, now);

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
        &mut self,
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
                self.notify_event(&event);
                self.history
                    .prune_watcher_events(watch_path, self.max_activity_events)?;
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
                    // Add all destination paths to the dedup cache so the
                    // watcher doesn't re-process its own output when watching
                    // recursively and output lands inside the watched folder.
                    let now = Instant::now();
                    for r in &results {
                        if r.success {
                            let canonical = r
                                .dest_path
                                .canonicalize()
                                .unwrap_or_else(|_| r.dest_path.clone());
                            self.recently_processed.insert(canonical, now);
                        }
                    }

                    // Record batch in history
                    let media_info_map: std::collections::HashMap<String, MediaInfo> = results
                        .iter()
                        .filter(|r| r.success)
                        .map(|r| {
                            (
                                r.source_path.to_string_lossy().to_string(),
                                scan_result.media_info.clone(),
                            )
                        })
                        .collect();

                    let batch_id =
                        match self.history.record_rename_results(&results, &media_info_map) {
                            Ok(id) => Some(id).filter(|s| !s.is_empty()),
                            Err(e) => {
                                warn!(error = %e, "failed to record rename batch in history");
                                None
                            }
                        };

                    // Log watcher event
                    let event = WatcherEvent {
                        id: None,
                        timestamp: timestamp.clone(),
                        watch_path: watch_path.to_path_buf(),
                        filename,
                        action: WatcherAction::Renamed,
                        detail: Some(format!("{}", scan_result.proposed_path.display())),
                        batch_id,
                    };
                    self.history.log_watcher_event(&event)?;
                    self.notify_event(&event);
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
                    self.notify_event(&event);
                }

                // Prune old events
                self.history
                    .prune_watcher_events(watch_path, self.max_activity_events)?;
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
                self.notify_event(&event);

                // Prune old events
                self.history
                    .prune_watcher_events(watch_path, self.max_activity_events)?;
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
    use std::path::PathBuf;
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

        let video = source
            .path()
            .join("Inception.2010.1080p.BluRay.x264-GROUP.mkv");
        fs::write(&video, b"video data").unwrap();

        let mut watcher = setup_watcher(output.path(), &db_path);
        let result = watcher.process_single_file(&video, source.path(), WatcherMode::Auto);
        assert!(
            result.is_ok(),
            "process_single_file should succeed: {:?}",
            result
        );

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

        let video = source
            .path()
            .join("Inception.2010.1080p.BluRay.x264-GROUP.mkv");
        fs::write(&video, b"video data").unwrap();

        let mut watcher = setup_watcher(output.path(), &db_path);
        let result = watcher.process_single_file(&video, source.path(), WatcherMode::Review);
        assert!(
            result.is_ok(),
            "process_single_file should succeed: {:?}",
            result
        );

        // Source file should still exist (not renamed)
        assert!(
            video.exists(),
            "source file should NOT be moved in review mode"
        );

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

        let mut watcher = setup_watcher(output.path(), &db_path);
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

        let video = source
            .path()
            .join("The.Office.S02E03.720p.BluRay.x264-DEMAND.mkv");
        fs::write(&video, b"series data").unwrap();

        let mut watcher = setup_watcher(output.path(), &db_path);
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

        let video = source
            .path()
            .join("The.Office.S02E03.720p.BluRay.x264-DEMAND.mkv");
        fs::write(&video, b"series data").unwrap();

        let mut watcher = setup_watcher(output.path(), &db_path);
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
        let video_exts = [
            "mkv", "mp4", "avi", "m4v", "mov", "wmv", "ts", "flv", "webm",
        ];
        for ext in &video_exts {
            let path = PathBuf::from(format!("file.{ext}"));
            assert!(is_video_file(&path), "{ext} should be recognised as video");
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

    // -----------------------------------------------------------------------
    // Test: Full watcher run() loop detects new files via filesystem events
    // -----------------------------------------------------------------------

    #[test]
    fn run_detects_new_file_via_filesystem_event() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();
        let db_dir = TempDir::new().unwrap();
        let db_path = db_dir.path().join("test.db");

        let config = test_config(output.path());
        let history = HistoryDb::open(&db_path).unwrap();

        let event_count = Arc::new(AtomicUsize::new(0));
        let event_count_clone = event_count.clone();

        let mut watcher = WatcherManager::new(config, history);
        watcher.set_on_event(Box::new(move |_event| {
            event_count_clone.fetch_add(1, Ordering::SeqCst);
        }));

        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let (init_tx, init_rx) = std::sync::mpsc::sync_channel(1);

        let watch_path = source.path().to_path_buf();
        let watch_path_clone = watch_path.clone();

        // Spawn dedicated OS thread with its own single-threaded tokio runtime
        // (mirrors the Tauri command pattern — WatcherManager is !Send due to rusqlite)
        let thread_handle = std::thread::Builder::new()
            .name("test-watcher".to_string())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("build tokio runtime");
                rt.block_on(watcher.run_with_init_signal(
                    &watch_path_clone,
                    WatcherMode::Auto,
                    1,
                    shutdown_rx,
                    init_tx,
                ))
            })
            .expect("spawn watcher thread");

        // Wait for watcher to confirm it's ready
        let init_result = init_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("watcher init signal should arrive");
        assert!(init_result.is_ok(), "watcher init failed: {:?}", init_result);

        // Create a video file in the watched directory
        let video = watch_path.join("Inception.2010.1080p.BluRay.x264-GROUP.mkv");
        fs::write(&video, b"video data").unwrap();

        // Wait for debounce (1 second timeout) + processing time
        // The debouncer ticks at timeout/4 = 250ms, events emitted after timeout (1s)
        std::thread::sleep(std::time::Duration::from_secs(5));

        // Shutdown the watcher
        let _ = shutdown_tx.send(true);
        let result = thread_handle
            .join()
            .expect("watcher thread should not panic");
        assert!(
            result.is_ok(),
            "watcher run should complete without error: {:?}",
            result
        );

        // Check that at least one event was processed
        let count = event_count.load(Ordering::SeqCst);
        assert!(
            count > 0,
            "expected at least 1 watcher event from filesystem, got {count}"
        );
    }

    // -----------------------------------------------------------------------
    // Test: Watcher detects files moved (renamed) into the watched directory.
    // On macOS FSEvents emits RenameMode::Any for moves; this test verifies
    // the filter correctly catches these events.
    // -----------------------------------------------------------------------

    #[test]
    fn run_detects_moved_file_via_rename_event() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let watched = TempDir::new().unwrap();
        let staging = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();
        let db_dir = TempDir::new().unwrap();
        let db_path = db_dir.path().join("test.db");

        let config = test_config(output.path());
        let history = HistoryDb::open(&db_path).unwrap();

        let event_count = Arc::new(AtomicUsize::new(0));
        let event_count_clone = event_count.clone();

        let mut watcher = WatcherManager::new(config, history);
        watcher.set_on_event(Box::new(move |_event| {
            event_count_clone.fetch_add(1, Ordering::SeqCst);
        }));

        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let (init_tx, init_rx) = std::sync::mpsc::sync_channel(1);

        let watch_path = watched.path().to_path_buf();
        let watch_path_clone = watch_path.clone();

        let thread_handle = std::thread::Builder::new()
            .name("test-watcher-mv".to_string())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("build tokio runtime");
                rt.block_on(watcher.run_with_init_signal(
                    &watch_path_clone,
                    WatcherMode::Auto,
                    1,
                    shutdown_rx,
                    init_tx,
                ))
            })
            .expect("spawn watcher thread");

        // Wait for watcher to confirm it's ready
        let init_result = init_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("watcher init signal should arrive");
        assert!(init_result.is_ok(), "watcher init failed: {:?}", init_result);

        // Create a video file OUTSIDE the watched directory, then move it in.
        // This triggers a rename event (RenameMode::Any on macOS) rather than
        // a Create event.
        let staged_video = staging
            .path()
            .join("Inception.2010.1080p.BluRay.x264-GROUP.mkv");
        fs::write(&staged_video, b"video data").unwrap();
        let dest_video = watch_path.join("Inception.2010.1080p.BluRay.x264-GROUP.mkv");
        fs::rename(&staged_video, &dest_video).unwrap();

        // Wait for debounce + processing
        std::thread::sleep(std::time::Duration::from_secs(5));

        // Shutdown the watcher
        let _ = shutdown_tx.send(true);
        let result = thread_handle
            .join()
            .expect("watcher thread should not panic");
        assert!(
            result.is_ok(),
            "watcher run should complete without error: {:?}",
            result
        );

        let count = event_count.load(Ordering::SeqCst);
        assert!(
            count > 0,
            "expected at least 1 watcher event from move/rename, got {count}"
        );
    }

    // -----------------------------------------------------------------------
    // Test: Watcher works with default config (no output_dir) — in-place rename.
    // This mimics the real Tauri app scenario more closely.
    // -----------------------------------------------------------------------

    #[test]
    fn run_works_with_default_config_no_output_dir() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let source = TempDir::new().unwrap();
        let db_dir = TempDir::new().unwrap();
        let db_path = db_dir.path().join("test.db");

        // Use default config — output_dir is None (in-place rename)
        let config = Config::default();
        let history = HistoryDb::open(&db_path).unwrap();

        let event_count = Arc::new(AtomicUsize::new(0));
        let event_count_clone = event_count.clone();

        let mut watcher = WatcherManager::new(config, history);
        watcher.set_on_event(Box::new(move |event| {
            eprintln!(
                "[test] on_event callback: action={:?} filename={}",
                event.action, event.filename
            );
            event_count_clone.fetch_add(1, Ordering::SeqCst);
        }));

        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let (init_tx, init_rx) = std::sync::mpsc::sync_channel(1);

        let watch_path = source.path().to_path_buf();
        let watch_path_clone = watch_path.clone();

        let thread_handle = std::thread::Builder::new()
            .name("test-watcher-nooutput".to_string())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("build tokio runtime");
                rt.block_on(watcher.run_with_init_signal(
                    &watch_path_clone,
                    WatcherMode::Auto,
                    1,
                    shutdown_rx,
                    init_tx,
                ))
            })
            .expect("spawn watcher thread");

        // Wait for watcher to confirm it's ready
        let init_result = init_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("watcher init signal should arrive");
        assert!(init_result.is_ok(), "watcher init failed: {:?}", init_result);

        // Create a video file in the watched directory
        let video = watch_path.join("Inception.2010.1080p.BluRay.x264-GROUP.mkv");
        fs::write(&video, b"video data").unwrap();

        // Wait for debounce (1s) + processing time
        std::thread::sleep(std::time::Duration::from_secs(5));

        // Shutdown the watcher
        let _ = shutdown_tx.send(true);
        let result = thread_handle
            .join()
            .expect("watcher thread should not panic");
        assert!(
            result.is_ok(),
            "watcher run should complete without error: {:?}",
            result
        );

        let count = event_count.load(Ordering::SeqCst);
        assert!(
            count > 0,
            "expected at least 1 watcher event with default config (no output_dir), got {count}"
        );
    }

    // -----------------------------------------------------------------------
    // Test: Rapid file additions are debounced into fewer callback invocations
    // -----------------------------------------------------------------------

    #[test]
    fn run_debounces_rapid_file_additions() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();
        let db_dir = TempDir::new().unwrap();
        let db_path = db_dir.path().join("test.db");

        let config = test_config(output.path());
        let history = HistoryDb::open(&db_path).unwrap();

        let event_count = Arc::new(AtomicUsize::new(0));
        let event_count_clone = event_count.clone();

        let mut watcher = WatcherManager::new(config, history);
        watcher.set_on_event(Box::new(move |_event| {
            event_count_clone.fetch_add(1, Ordering::SeqCst);
        }));

        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let (init_tx, init_rx) = std::sync::mpsc::sync_channel(1);

        let watch_path = source.path().to_path_buf();
        let watch_path_clone = watch_path.clone();

        let thread_handle = std::thread::Builder::new()
            .name("test-watcher-debounce".to_string())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("build tokio runtime");
                rt.block_on(
                    // 2-second debounce window
                    watcher.run_with_init_signal(
                        &watch_path_clone,
                        WatcherMode::Auto,
                        2,
                        shutdown_rx,
                        init_tx,
                    ),
                )
            })
            .expect("spawn watcher thread");

        // Wait for watcher to confirm it's ready
        let init_result = init_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("watcher init signal should arrive");
        assert!(init_result.is_ok(), "watcher init failed: {:?}", init_result);

        // Rapidly create 5 video files within the debounce window
        for i in 1..=5 {
            let video = watch_path.join(format!("Movie.{}.2020.1080p.mkv", 2000 + i));
            fs::write(&video, format!("video data {i}").as_bytes()).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        // Wait for debounce (2s) + processing time
        std::thread::sleep(std::time::Duration::from_secs(5));

        // Shutdown
        let _ = shutdown_tx.send(true);
        let result = thread_handle
            .join()
            .expect("watcher thread should not panic");
        assert!(
            result.is_ok(),
            "watcher run should complete without error: {:?}",
            result
        );

        // All 5 files should have been processed (debouncer batches them)
        let count = event_count.load(Ordering::SeqCst);
        assert!(
            count >= 5,
            "expected at least 5 watcher events from rapid additions, got {count}"
        );
    }

    // -----------------------------------------------------------------------
    // Test: Callback is invoked with correct WatcherEvent data
    // -----------------------------------------------------------------------

    #[test]
    fn callback_receives_correct_event_data() {
        use std::sync::{Arc, Mutex};

        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();
        let db_dir = TempDir::new().unwrap();
        let db_path = db_dir.path().join("test.db");

        let config = test_config(output.path());
        let history = HistoryDb::open(&db_path).unwrap();

        let captured_events: Arc<Mutex<Vec<WatcherEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let captured_clone = captured_events.clone();

        let mut watcher = WatcherManager::new(config, history);
        watcher.set_on_event(Box::new(move |event| {
            captured_clone.lock().unwrap().push(event.clone());
        }));

        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let watch_path = source.path().to_path_buf();
        let watch_path_clone = watch_path.clone();

        let thread_handle = std::thread::Builder::new()
            .name("test-watcher-callback".to_string())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("build tokio runtime");
                rt.block_on(watcher.run(&watch_path_clone, WatcherMode::Auto, 1, shutdown_rx))
            })
            .expect("spawn watcher thread");

        // Wait for watcher to start
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Create a video file
        let video = watch_path.join("Inception.2010.1080p.BluRay.x264-GROUP.mkv");
        fs::write(&video, b"video data").unwrap();

        // Wait for debounce + processing
        std::thread::sleep(std::time::Duration::from_secs(4));

        // Shutdown
        let _ = shutdown_tx.send(true);
        let result = thread_handle
            .join()
            .expect("watcher thread should not panic");
        assert!(result.is_ok(), "watcher run should succeed: {:?}", result);

        let events = captured_events.lock().unwrap();
        assert!(
            !events.is_empty(),
            "should have captured at least one event"
        );

        let event = &events[0];
        assert_eq!(event.action, WatcherAction::Renamed);
        assert!(
            event.filename.contains("Inception"),
            "filename should contain 'Inception', got: {}",
            event.filename
        );
        assert_eq!(event.watch_path, watch_path);
        assert!(event.detail.is_some(), "event should have detail");
        assert!(
            event.batch_id.is_some(),
            "renamed event should have batch_id"
        );
    }

    // -----------------------------------------------------------------------
    // Test: Review mode callback receives Queued action
    // -----------------------------------------------------------------------

    #[test]
    fn review_mode_callback_receives_queued_action() {
        use std::sync::{Arc, Mutex};

        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();
        let db_dir = TempDir::new().unwrap();
        let db_path = db_dir.path().join("test.db");

        let config = test_config(output.path());
        let history = HistoryDb::open(&db_path).unwrap();

        let captured_events: Arc<Mutex<Vec<WatcherEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let captured_clone = captured_events.clone();

        let mut watcher = WatcherManager::new(config, history);
        watcher.set_on_event(Box::new(move |event| {
            captured_clone.lock().unwrap().push(event.clone());
        }));

        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let (init_tx, init_rx) = std::sync::mpsc::sync_channel(1);

        let watch_path = source.path().to_path_buf();
        let watch_path_clone = watch_path.clone();

        let thread_handle = std::thread::Builder::new()
            .name("test-watcher-review-cb".to_string())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("build tokio runtime");
                rt.block_on(watcher.run_with_init_signal(
                    &watch_path_clone,
                    WatcherMode::Review,
                    1,
                    shutdown_rx,
                    init_tx,
                ))
            })
            .expect("spawn watcher thread");

        // Wait for watcher to confirm it's ready
        let init_result = init_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("watcher init signal should arrive");
        assert!(init_result.is_ok(), "watcher init failed: {:?}", init_result);

        // Create a video file
        let video = watch_path.join("The.Office.S02E03.720p.mkv");
        fs::write(&video, b"series data").unwrap();

        // Wait for debounce + processing
        std::thread::sleep(std::time::Duration::from_secs(5));

        // Shutdown
        let _ = shutdown_tx.send(true);
        let result = thread_handle
            .join()
            .expect("watcher thread should not panic");
        assert!(result.is_ok(), "watcher run should succeed: {:?}", result);

        let events = captured_events.lock().unwrap();
        assert!(
            !events.is_empty(),
            "should have captured at least one event"
        );
        assert_eq!(events[0].action, WatcherAction::Queued);

        // File should still exist (review mode doesn't rename)
        assert!(
            video.exists(),
            "source file should still exist in review mode"
        );
    }

    // -----------------------------------------------------------------------
    // Test: Non-video files dropped into watched directory are ignored
    // -----------------------------------------------------------------------

    #[test]
    fn run_ignores_non_video_files() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();
        let db_dir = TempDir::new().unwrap();
        let db_path = db_dir.path().join("test.db");

        let config = test_config(output.path());
        let history = HistoryDb::open(&db_path).unwrap();

        let event_count = Arc::new(AtomicUsize::new(0));
        let event_count_clone = event_count.clone();

        let mut watcher = WatcherManager::new(config, history);
        watcher.set_on_event(Box::new(move |_event| {
            event_count_clone.fetch_add(1, Ordering::SeqCst);
        }));

        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let (init_tx, init_rx) = std::sync::mpsc::sync_channel(1);

        let watch_path = source.path().to_path_buf();
        let watch_path_clone = watch_path.clone();

        let thread_handle = std::thread::Builder::new()
            .name("test-watcher-nonvid".to_string())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("build tokio runtime");
                rt.block_on(watcher.run_with_init_signal(
                    &watch_path_clone,
                    WatcherMode::Auto,
                    1,
                    shutdown_rx,
                    init_tx,
                ))
            })
            .expect("spawn watcher thread");

        // Wait for watcher to confirm it's ready
        let init_result = init_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("watcher init signal should arrive");
        assert!(init_result.is_ok(), "watcher init failed: {:?}", init_result);

        // Create non-video files only
        fs::write(watch_path.join("readme.txt"), b"text").unwrap();
        fs::write(watch_path.join("image.jpg"), b"image").unwrap();
        fs::write(watch_path.join("subtitle.srt"), b"sub").unwrap();

        // Wait for debounce + processing
        std::thread::sleep(std::time::Duration::from_secs(5));

        // Shutdown
        let _ = shutdown_tx.send(true);
        let result = thread_handle
            .join()
            .expect("watcher thread should not panic");
        assert!(result.is_ok(), "watcher run should succeed: {:?}", result);

        // No events should have been processed (all non-video)
        let count = event_count.load(Ordering::SeqCst);
        assert_eq!(
            count, 0,
            "non-video files should not trigger watcher events, got {count}"
        );
    }

    // -----------------------------------------------------------------------
    // Test: Shutdown signal stops the watcher cleanly
    // -----------------------------------------------------------------------

    #[test]
    fn shutdown_signal_stops_watcher() {
        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();
        let db_dir = TempDir::new().unwrap();
        let db_path = db_dir.path().join("test.db");

        let config = test_config(output.path());
        let history = HistoryDb::open(&db_path).unwrap();
        let mut watcher = WatcherManager::new(config, history);

        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let watch_path = source.path().to_path_buf();

        let thread_handle = std::thread::Builder::new()
            .name("test-watcher-shutdown".to_string())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("build tokio runtime");
                rt.block_on(watcher.run(&watch_path, WatcherMode::Auto, 1, shutdown_rx))
            })
            .expect("spawn watcher thread");

        // Let watcher start
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Send shutdown immediately
        let _ = shutdown_tx.send(true);

        // Thread should exit within a reasonable time
        let result = thread_handle
            .join()
            .expect("watcher thread should not panic");
        assert!(
            result.is_ok(),
            "watcher should shut down cleanly: {:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // Test: run_with_init_signal reports success on valid path
    // -----------------------------------------------------------------------

    #[test]
    fn run_with_init_signal_reports_success() {
        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();
        let db_dir = TempDir::new().unwrap();
        let db_path = db_dir.path().join("test.db");

        let config = test_config(output.path());
        let history = HistoryDb::open(&db_path).unwrap();
        let mut watcher = WatcherManager::new(config, history);

        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let (init_tx, init_rx) =
            std::sync::mpsc::sync_channel::<std::result::Result<(), String>>(1);

        let watch_path = source.path().to_path_buf();

        let thread_handle = std::thread::Builder::new()
            .name("test-watcher-init".to_string())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("build tokio runtime");
                rt.block_on(watcher.run_with_init_signal(
                    &watch_path,
                    WatcherMode::Auto,
                    1,
                    shutdown_rx,
                    init_tx,
                ))
            })
            .expect("spawn watcher thread");

        // Should receive Ok(()) from init signal
        let init_result = init_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("should receive init signal");
        assert!(
            init_result.is_ok(),
            "init signal should be Ok, got: {:?}",
            init_result
        );

        // Shutdown
        let _ = shutdown_tx.send(true);
        let _ = thread_handle.join();
    }

    // -----------------------------------------------------------------------
    // Test: run_with_init_signal reports error for invalid path
    // -----------------------------------------------------------------------

    #[test]
    fn run_with_init_signal_reports_error_for_invalid_path() {
        let output = TempDir::new().unwrap();
        let db_dir = TempDir::new().unwrap();
        let db_path = db_dir.path().join("test.db");

        let config = test_config(output.path());
        let history = HistoryDb::open(&db_path).unwrap();
        let mut watcher = WatcherManager::new(config, history);

        let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let (init_tx, init_rx) =
            std::sync::mpsc::sync_channel::<std::result::Result<(), String>>(1);

        // Use a path that doesn't exist
        let watch_path = PathBuf::from("/tmp/nonexistent_watcher_test_dir_12345");

        let thread_handle = std::thread::Builder::new()
            .name("test-watcher-init-err".to_string())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("build tokio runtime");
                rt.block_on(watcher.run_with_init_signal(
                    &watch_path,
                    WatcherMode::Auto,
                    1,
                    shutdown_rx,
                    init_tx,
                ))
            })
            .expect("spawn watcher thread");

        // Should receive an error from init signal
        let init_result = init_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("should receive init signal");
        assert!(
            init_result.is_err(),
            "init signal should be Err for invalid path, got: {:?}",
            init_result
        );

        let _ = thread_handle.join();
    }

    // -----------------------------------------------------------------------
    // Regression test for R004: watcher dedup checks both canonical and raw path.
    // When a file is accessed via a symlink (e.g. /var -> /private/var on macOS),
    // the canonical path differs from the raw path. The dedup cache must catch both.
    // -----------------------------------------------------------------------

    #[test]
    fn dedup_catches_raw_path_when_canonical_is_cached() {
        use std::time::Instant;

        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();
        let db_dir = TempDir::new().unwrap();
        let db_path = db_dir.path().join("test.db");

        let config = test_config(output.path());
        let history = HistoryDb::open(&db_path).unwrap();
        let mut watcher = WatcherManager::new(config, history);

        // Create a video file
        let video = source
            .path()
            .join("Inception.2010.1080p.BluRay.x264-GROUP.mkv");
        fs::write(&video, b"video data").unwrap();

        // Simulate: the canonical path is already in the dedup cache
        let canonical = video.canonicalize().unwrap();
        watcher
            .recently_processed
            .insert(canonical.clone(), Instant::now());

        // Create a symlink to the source dir so the raw path differs
        let link_parent = TempDir::new().unwrap();
        let link_dir = link_parent.path().join("link_to_source");
        #[cfg(unix)]
        std::os::unix::fs::symlink(source.path(), &link_dir).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(source.path(), &link_dir).unwrap();

        let raw_video = link_dir.join("Inception.2010.1080p.BluRay.x264-GROUP.mkv");
        assert!(raw_video.exists(), "symlinked video should exist");
        assert_ne!(
            raw_video, canonical,
            "raw and canonical should differ for this test to be meaningful"
        );

        // Fire a debounced event with the raw (symlinked) path
        let event = notify_debouncer_full::DebouncedEvent {
            event: notify::Event {
                kind: notify::EventKind::Create(notify::event::CreateKind::File),
                paths: vec![raw_video.clone()],
                attrs: Default::default(),
            },
            time: Instant::now(),
        };

        // Track events via callback
        let event_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = event_count.clone();
        watcher.set_on_event(Box::new(move |_| {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }));

        watcher.process_debounced_events(&[event], source.path(), WatcherMode::Auto);

        // The event should have been deduped — canonical resolves to the cached path
        let count = event_count.load(std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            count, 0,
            "event should be deduped because canonical matches cached path, but got {count} events"
        );
    }

    /// Regression test for R004: after processing a file via symlinked path,
    /// both canonical AND raw paths should be in the dedup cache.
    #[test]
    fn dedup_inserts_both_canonical_and_raw_paths() {
        use std::time::Instant;

        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();
        let db_dir = TempDir::new().unwrap();
        let db_path = db_dir.path().join("test.db");

        let config = test_config(output.path());
        let history = HistoryDb::open(&db_path).unwrap();
        let mut watcher = WatcherManager::new(config, history);

        // Create a video file
        let video = source
            .path()
            .join("Test.Movie.2020.720p.WEB.x264.mkv");
        fs::write(&video, b"video data").unwrap();

        let canonical = video.canonicalize().unwrap();

        // Create symlink so raw != canonical
        let link_parent = TempDir::new().unwrap();
        let link_dir = link_parent.path().join("link_to_source");
        #[cfg(unix)]
        std::os::unix::fs::symlink(source.path(), &link_dir).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(source.path(), &link_dir).unwrap();

        let raw_video = link_dir.join("Test.Movie.2020.720p.WEB.x264.mkv");
        assert_ne!(raw_video, canonical);

        // Process the file via the symlinked (raw) path
        let event = notify_debouncer_full::DebouncedEvent {
            event: notify::Event {
                kind: notify::EventKind::Create(notify::event::CreateKind::File),
                paths: vec![raw_video.clone()],
                attrs: Default::default(),
            },
            time: Instant::now(),
        };

        watcher.process_debounced_events(&[event], source.path(), WatcherMode::Auto);

        // Both canonical and raw paths should be in the dedup cache
        assert!(
            watcher.recently_processed.contains_key(&canonical),
            "canonical path should be in dedup cache"
        );
        assert!(
            watcher.recently_processed.contains_key(&raw_video),
            "raw (symlinked) path should also be in dedup cache"
        );
    }
}
