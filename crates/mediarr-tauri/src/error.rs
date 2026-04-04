use serde::Serialize;

/// IPC-safe error type for Tauri commands.
///
/// Wraps `mediarr_core::MediError` (which does not implement `Serialize`)
/// into a type that can cross the IPC boundary as a serialized string.
#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    /// Propagated error from mediarr-core.
    #[error(transparent)]
    Core(#[from] mediarr_core::MediError),
    /// Mutex/RwLock poisoned or otherwise failed to acquire.
    #[error("state lock failed")]
    StateLock,
    /// Catch-all for ad-hoc error messages.
    #[error("{0}")]
    Other(String),
}

impl Serialize for CommandError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

/// Convenience alias for Tauri command return types.
pub type CommandResult<T> = Result<T, CommandError>;
