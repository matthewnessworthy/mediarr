use std::path::Path;

use tauri::State;

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

/// Start a folder watcher (placeholder -- full implementation in watcher view plan).
#[tauri::command]
pub fn start_watcher(_state: State<'_, ManagedState>, _path: String) -> CommandResult<()> {
    Err(CommandError::Other("not implemented".into()))
}

/// Stop a folder watcher (placeholder -- full implementation in watcher view plan).
#[tauri::command]
pub fn stop_watcher(_state: State<'_, ManagedState>, _path: String) -> CommandResult<()> {
    Err(CommandError::Other("not implemented".into()))
}
