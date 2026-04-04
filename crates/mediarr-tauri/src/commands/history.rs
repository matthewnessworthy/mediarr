use tauri::State;

use mediarr_core::{BatchSummary, RenameResult, UndoEligibility};

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
