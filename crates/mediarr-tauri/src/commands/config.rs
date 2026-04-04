use tauri::State;

use mediarr_core::{config, Config, MediaInfo, MediaType, TemplateEngine, TemplateWarning};

use crate::error::{CommandError, CommandResult};
use crate::state::ManagedState;

/// Get the current application configuration.
#[tauri::command]
pub fn get_config(state: State<'_, ManagedState>) -> CommandResult<Config> {
    let state = state.lock().map_err(|_| CommandError::StateLock)?;
    Ok(state.config.clone())
}

/// Update the application configuration and persist to disk.
#[tauri::command]
pub fn update_config(state: State<'_, ManagedState>, config: Config) -> CommandResult<()> {
    let mut state = state.lock().map_err(|_| CommandError::StateLock)?;
    let config_path = config::default_config_path()?;
    config.save(&config_path)?;
    state.config = config;
    Ok(())
}

/// Preview a naming template by rendering it against sample media info.
#[tauri::command]
pub fn preview_template(template: String, media_info: MediaInfo) -> CommandResult<String> {
    let engine = TemplateEngine::new();
    let path = engine.render(&template, &media_info)?;
    Ok(path.to_string_lossy().into_owned())
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
