use std::path::{Path, PathBuf};

use tauri::{Emitter, State};
use tracing::{error, info};

use mediarr_core::{ReviewQueueEntry, ReviewStatus, WatcherConfig, WatcherEvent, WatcherManager};

use crate::error::{CommandError, CommandResult};
use crate::state::{ManagedState, WatcherHandle};

/// List all configured folder watchers from the current config.
#[tauri::command]
pub fn list_watchers(state: State<'_, ManagedState>) -> CommandResult<Vec<WatcherConfig>> {
    let state = state.lock().map_err(|_| CommandError::StateLock)?;
    Ok(state.config.watchers.clone())
}

/// List watcher events, optionally filtered by watch path.
#[tauri::command]
pub fn list_watcher_events(
    state: State<'_, ManagedState>,
    watch_path: Option<String>,
    limit: Option<u32>,
) -> CommandResult<Vec<WatcherEvent>> {
    let state = state.lock().map_err(|_| CommandError::StateLock)?;
    let path_ref = watch_path.as_deref().map(Path::new);
    let events = state
        .db
        .list_watcher_events(path_ref, limit.map(|l| l as usize))?;
    Ok(events)
}

/// List review queue entries, optionally filtered by watch path.
#[tauri::command]
pub fn list_review_queue(
    state: State<'_, ManagedState>,
    watch_path: Option<String>,
) -> CommandResult<Vec<ReviewQueueEntry>> {
    let state = state.lock().map_err(|_| CommandError::StateLock)?;
    let path_ref = watch_path.as_deref().map(Path::new);
    let entries = state.db.list_review_queue(path_ref, None)?;
    Ok(entries)
}

/// Update the review status of a queue entry.
#[tauri::command]
pub fn update_review_status(
    state: State<'_, ManagedState>,
    id: i64,
    status: String,
) -> CommandResult<()> {
    let state = state.lock().map_err(|_| CommandError::StateLock)?;
    let parsed_status = match status.as_str() {
        "pending" => ReviewStatus::Pending,
        "approved" => ReviewStatus::Approved,
        "rejected" => ReviewStatus::Rejected,
        other => {
            return Err(CommandError::Other(format!(
                "invalid review status: {other}"
            )))
        }
    };
    state.db.update_review_status(id, parsed_status)?;
    Ok(())
}

/// Start a folder watcher for the given path.
///
/// Spawns a dedicated OS thread with its own single-threaded tokio runtime
/// because `WatcherManager` holds a `rusqlite::Connection` which is `!Send`.
/// The watcher config must already exist in the application config.
///
/// Waits for the watcher thread to signal successful initialization before
/// returning. If the thread fails during setup (DB open, debouncer creation,
/// path watch), the error is propagated back to the frontend instead of being
/// silently swallowed.
#[tauri::command]
pub fn start_watcher(app: tauri::AppHandle, state: State<'_, ManagedState>, path: String) -> CommandResult<()> {
    let mut state = state.lock().map_err(|_| CommandError::StateLock)?;

    // Find the watcher config for this path
    let watcher_config = state
        .config
        .watchers
        .iter()
        .find(|w| w.path.to_string_lossy() == path)
        .cloned()
        .ok_or_else(|| CommandError::Other(format!("no watcher configured for path: {path}")))?;

    // Check if already running
    if state.active_watchers.contains_key(&path) {
        return Err(CommandError::Other(format!(
            "watcher already running for path: {path}"
        )));
    }

    // Resolve per-watcher settings onto the global config so Scanner and
    // Renamer operate with watcher-specific overrides transparently.
    let config = watcher_config.resolve_config(&state.config);
    let data_path = mediarr_core::config::default_data_path()
        .map_err(|e| CommandError::Other(format!("failed to determine data path: {e}")))?;
    let watch_path = PathBuf::from(&path);
    let mode = watcher_config.mode;
    let debounce = watcher_config.debounce_seconds;

    // Create event emission callback for real-time frontend updates
    let app_handle = app.clone();
    let on_event_callback: Box<dyn Fn(&mediarr_core::WatcherEvent) + Send> =
        Box::new(move |event: &mediarr_core::WatcherEvent| {
            let _ = app_handle.emit("watcher-event", event);
        });

    // Create shutdown channel
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // Init-result channel: the thread sends Ok(()) once the watcher is running,
    // or Err(message) if initialization fails. This prevents silent thread death.
    let (init_tx, init_rx) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);

    // Spawn dedicated thread with its own tokio runtime
    let thread_name = format!("watcher-{path}");
    let thread_handle = std::thread::Builder::new()
        .name(thread_name.clone())
        .spawn(move || {
            // Open a separate HistoryDb connection for this thread
            let db = match mediarr_core::HistoryDb::open(&data_path) {
                Ok(db) => db,
                Err(e) => {
                    let msg = format!("failed to open history db: {e}");
                    error!(path = %watch_path.display(), "{msg}");
                    let _ = init_tx.send(Err(msg));
                    return;
                }
            };

            let mut watcher = WatcherManager::new(config, db);
            watcher.set_on_event(on_event_callback);

            // Create a single-threaded tokio runtime for this thread
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    let msg = format!("failed to create tokio runtime: {e}");
                    error!(path = %watch_path.display(), "{msg}");
                    let _ = init_tx.send(Err(msg));
                    return;
                }
            };

            // Run the watcher; it sends the init signal internally once the
            // debouncer is watching and the event loop is about to start.
            if let Err(e) = rt.block_on(watcher.run_with_init_signal(
                &watch_path,
                mode,
                debounce,
                shutdown_rx,
                init_tx,
            )) {
                error!(path = %watch_path.display(), "watcher exited with error: {e}");
            }
        })
        .map_err(|e| CommandError::Other(format!("failed to spawn watcher thread: {e}")))?;

    // Wait for the thread to report initialization status.
    // This blocks the Tauri command handler briefly but ensures we catch
    // early failures (DB open, debouncer creation, path watch) instead of
    // silently swallowing them.
    init_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| CommandError::Other(format!("watcher init timed out: {e}")))?
        .map_err(CommandError::Other)?;

    info!(path = %path, "watcher started and confirmed running");

    // Persist active = true in config so watchers survive app restart
    if let Some(wc) = state.config.watchers.iter_mut().find(|w| w.path.to_string_lossy() == path) {
        wc.active = true;
    }
    let config_path = mediarr_core::config::default_config_path()
        .map_err(|e| CommandError::Other(format!("failed to determine config path: {e}")))?;
    state.config.save(&config_path)?;

    state.active_watchers.insert(
        path,
        WatcherHandle {
            shutdown_tx,
            thread_handle,
        },
    );

    Ok(())
}

