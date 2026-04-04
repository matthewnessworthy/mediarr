use std::path::PathBuf;

use serde::Deserialize;
use tauri::State;

use mediarr_core::{HistoryDb, RenameRecord, RenamePlan, RenamePlanEntry, Renamer, RenameResult};

use crate::error::{CommandError, CommandResult};
use crate::state::ManagedState;

/// A rename entry received from the frontend.
#[derive(Deserialize)]
pub struct RenameEntry {
    pub source_path: String,
    pub dest_path: String,
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
#[tauri::command]
pub fn execute_renames(
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
    let results = renamer.execute(&plan);

    // Record successful renames in history
    let batch_id = HistoryDb::generate_batch_id();
    let timestamp = chrono::Utc::now().to_rfc3339();
    let records: Vec<RenameRecord> = results
        .iter()
        .filter(|r| r.success)
        .filter_map(|r| {
            let meta = std::fs::metadata(&r.dest_path).ok()?;
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| {
                    let duration = t.duration_since(std::time::UNIX_EPOCH).ok()?;
                    Some(
                        chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0)?
                            .to_rfc3339(),
                    )
                })
                .unwrap_or_default();
            Some(RenameRecord {
                batch_id: batch_id.clone(),
                timestamp: timestamp.clone(),
                source_path: r.source_path.clone(),
                dest_path: r.dest_path.clone(),
                media_info: mediarr_core::MediaInfo {
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
                },
                file_size: meta.len(),
                file_mtime: mtime,
            })
        })
        .collect();

    if !records.is_empty() {
        if let Err(e) = state.db.record_batch(&records) {
            tracing::warn!(error = %e, "failed to record rename batch in history");
        }
    }

    Ok(results)
}
