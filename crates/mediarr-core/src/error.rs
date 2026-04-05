use std::path::PathBuf;
use thiserror::Error;

/// Central error type for all mediarr-core operations.
#[derive(Debug, Error)]
pub enum MediError {
    // -- Parse errors --
    /// Filename parsing failed entirely.
    #[error("failed to parse filename: {0}")]
    ParseFailed(String),

    /// Parser ran but could not extract a title from the filename.
    #[error("no title extracted from filename: {filename}")]
    NoTitle {
        /// The filename that was parsed.
        filename: String,
    },

    // -- Template errors --
    /// Template string has invalid syntax.
    #[error("invalid template: {0}")]
    InvalidTemplate(String),

    /// Template references a variable name that does not exist.
    #[error("unknown template variable: {name}")]
    UnknownVariable {
        /// The unrecognised variable name.
        name: String,
    },

    /// Template format modifier (e.g. `:02`) is not supported.
    #[error("invalid format modifier: {modifier}")]
    InvalidModifier {
        /// The modifier string that was rejected.
        modifier: String,
    },

    // -- Scan errors --
    /// The scan target path does not exist on disk.
    #[error("scan path does not exist: {}", path.display())]
    ScanPathNotFound {
        /// The missing path.
        path: PathBuf,
    },

    /// The scan target path exists but is not a directory.
    #[error("scan path is not a directory: {}", path.display())]
    ScanPathNotDirectory {
        /// The non-directory path.
        path: PathBuf,
    },

    // -- Rename errors --
    /// A rename (move) operation failed.
    #[error("rename failed: {} -> {}: {cause}", from.display(), to.display())]
    RenameFailed {
        /// Source file path.
        from: PathBuf,
        /// Destination file path.
        to: PathBuf,
        /// Underlying I/O cause.
        #[source]
        cause: std::io::Error,
    },

    /// Cross-filesystem copy succeeded but file sizes do not match.
    #[error("copy verification failed: {} -> {} (size mismatch)", from.display(), to.display())]
    CopyVerificationFailed {
        /// Source file path.
        from: PathBuf,
        /// Destination file path.
        to: PathBuf,
    },

    /// Target path already exists and conflict strategy prevents overwriting.
    #[error("conflict: target already exists: {}", path.display())]
    ConflictExists {
        /// The conflicting target path.
        path: PathBuf,
    },

    /// All numeric suffixes (1-99) exhausted for conflict resolution.
    #[error("conflict resolution exhausted: all suffixes (1)--(99) exist for {}", path.display())]
    ConflictResolutionExhausted {
        /// The base destination path that has all suffixes taken.
        path: PathBuf,
    },

    // -- Path encoding errors --
    /// Path contains non-UTF-8 bytes and cannot be stored reliably.
    /// We error rather than silently losing data via `to_string_lossy()`.
    #[error("path contains non-UTF-8 bytes, cannot store: {}", path.display())]
    NonUtf8Path {
        /// The non-UTF-8 path.
        path: PathBuf,
    },

    // -- History errors --
    /// SQLite history database error.
    #[error("history database error: {0}")]
    HistoryDb(#[from] rusqlite::Error),

    /// Undo operation is not eligible for the given batch.
    #[error("undo not eligible for batch {batch_id}: {reason}")]
    UndoNotEligible {
        /// The batch that cannot be undone.
        batch_id: String,
        /// Why the undo is ineligible.
        reason: String,
    },

    // -- Config errors --
    /// TOML config file could not be parsed.
    #[error("config parse error: {0}")]
    ConfigParse(#[from] toml::de::Error),

    /// TOML config could not be serialized.
    #[error("config serialize error: {0}")]
    ConfigSerialize(#[from] toml::ser::Error),

    /// Platform config directory could not be determined (e.g. `dirs::config_dir()` returned None).
    #[error("config path not available: platform config directory could not be determined")]
    ConfigPathUnavailable,

    // -- I/O errors --
    /// Generic I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    // -- JSON serialization --
    /// JSON serialization/deserialization error (used for history media_info storage).
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    // -- Subtitle errors --
    /// Subtitle discovery or processing error.
    #[error("subtitle discovery error: {0}")]
    SubtitleDiscovery(String),

    // -- Watcher errors --
    /// Filesystem watcher error.
    #[error("watcher error: {0}")]
    Watcher(String),
}

/// Convenience result alias for mediarr-core operations.
pub type Result<T> = std::result::Result<T, MediError>;
