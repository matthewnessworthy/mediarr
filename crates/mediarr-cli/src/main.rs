use std::path::PathBuf;
use std::process;

use clap::{ArgAction, Parser, Subcommand};
use tracing_subscriber::EnvFilter;

mod commands;
mod output;

/// Rename and organise media files.
#[derive(Parser)]
#[command(
    name = "mediarr",
    about = "Rename and organise media files",
    version,
    arg_required_else_help = true
)]
struct Cli {
    /// Increase log verbosity (-v = info, -vv = debug, -vvv = trace)
    #[arg(short, long, action = ArgAction::Count, global = true)]
    verbose: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan a folder for media files and show rename proposals
    Scan(ScanArgs),
    /// Rename media files according to naming templates
    Rename(RenameArgs),
    /// Show rename history
    History(HistoryArgs),
    /// Undo a previous rename batch
    Undo(UndoArgs),
    /// Watch a folder for new media files
    Watch(WatchArgs),
    /// View or modify configuration
    Config(ConfigArgs),
    /// Review queued rename proposals from watch mode
    Review(ReviewArgs),
}

/// Arguments for the scan command.
#[derive(Parser)]
pub struct ScanArgs {
    /// Path to scan for media files
    pub path: PathBuf,
    /// Scan subdirectories recursively
    #[arg(short, long, default_value_t = true)]
    pub recursive: bool,
    /// Filter by media type
    #[arg(short = 't', long = "type", value_parser = ["series", "movie", "anime"])]
    pub media_type: Option<String>,
    /// Preview mode (same as scan, included for consistency)
    #[arg(long)]
    pub dry_run: bool,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
    /// Show verbose tree view with subtitle details
    #[arg(long = "tree")]
    pub tree: bool,
}

/// Arguments for the rename command.
#[derive(Parser)]
pub struct RenameArgs {
    /// Path to scan and rename media files
    pub path: PathBuf,
    /// Scan subdirectories recursively
    #[arg(short, long, default_value_t = true)]
    pub recursive: bool,
    /// Skip confirmation prompt
    #[arg(short, long)]
    pub yes: bool,
    /// Show rename plan without executing
    #[arg(long)]
    pub dry_run: bool,
}

/// Arguments for the history command.
#[derive(Parser)]
pub struct HistoryArgs {
    /// Maximum number of batches to show
    #[arg(short, long)]
    pub limit: Option<usize>,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

/// Arguments for the undo command.
#[derive(Parser)]
pub struct UndoArgs {
    /// Batch ID to undo (from history output)
    pub batch_id: String,
}

/// Arguments for the watch command.
#[derive(Parser)]
pub struct WatchArgs {
    /// Path to watch for new media files
    pub path: PathBuf,
    /// Watch mode
    #[arg(short, long, value_parser = ["auto", "review"])]
    pub mode: Option<String>,
    /// Debounce duration in seconds
    #[arg(short, long)]
    pub debounce: Option<u64>,
}

/// Arguments for the config command.
#[derive(Parser)]
pub struct ConfigArgs {
    /// Get a config value by key
    #[arg(long)]
    pub get: Option<String>,
    /// Set a config value (key value)
    #[arg(long, num_args = 2)]
    pub set: Option<Vec<String>>,
}

/// Arguments for the review command.
#[derive(Parser)]
pub struct ReviewArgs {
    /// Approve all pending review items
    #[arg(long)]
    pub approve_all: bool,
    /// Reject all pending review items
    #[arg(long)]
    pub reject_all: bool,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Initialize tracing to stderr (stdout is reserved for user output)
    let default_level = match cli.verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_level));

    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(filter)
        .init();

    let result = match cli.command {
        Commands::Scan(args) => commands::scan::execute(args).await,
        Commands::Rename(args) => commands::rename::execute(args).await,
        Commands::History(args) => commands::history::execute(args),
        Commands::Undo(args) => commands::undo::execute(args),
        Commands::Watch(args) => commands::watch::execute(args).await,
        Commands::Config(args) => commands::config::execute(args),
        Commands::Review(args) => commands::review::execute(args),
    };

    if let Err(e) = result {
        eprintln!("Error: {e:#}");
        process::exit(1);
    }
}
