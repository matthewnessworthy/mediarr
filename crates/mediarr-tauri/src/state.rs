use std::collections::HashMap;
use std::sync::Mutex;
use std::thread::JoinHandle;

use mediarr_core::{Config, HistoryDb};

/// Handle for a running watcher, used to stop it.
///
/// Each watcher runs on a dedicated OS thread with its own single-threaded
/// tokio runtime (because `WatcherManager` contains `HistoryDb` which holds
/// a `rusqlite::Connection` that is `!Send`).
pub struct WatcherHandle {
    /// Send `true` to shut down the watcher event loop.
    pub shutdown_tx: tokio::sync::watch::Sender<bool>,
    /// Thread handle (watcher runs on a dedicated thread with its own tokio runtime).
    /// Retained for graceful shutdown (join) in future.
    #[allow(dead_code)]
    pub thread_handle: JoinHandle<()>,
}

/// Shared application state managed by Tauri.
///
/// Wrapped in a `Mutex` and registered via `tauri::Builder::manage()`.
/// Tauri commands access it through `tauri::State<ManagedState>`.
pub struct AppState {
    /// Current application configuration.
    pub config: Config,
    /// SQLite rename history database.
    pub db: HistoryDb,
    /// Currently running watchers, keyed by watch path string.
    pub active_watchers: HashMap<String, WatcherHandle>,
}

/// Type alias for the Tauri-managed state.
pub type ManagedState = Mutex<AppState>;