/// Stop a running folder watcher for the given path.
///
/// Sends a shutdown signal via the watch channel. The watcher thread will
/// exit on its own after processing the signal. Does NOT join the thread
/// to avoid blocking the Tauri command handler.
#[tauri::command]
pub fn stop_watcher(state: State<'_, ManagedState>, path: String) -> CommandResult<()> {
    let mut state = state.lock().map_err(|_| CommandError::StateLock)?;

    let handle = state
        .active_watchers
        .remove(&path)
        .ok_or_else(|| CommandError::Other(format!("no running watcher for path: {path}")))?;

    // Send shutdown signal (ignore error if receiver already dropped)
    let _ = handle.shutdown_tx.send(true);

    // Persist active = false in config so watcher stays off after app restart
    if let Some(wc) = state.config.watchers.iter_mut().find(|w| w.path.to_string_lossy() == path) {
        wc.active = false;
    }
    if let Ok(config_path) = mediarr_core::config::default_config_path() {
        let _ = state.config.save(&config_path);
    }

    info!(path = %path, "watcher stop signal sent");

    Ok(())
}

/// Approve a review queue entry: execute the rename, record history, update status.
///
/// Mirrors the CLI's execute_review_rename flow. Rejects stale entries where
/// the source file no longer exists.
#[tauri::command]
pub fn approve_review_entry(
    state: State<'_, ManagedState>,
    id: i64,
) -> CommandResult<()> {
    let state = state.lock().map_err(|_| CommandError::StateLock)?;

    // 1. Find the pending review entry
    let entries = state.db.list_review_queue(None, None)?;
    let entry = entries
        .iter()
        .find(|e| e.id == Some(id))
        .ok_or_else(|| CommandError::Other(format!("review entry not found: {id}")))?;

    // 2. Stale check: reject if source file no longer exists
    if !entry.source_path.exists() {
        state.db.update_review_status(id, ReviewStatus::Rejected)?;
        return Err(CommandError::Other(
            "source file no longer exists -- entry rejected as stale".into(),
        ));
    }

    // 3. Build rename plan (video + subtitles)
    let mut plan_entries = vec![mediarr_core::RenamePlanEntry {
        source_path: entry.source_path.clone(),
        dest_path: entry.proposed_path.clone(),
    }];

    // Parse subtitle entries from JSON and add to plan
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

    // 4. Execute rename
    // Review entries don't store which watcher queued them, so use
    // global config for rename operations. Per-watcher settings apply
    // only during auto-mode processing within the watcher thread.
    let renamer = mediarr_core::Renamer::from_config(&state.config.general);
    let results = renamer.execute(&plan);
    let all_success = results.iter().all(|r| r.success);

    if !all_success {
        let errors: Vec<String> = results
            .iter()
            .filter(|r| !r.success)
            .filter_map(|r| r.error.clone())
            .collect();
        return Err(CommandError::Other(format!(
            "rename failed: {}",
            errors.join("; ")
        )));
    }

    // 5. Record to history
    let batch_id = mediarr_core::HistoryDb::generate_batch_id();
    let timestamp = chrono::Utc::now().to_rfc3339();

    let media_info: mediarr_core::MediaInfo =
        serde_json::from_str(&entry.media_info_json).unwrap_or_else(|_| {
            mediarr_core::MediaInfo {
                title: String::new(),
                media_type: mediarr_core::MediaType::Movie,
                year: None,
                season: None,
                episodes: vec![],
                resolution: None,
                video_codec: None,
                audio_codec: None,
                source: None,
                release_group: None,
                container: String::new(),
                language: None,
                confidence: mediarr_core::ParseConfidence::High,
            }
        });

    let records: Vec<mediarr_core::RenameRecord> = results
        .iter()
        .filter(|r| r.success)
        .map(|r| {
            let file_size = std::fs::metadata(&r.dest_path)
                .map(|m| m.len())
                .unwrap_or(0);
            let file_mtime = std::fs::metadata(&r.dest_path)
                .and_then(|m| m.modified())
                .ok()
                .map(|t| {
                    let dt: chrono::DateTime<chrono::Utc> = t.into();
                    dt.to_rfc3339()
                })
                .unwrap_or_default();

            mediarr_core::RenameRecord {
                batch_id: batch_id.clone(),
                timestamp: timestamp.clone(),
                source_path: r.source_path.clone(),
                dest_path: r.dest_path.clone(),
                media_info: media_info.clone(),
                file_size,
                file_mtime,
            }
        })
        .collect();

    state.db.record_batch(&records)?;

    // 6. Update review status
    state
        .db
        .update_review_status(id, ReviewStatus::Approved)?;

    Ok(())
}
