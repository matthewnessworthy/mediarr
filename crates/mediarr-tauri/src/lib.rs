mod error;
mod state;

use std::sync::Mutex;

use mediarr_core::{config, Config, HistoryDb};
use tracing_subscriber::EnvFilter;

/// Entry point for the Tauri application.
///
/// Initialises logging, loads config and history database, registers plugins,
/// and starts the Tauri event loop.
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config_path = config::default_config_path()
        .expect("could not determine config path");
    let data_path = config::default_data_path()
        .expect("could not determine data path");

    let config = Config::load(&config_path).unwrap_or_default();
    let db = HistoryDb::open(&data_path)
        .expect("failed to open history database");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(Mutex::new(state::AppState { config, db }))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
