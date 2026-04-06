use std::path::Path;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::State;

use mediarr_core::{ScanResult, Scanner};

use crate::error::{CommandError, CommandResult};
use crate::state::ManagedState;

/// Scan a folder for media files and return all results at once.
#[tauri::command]
pub fn scan_folder(state: State<'_, ManagedState>, path: String) -> CommandResult<Vec<ScanResult>> {
    let state = state.lock().map_err(|_| CommandError::StateLock)?;
    let scanner = Scanner::new(state.config.clone());
    let results = scanner.scan_folder(Path::new(&path))?;
    Ok(results)
}

/// Scan individual files and return all results at once.
///
/// Each path is scanned independently via [`Scanner::scan_file`]. Non-video
/// files and missing paths are silently skipped (an error event would be
/// overkill for drag-and-drop where the user may drop mixed content).
#[tauri::command]
pub fn scan_files(
    state: State<'_, ManagedState>,
    paths: Vec<String>,
) -> CommandResult<Vec<ScanResult>> {
    let state = state.lock().map_err(|_| CommandError::StateLock)?;
    let scanner = Scanner::new(state.config.clone());
    let mut results = Vec::new();
    for p in &paths {
        match scanner.scan_file(Path::new(p)) {
            Ok(r) => results.push(r),
            Err(_) => { /* skip non-video / missing files */ }
        }
    }
    Ok(results)
}

/// Events emitted during a streaming scan.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "event", content = "data")]
#[allow(dead_code)]
pub enum ScanEvent {
    /// Progress update with count of scanned files.
    Progress {
        scanned: usize,
        total_estimate: usize,
    },
    /// A single scan result ready for display.
    Result { scan_result: Box<ScanResult> },
    /// Scan is complete.
    Complete { total: usize },
    /// An error occurred during scanning.
    Error { message: String },
}

/// Scan a folder for media files, streaming results back via a channel.
#[tauri::command]
pub fn scan_folder_streaming(
    state: State<'_, ManagedState>,
    path: String,
    on_event: Channel<ScanEvent>,
) -> CommandResult<()> {
    let state = state.lock().map_err(|_| CommandError::StateLock)?;
    let scanner = Scanner::new(state.config.clone());
    let results = scanner.scan_folder(Path::new(&path))?;
    let total = results.len();
    for (i, result) in results.into_iter().enumerate() {
        on_event
            .send(ScanEvent::Result {
                scan_result: Box::new(result),
            })
            .ok();
        on_event
            .send(ScanEvent::Progress {
                scanned: i + 1,
                total_estimate: total,
            })
            .ok();
    }
    on_event.send(ScanEvent::Complete { total }).ok();
    Ok(())
}