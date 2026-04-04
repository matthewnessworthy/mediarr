use std::path::{Path, PathBuf};

use tauri::State;
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
#[tauri::command]
pub fn start_watcher(state: State<'_, ManagedState>, path: String) -> CommandResult<()> {
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

    // Clone what we need for the spawned thread
    let config = state.config.clone();
    let data_path = mediarr_core::config::default_data_path()
        .map_err(|e| CommandError::Other(format!("failed to determine data path: {e}")))?;
    let watch_path = PathBuf::from(&path);
    let mode = watcher_config.mode;
    let debounce = watcher_config.debounce_seconds;

    // Create shutdown channel
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // Spawn dedicated thread with its own tokio runtime
    let thread_name = format!("watcher-{path}");
    let thread_handle = std::thread::Builder::new()
        .name(thread_name.clone())
        .spawn(move || {
            // Open a separate HistoryDb connection for this thread
            let db = match mediarr_core::HistoryDb::open(&data_path) {
                Ok(db) => db,
                Err(e) => {
                    error!(path = %watch_path.display(), "failed to open history db for watcher: {e}");
                    return;
                }
            };

            let watcher = WatcherManager::new(config, db);

            // Create a single-threaded tokio runtime for this thread
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    error!(path = %watch_path.display(), "failed to create tokio runtime for watcher: {e}");
                    return;
                }
            };

            if let Err(e) = rt.block_on(watcher.run(&watch_path, mode, debounce, shutdown_rx)) {
                error!(path = %watch_path.display(), "watcher exited with error: {e}");
            }
        })
        .map_err(|e| CommandError::Other(format!("failed to spawn watcher thread: {e}")))?;

    info!(path = %path, "watcher started");

    state
        .active_watchers
        .insert(path, WatcherHandle { shutdown_tx, thread_handle });

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

    info!(path = %path, "watcher stop signal sent");

    Ok(())
}
