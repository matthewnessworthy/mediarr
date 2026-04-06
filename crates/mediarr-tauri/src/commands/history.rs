use tauri::State;

use mediarr_core::{BatchSummary, RenameRecord, RenameResult, UndoEligibility};

use crate::error::{CommandError, CommandResult};
use crate::state::ManagedState;

/// List rename batches from history, newest first.
#[tauri::command]
pub fn list_batches(
    state: State<'_, ManagedState>,
    limit: Option<u32>,
) -> CommandResult<Vec<BatchSummary>> {
    let state = state.lock().map_err(|_| CommandError::StateLock)?;
    let batches = state.db.list_batches(limit.map(|l| l as usize))?;
    Ok(batches)
}

/// Get all rename records for a specific batch.
#[tauri::command]
pub fn get_batch(
    state: State<'_, ManagedState>,
    batch_id: String,
) -> CommandResult<Vec<RenameRecord>> {
    let state = state.lock().map_err(|_| CommandError::StateLock)?;
    let records = state.db.get_batch(&batch_id)?;
    Ok(records)
}

/// Clear all rename history.
#[tauri::command]
pub fn clear_history(
    state: State<'_, ManagedState>,
) -> CommandResult<usize> {
    let state = state.lock().map_err(|_| CommandError::StateLock)?;
    let deleted = state.db.clear_history()?;
    Ok(deleted)
}

/// Check whether a rename batch is eligible for undo.
#[tauri::command]
pub fn check_undo(
    state: State<'_, ManagedState>,
    batch_id: String,
) -> CommandResult<UndoEligibility> {
    let state = state.lock().map_err(|_| CommandError::StateLock)?;
    let eligibility = state.db.check_undo_eligible(&batch_id)?;
    Ok(eligibility)
}

/// Execute an undo operation, reversing all renames in a batch.
#[tauri::command]
pub fn execute_undo(
    state: State<'_, ManagedState>,
    batch_id: String,
) -> CommandResult<Vec<RenameResult>> {
    let state = state.lock().map_err(|_| CommandError::StateLock)?;
    let results = state.db.execute_undo(&batch_id)?;
    Ok(results)
}
