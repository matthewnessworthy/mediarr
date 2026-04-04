use std::collections::HashMap;
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
        .filter_map(|e| e.media_info.as_ref().map(|mi| (e.source_path.clone(), mi.clone())))
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

            let source_key = r.source_path.to_string_lossy().to_string();
            let info = media_info_map.get(&source_key).cloned().unwrap_or_else(|| {
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

            Some(RenameRecord {
                batch_id: batch_id.clone(),
                timestamp: timestamp.clone(),
                source_path: r.source_path.clone(),
                dest_path: r.dest_path.clone(),
                media_info: info,
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
