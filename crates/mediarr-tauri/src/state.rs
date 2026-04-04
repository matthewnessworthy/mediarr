use std::sync::Mutex;

use mediarr_core::{Config, HistoryDb};

/// Shared application state managed by Tauri.
///
/// Wrapped in a `Mutex` and registered via `tauri::Builder::manage()`.
/// Tauri commands access it through `tauri::State<ManagedState>`.
pub struct AppState {
    /// Current application configuration.
    pub config: Config,
    /// SQLite rename history database.
    pub db: HistoryDb,
}

/// Type alias for the Tauri-managed state.
pub type ManagedState = Mutex<AppState>;
