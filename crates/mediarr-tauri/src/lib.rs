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
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config_path = config::default_config_path().expect("could not determine config path");
    let data_path = config::default_data_path().expect("could not determine data path");

    let config = Config::load(&config_path).unwrap_or_default();
    let db = HistoryDb::open(&data_path).expect("failed to open history database");

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
            commands::config::get_config,
            commands::config::update_config,
            commands::config::preview_template,
            commands::config::validate_template,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
