//! Watch command implementation.
//!
//! Thin wrapper around `mediarr_core::WatcherManager` that monitors a folder
//! for new media files and streams events to the terminal in real-time.
//! Ctrl+C cleanly shuts down the watcher via a tokio shutdown channel.

use mediarr_core::{Config, HistoryDb, WatcherManager, WatcherMode};

use crate::WatchArgs;

/// Execute the watch command.
pub async fn execute(args: WatchArgs) -> anyhow::Result<()> {
    // Load config
    let config_path = mediarr_core::config::default_config_path()?;
    let config = Config::load(&config_path)?;

    // Determine mode
    let mode = match args.mode.as_deref() {
        Some("review") => WatcherMode::Review,
        Some("auto") | None => WatcherMode::Auto,
        Some(other) => anyhow::bail!("unknown mode: {other}"),
    };

    // Determine debounce
    let debounce = args.debounce.unwrap_or(5);

    // Open history database
    let data_path = mediarr_core::config::default_data_path()?;
    let db = HistoryDb::open(&data_path)?;

    // Create watcher manager
    let watcher = WatcherManager::new(config, db);

    // Create shutdown channel -- Ctrl+C will set this to true
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // Set up Ctrl+C handler to send shutdown signal
    let tx = shutdown_tx.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            eprintln!("\nShutting down watcher...");
            let _ = tx.send(true);
        }
    });

    // Print startup message to stderr
    let path_display = args.path.display();
    eprintln!("Watching {path_display} (mode: {mode}, debounce: {debounce}s)");
    eprintln!("Press Ctrl+C to stop");

    // Run watcher on the current task (WatcherManager is !Send due to rusqlite)
    watcher.run(&args.path, mode, debounce, shutdown_rx).await?;

    eprintln!("Watcher stopped");
    Ok(())
}
