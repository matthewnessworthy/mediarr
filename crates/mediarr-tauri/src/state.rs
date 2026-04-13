use std::collections::HashMap;
use std::sync::{Mutex, RwLock};
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

/// Application configuration, behind an `RwLock` for concurrent read access.
///
/// Config reads (scan, dry-run, list watchers) only need a shared read lock,
/// so they no longer block each other. Only mutations (update_config,
/// start/stop_watcher) acquire a write lock.
pub type ManagedConfig = RwLock<Config>;

/// SQLite rename history database, behind a `Mutex`.
///
/// `rusqlite::Connection` is `!Sync`, so a `Mutex` is required.
pub type ManagedDb = Mutex<HistoryDb>;

/// Currently running watchers, keyed by watch path string.
pub type ManagedWatchers = Mutex<HashMap<String, WatcherHandle>>;
