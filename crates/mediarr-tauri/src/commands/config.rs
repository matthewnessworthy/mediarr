use std::path::Path;

use tauri::State;

use mediarr_core::{config, Config, MediaInfo, MediaType, TemplateEngine, TemplateWarning};

use crate::error::{CommandError, CommandResult};
use crate::state::ManagedConfig;

/// Get the current application configuration.
#[tauri::command]
pub fn get_config(config: State<'_, ManagedConfig>) -> CommandResult<Config> {
    let config = config.read().map_err(|_| CommandError::StateLock)?;
    Ok(config.clone())
}

/// Update the application configuration and persist to disk.
#[tauri::command]
pub fn update_config(
    managed_config: State<'_, ManagedConfig>,
    config: Config,
) -> CommandResult<()> {
    let mut current = managed_config
        .write()
        .map_err(|_| CommandError::StateLock)?;
    let config_path = config::default_config_path()?;
    config.save(&config_path)?;
    *current = config;
    Ok(())
}

/// Preview a naming template by rendering it against sample media info.
#[tauri::command]
pub fn preview_template(template: String, media_info: MediaInfo) -> CommandResult<String> {
    let engine = TemplateEngine::new();
    let path = engine.render(&template, &media_info)?;
    Ok(path.to_string_lossy().into_owned())
}

/// Preview a proposed path for a file, applying the same base-directory logic
/// as the scanner: if `output_dir` is configured, prepend it; otherwise prepend
/// the source file's parent directory (in-place rename).
#[tauri::command]
pub fn preview_proposed_path(
    config: State<'_, ManagedConfig>,
    template: String,
    media_info: MediaInfo,
    source_path: String,
) -> CommandResult<String> {
    let config = config.read().map_err(|_| CommandError::StateLock)?;
    let engine = TemplateEngine::new();
    let relative_path = engine.render(&template, &media_info)?;

    let proposed_path = if let Some(ref output_dir) = config.general.output_dir {
        output_dir.join(&relative_path)
    } else {
        Path::new(&source_path)
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(&relative_path)
    };

    Ok(proposed_path.to_string_lossy().into_owned())
}

/// Validate a naming template for a specific media type.
#[tauri::command]
pub fn validate_template(
    template: String,
    media_type: MediaType,
) -> CommandResult<Vec<TemplateWarning>> {
    let engine = TemplateEngine::new();
    Ok(engine.validate(&template, &media_type))
}
