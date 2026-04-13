use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;
use tauri::State;

use mediarr_core::{RenamePlan, RenamePlanEntry, RenameResult, Renamer};

use crate::error::{CommandError, CommandResult};
use crate::state::ManagedState;

/// A rename entry received from the frontend.
#[derive(Deserialize)]
pub struct RenameEntry {
    pub source_path: String,
    pub dest_path: String,
    /// Optional media info from scan results for accurate history recording.
    #[serde(default)]
    pub media_info: Option<mediarr_core::MediaInfo>,
}

/// Validate a rename plan without touching the filesystem.
#[tauri::command]
pub fn dry_run_renames(
    state: State<'_, ManagedState>,
    entries: Vec<RenameEntry>,
) -> CommandResult<Vec<RenameResult>> {
    let state = state.lock().map_err(|_| CommandError::StateLock)?;
    let renamer = Renamer::from_config(&state.config.general);
    let plan = RenamePlan {
        entries: entries
            .into_iter()
            .map(|e| RenamePlanEntry {
                source_path: PathBuf::from(e.source_path),
                dest_path: PathBuf::from(e.dest_path),
            })
            .collect(),
    };
    Ok(renamer.dry_run(&plan))
}

/// Execute a rename plan, moving or copying files.
///
/// Records successful renames in the history database for undo support.
/// Uses real `MediaInfo` from scan results when available for accurate history.
#[tauri::command]
pub fn execute_renames(
    state: State<'_, ManagedState>,
    entries: Vec<RenameEntry>,
) -> CommandResult<Vec<RenameResult>> {
    let state = state.lock().map_err(|_| CommandError::StateLock)?;
    let renamer = Renamer::from_config(&state.config.general);

    // Build source_path -> MediaInfo lookup before consuming entries
    let media_info_map: HashMap<String, mediarr_core::MediaInfo> = entries
        .iter()
        .filter_map(|e| {
            e.media_info
                .as_ref()
                .map(|mi| (e.source_path.clone(), mi.clone()))
        })
        .collect();

    let plan = RenamePlan {
        entries: entries
            .into_iter()
            .map(|e| RenamePlanEntry {
                source_path: PathBuf::from(e.source_path),
                dest_path: PathBuf::from(e.dest_path),
            })
            .collect(),
    };
    let results = renamer.execute(&plan);

    // Record successful renames in history
    if let Err(e) = state.db.record_rename_results(&results, &media_info_map) {
        tracing::warn!(error = %e, "failed to record rename batch in history");
    }

    Ok(results)
}
