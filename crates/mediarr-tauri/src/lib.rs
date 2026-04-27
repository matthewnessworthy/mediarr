mod commands;
mod error;
mod state;

use std::collections::HashMap;
use std::sync::{Mutex, RwLock};

use mediarr_core::{config, Config, HistoryDb, WatcherManager, WatcherMode};
use tracing::{error, info, warn};
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
        .plugin(tauri_plugin_process::init())
        .manage::<state::ManagedConfig>(RwLock::new(config))
        .manage::<state::ManagedDb>(Mutex::new(db))
        .manage::<state::ManagedWatchers>(Mutex::new(HashMap::new()))
        .invoke_handler(tauri::generate_handler![
            commands::scan::scan_folder,
            commands::scan::scan_folder_streaming,
            commands::scan::scan_files,
            commands::rename::dry_run_renames,
            commands::rename::execute_renames,
            commands::history::list_batches,
            commands::history::get_batch,
            commands::history::check_undo,
            commands::history::execute_undo,
            commands::history::clear_history,
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
            commands::config::preview_proposed_path,
            commands::config::validate_template,
        ])
        .setup(|app| {
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;
            auto_start_watchers(app.handle());
            Ok(())
        })
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            show_startup_error(&format!("Tauri application failed to start.\n\n{e}"));
            std::process::exit(1);
        });
}

/// Build the per-watcher event callback that forwards mediarr-core
/// `WatcherEvent`s to the frontend over Tauri's event bus. Used by both
/// `start_watcher` and `auto_start_watchers`.
pub(crate) fn make_event_callback(
    app: &tauri::AppHandle,
) -> Box<dyn Fn(&mediarr_core::WatcherEvent) + Send> {
    use tauri::Emitter;
    let app_handle = app.clone();
    Box::new(move |event: &mediarr_core::WatcherEvent| {
        let _ = app_handle.emit("watcher-event", event);
    })
}

/// Spawn a watcher on a dedicated OS thread with its own single-threaded tokio runtime.
///
/// Returns the `WatcherHandle` (shutdown channel + thread handle) and an init
/// receiver that resolves once the watcher is running or has failed during setup.
///
/// Both `start_watcher` and `auto_start_watchers` use this helper to avoid
/// duplicating the thread-spawn + DB-open + runtime-creation boilerplate.
pub(crate) fn spawn_watcher_thread(
    config: Config,
    data_path: std::path::PathBuf,
    watch_path: std::path::PathBuf,
    mode: WatcherMode,
    debounce: u64,
    on_event: Box<dyn Fn(&mediarr_core::WatcherEvent) + Send>,
) -> std::result::Result<
    (
        state::WatcherHandle,
        std::sync::mpsc::Receiver<std::result::Result<(), String>>,
    ),
    String,
> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let (init_tx, init_rx) = std::sync::mpsc::sync_channel::<std::result::Result<(), String>>(1);

    let thread_name = format!("watcher-{}", watch_path.display());
    let thread_handle = std::thread::Builder::new()
        .name(thread_name)
        .spawn(move || {
            let db = match mediarr_core::HistoryDb::open(&data_path) {
                Ok(db) => db,
                Err(e) => {
                    let msg = format!("failed to open history db: {e}");
                    error!(path = %watch_path.display(), "{msg}");
                    let _ = init_tx.send(Err(msg));
                    return;
                }
            };

            let mut watcher = WatcherManager::new(config, db);
            watcher.set_on_event(on_event);

            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    let msg = format!("failed to create tokio runtime: {e}");
                    error!(path = %watch_path.display(), "{msg}");
                    let _ = init_tx.send(Err(msg));
                    return;
                }
            };

            if let Err(e) = rt.block_on(watcher.run_with_init_signal(
                &watch_path,
                mode,
                debounce,
                shutdown_rx,
                init_tx,
            )) {
                error!(path = %watch_path.display(), "watcher exited with error: {e}");
            }
        })
        .map_err(|e| format!("failed to spawn watcher thread: {e}"))?;

    Ok((
        state::WatcherHandle {
            shutdown_tx,
            thread_handle,
        },
        init_rx,
    ))
}

/// Auto-start all watchers that have `active: true` in the config.
///
/// Called during Tauri setup. Per-watcher failures are logged and do not
/// prevent other watchers from starting; only setup-level failures (lock
/// poisoning, missing data path) abort the whole pass.
fn auto_start_watchers(app: &tauri::AppHandle) {
    if let Err(e) = auto_start_watchers_impl(app) {
        error!("auto-start watchers aborted: {e}");
    }
}

fn auto_start_watchers_impl(app: &tauri::AppHandle) -> std::result::Result<(), String> {
    use tauri::Manager;

    let config_state: tauri::State<'_, state::ManagedConfig> = app.state();
    let watchers_state: tauri::State<'_, state::ManagedWatchers> = app.state();

    let config = config_state
        .read()
        .map_err(|_| "failed to read config".to_string())?;

    let active_configs: Vec<_> = config
        .watchers
        .iter()
        .filter(|w| w.active)
        .cloned()
        .collect();

    if active_configs.is_empty() {
        return Ok(());
    }

    info!(
        count = active_configs.len(),
        "auto-starting active watchers from config"
    );

    let data_path =
        config::default_data_path().map_err(|e| format!("failed to determine data path: {e}"))?;

    let mut watchers = watchers_state
        .lock()
        .map_err(|_| "failed to lock watchers map".to_string())?;

    for wc in active_configs {
        let path_str = wc.path.to_string_lossy().to_string();

        if watchers.contains_key(&path_str) {
            continue;
        }

        match start_one_watcher(app, &wc, &config, &data_path) {
            Ok(handle) => {
                info!(path = %path_str, "auto-started watcher successfully");
                watchers.insert(path_str, handle);
            }
            Err(e) => {
                warn!(path = %path_str, "auto-start watcher failed: {e}");
            }
        }
    }

    Ok(())
}

/// Spawn one watcher and wait for it to confirm initialization. Returns the
/// handle on success, an error message otherwise.
fn start_one_watcher(
    app: &tauri::AppHandle,
    wc: &mediarr_core::WatcherConfig,
    global: &Config,
    data_path: &std::path::Path,
) -> std::result::Result<state::WatcherHandle, String> {
    let resolved_config = wc.resolve_config(global);

    let (handle, init_rx) = spawn_watcher_thread(
        resolved_config,
        data_path.to_path_buf(),
        wc.path.clone(),
        wc.mode,
        wc.debounce_seconds,
        make_event_callback(app),
    )?;

    match init_rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(Ok(())) => Ok(handle),
        Ok(Err(msg)) => Err(msg),
        Err(e) => Err(format!("init timed out: {e}")),
    }
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
