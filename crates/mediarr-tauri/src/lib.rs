mod commands;
mod error;
mod state;

use std::collections::HashMap;
use std::sync::Mutex;

use mediarr_core::{config, Config, HistoryDb};
use tracing_subscriber::EnvFilter;

/// Entry point for the Tauri application.
///
/// Initialises logging, loads config and history database, registers plugins,
/// registers all IPC command handlers, and starts the Tauri event loop.
///
/// Startup failures are shown as native error dialogs before exit instead
/// of panicking, so users see a clear explanation of what went wrong.
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config_path = match config::default_config_path() {
        Ok(p) => p,
        Err(e) => {
            show_startup_error(&format!("Could not determine configuration path.\n\n{e}"));
            std::process::exit(1);
        }
    };

    let data_path = match config::default_data_path() {
        Ok(p) => p,
        Err(e) => {
            show_startup_error(&format!("Could not determine data path.\n\n{e}"));
            std::process::exit(1);
        }
    };

    let config = Config::load(&config_path).unwrap_or_default();

    let db = match HistoryDb::open(&data_path) {
        Ok(db) => db,
        Err(e) => {
            show_startup_error(&format!(
                "Failed to open history database at {}.\n\n{e}",
                data_path.display()
            ));
            std::process::exit(1);
        }
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(Mutex::new(state::AppState {
            config,
            db,
            active_watchers: HashMap::new(),
        }))
        .invoke_handler(tauri::generate_handler![
            commands::scan::scan_folder,
            commands::scan::scan_folder_streaming,
            commands::rename::dry_run_renames,
            commands::rename::execute_renames,
            commands::history::list_batches,
            commands::history::check_undo,
            commands::history::execute_undo,
            commands::watcher::list_watchers,
            commands::watcher::list_watcher_events,
            commands::watcher::list_review_queue,
            commands::watcher::update_review_status,
            commands::watcher::start_watcher,
            commands::watcher::stop_watcher,
            commands::watcher::approve_review_entry,
            commands::config::get_config,
            commands::config::update_config,
            commands::config::preview_template,
            commands::config::validate_template,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            show_startup_error(&format!("Tauri application failed to start.\n\n{e}"));
            std::process::exit(1);
        });
}

/// Show a native error dialog for startup failures.
///
/// Uses `rfd` for cross-platform native dialogs that work before
/// the Tauri webview is initialized. Falls back to tracing::error
/// if the dialog fails to display.
fn show_startup_error(message: &str) {
    tracing::error!("{message}");
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("Mediarr — Startup Error")
        .set_description(message)
        .show();
}
