use tauri::State;

use mediarr_core::{BatchSummary, RenameRecord, RenameResult, UndoEligibility};

use crate::error::{CommandError, CommandResult};
use crate::state::ManagedDb;

/// List rename batches from history, newest first.
#[tauri::command]
pub fn list_batches(
    db: State<'_, ManagedDb>,
    limit: Option<u32>,
) -> CommandResult<Vec<BatchSummary>> {
    let db = db.lock().map_err(|_| CommandError::StateLock)?;
    let batches = db.list_batches(limit.map(|l| l as usize))?;
    Ok(batches)
}

/// Get all rename records for a specific batch.
#[tauri::command]
pub fn get_batch(db: State<'_, ManagedDb>, batch_id: String) -> CommandResult<Vec<RenameRecord>> {
    let db = db.lock().map_err(|_| CommandError::StateLock)?;
    let records = db.get_batch(&batch_id)?;
    Ok(records)
}

/// Clear all rename history.
#[tauri::command]
pub fn clear_history(db: State<'_, ManagedDb>) -> CommandResult<usize> {
    let db = db.lock().map_err(|_| CommandError::StateLock)?;
    let deleted = db.clear_history()?;
    Ok(deleted)
}

/// Check whether a rename batch is eligible for undo.
#[tauri::command]
pub fn check_undo(db: State<'_, ManagedDb>, batch_id: String) -> CommandResult<UndoEligibility> {
    let db = db.lock().map_err(|_| CommandError::StateLock)?;
    let eligibility = db.check_undo_eligible(&batch_id)?;
    Ok(eligibility)
}

/// Execute an undo operation, reversing all renames in a batch.
#[tauri::command]
pub fn execute_undo(
    db: State<'_, ManagedDb>,
    batch_id: String,
) -> CommandResult<Vec<RenameResult>> {
    let db = db.lock().map_err(|_| CommandError::StateLock)?;
    let results = db.execute_undo(&batch_id)?;
    Ok(results)
}
