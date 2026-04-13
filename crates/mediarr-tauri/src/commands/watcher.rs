use std::path::{Path, PathBuf};

use tauri::State;
use tracing::info;

use mediarr_core::{ReviewQueueEntry, ReviewStatus, WatcherConfig, WatcherEvent};

use crate::error::{CommandError, CommandResult};
use crate::state::ManagedState;

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
/// Spawns a dedicated OS thread via [`crate::spawn_watcher_thread`]. Waits for
/// successful initialization before returning. If setup fails, the error is
/// propagated to the frontend.
#[tauri::command]
pub fn start_watcher(
    app: tauri::AppHandle,
    state: State<'_, ManagedState>,
    path: String,
) -> CommandResult<()> {
    use tauri::Emitter;

    let mut state = state.lock().map_err(|_| CommandError::StateLock)?;

    let watcher_config = state
        .config
        .watchers
        .iter()
        .find(|w| w.path.to_string_lossy() == path)
        .cloned()
        .ok_or_else(|| CommandError::Other(format!("no watcher configured for path: {path}")))?;

    if state.active_watchers.contains_key(&path) {
        return Err(CommandError::Other(format!(
            "watcher already running for path: {path}"
        )));
    }

    let resolved_config = watcher_config.resolve_config(&state.config);
    let data_path = mediarr_core::config::default_data_path()
        .map_err(|e| CommandError::Other(format!("failed to determine data path: {e}")))?;

    let app_handle = app.clone();
    let on_event_callback: Box<dyn Fn(&mediarr_core::WatcherEvent) + Send> =
        Box::new(move |event: &mediarr_core::WatcherEvent| {
            let _ = app_handle.emit("watcher-event", event);
        });

    let (handle, init_rx) = crate::spawn_watcher_thread(
        resolved_config,
        data_path,
        PathBuf::from(&path),
        watcher_config.mode,
        watcher_config.debounce_seconds,
        on_event_callback,
    )
    .map_err(CommandError::Other)?;

    init_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| CommandError::Other(format!("watcher init timed out: {e}")))?
        .map_err(CommandError::Other)?;

    info!(path = %path, "watcher started and confirmed running");

    if let Some(wc) = state
        .config
        .watchers
        .iter_mut()
        .find(|w| w.path.to_string_lossy() == path)
    {
        wc.active = true;
    }
    let config_path = mediarr_core::config::default_config_path()
        .map_err(|e| CommandError::Other(format!("failed to determine config path: {e}")))?;
    state.config.save(&config_path)?;

    state.active_watchers.insert(path, handle);

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
    if let Some(wc) = state
        .config
        .watchers
        .iter_mut()
        .find(|w| w.path.to_string_lossy() == path)
    {
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
/// Thin wrapper around `HistoryDb::execute_review_rename`. Rejects stale entries
/// where the source file no longer exists.
#[tauri::command]
pub fn approve_review_entry(state: State<'_, ManagedState>, id: i64) -> CommandResult<()> {
    let state = state.lock().map_err(|_| CommandError::StateLock)?;

    let entries = state.db.list_review_queue(None, None)?;
    let entry = entries
        .iter()
        .find(|e| e.id == Some(id))
        .ok_or_else(|| CommandError::Other(format!("review entry not found: {id}")))?;

    if !entry.source_path.exists() {
        state.db.update_review_status(id, ReviewStatus::Rejected)?;
        return Err(CommandError::Other(
            "source file no longer exists -- entry rejected as stale".into(),
        ));
    }

    state
        .db
        .execute_review_rename(entry, &state.config.general)?;
    state.db.update_review_status(id, ReviewStatus::Approved)?;

    Ok(())
}
