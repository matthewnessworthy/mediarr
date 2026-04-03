//! Configuration management for Mediarr.
//!
//! Loads, saves, and provides defaults for all application settings in TOML format.
//! Both CLI and GUI share the same `Config` struct and config file.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::error::{MediError, Result};
use crate::types::{ConflictStrategy, DiscoveryToggles, NonPreferredAction, RenameOperation};

/// Top-level application configuration.
///
/// Contains all settings organised into logical groups: general behaviour,
/// naming templates, and subtitle handling. Serialises to/from TOML.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    /// General application settings.
    pub general: GeneralConfig,
    /// Naming template strings per media type.
    pub templates: TemplateConfig,
    /// Subtitle discovery and handling settings.
    pub subtitles: SubtitleConfig,
}

/// General application settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GeneralConfig {
    /// Output directory. `None` means in-place rename (D-13).
    pub output_dir: Option<PathBuf>,
    /// Move or Copy (D-11: default Move).
    pub operation: RenameOperation,
    /// What to do on filename conflict (D-12: default Skip).
    pub conflict_strategy: ConflictStrategy,
    /// Create target directories if they don't exist.
    pub create_directories: bool,
}

/// Naming template strings per media type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TemplateConfig {
    /// Movie naming template (D-01).
    pub movie: String,
    /// Series naming template (D-02).
    pub series: String,
    /// Anime naming template (D-03).
    pub anime: String,
}

/// Subtitle discovery and handling settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubtitleConfig {
    /// Whether subtitle discovery is enabled at all.
    pub enabled: bool,
    /// Template for subtitle output names.
    pub naming_pattern: String,
    /// Which discovery methods are active.
    pub discovery: DiscoveryToggles,
    /// Ordered list of preferred ISO 639-1 language codes (D-05, D-06).
    pub preferred_languages: Vec<String>,
    /// What to do with non-preferred subtitles (SUBT-07).
    pub non_preferred_action: NonPreferredAction,
    /// Backup path for non-preferred subtitles when action = Backup.
    pub backup_path: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// Default implementations
// ---------------------------------------------------------------------------

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            templates: TemplateConfig::default(),
            subtitles: SubtitleConfig::default(),
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            output_dir: None,
            operation: RenameOperation::Move,
            conflict_strategy: ConflictStrategy::Skip,
            create_directories: true,
        }
    }
}

impl Default for TemplateConfig {
    fn default() -> Self {
        Self {
            movie: "{title} ({year})/{title} ({year}).{ext}".to_string(),
            series: "{title}/{title} - S{season:02}E{episode:02}.{ext}".to_string(),
            anime: "{title}/{title} - S{season:02}E{episode:02}.{ext}".to_string(),
        }
    }
}

impl Default for SubtitleConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            naming_pattern: "{video_name}.{lang}.{type}.{ext}".to_string(),
            discovery: DiscoveryToggles::default(),
            preferred_languages: Vec::new(),
            non_preferred_action: NonPreferredAction::Ignore,
            backup_path: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Returns the platform-appropriate path for the Mediarr config file.
///
/// Uses `dirs::config_dir()` to find the platform config directory, then
/// appends `mediarr/config.toml`. Returns `MediError::ConfigPathUnavailable`
/// if the platform directory cannot be determined (never falls back to `.`).
pub fn default_config_path() -> Result<PathBuf> {
    let base = dirs::config_dir().ok_or(MediError::ConfigPathUnavailable)?;
    Ok(base.join("mediarr").join("config.toml"))
}

/// Returns the platform-appropriate path for the Mediarr history database.
///
/// Uses `dirs::data_dir()` to find the platform data directory, then
/// appends `mediarr/history.db`. Returns `MediError::ConfigPathUnavailable`
/// if the platform directory cannot be determined (never falls back to `.`).
pub fn default_data_path() -> Result<PathBuf> {
    let base = dirs::data_dir().ok_or(MediError::ConfigPathUnavailable)?;
    Ok(base.join("mediarr").join("history.db"))
}

// ---------------------------------------------------------------------------
// Config implementation
// ---------------------------------------------------------------------------

impl Config {
    /// Load configuration from a TOML file at the given path.
    ///
    /// If the file does not exist, returns `Config::default()` (no error).
    /// Any other I/O or TOML parse error is propagated.
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(contents) => {
                debug!(path = %path.display(), "loading config from file");
                let config: Config = toml::from_str(&contents)?;
                info!(path = %path.display(), "config loaded successfully");
                Ok(config)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                info!(path = %path.display(), "config file not found, using defaults");
                Ok(Config::default())
            }
            Err(e) => Err(MediError::Io(e)),
        }
    }

    /// Save configuration to a TOML file at the given path.
    ///
    /// Creates parent directories if they do not exist.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(path, contents)?;
        info!(path = %path.display(), "config saved");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // -- Default value tests per D-01 through D-13 --

    #[test]
    fn default_movie_template() {
        let config = Config::default();
        assert_eq!(
            config.templates.movie,
            "{title} ({year})/{title} ({year}).{ext}"
        );
    }

    #[test]
    fn default_series_template() {
        let config = Config::default();
        assert_eq!(
            config.templates.series,
            "{title}/{title} - S{season:02}E{episode:02}.{ext}"
        );
    }

    #[test]
    fn default_anime_template() {
        let config = Config::default();
        assert_eq!(
            config.templates.anime,
            "{title}/{title} - S{season:02}E{episode:02}.{ext}"
        );
    }

    #[test]
    fn default_operation_is_move() {
        let config = Config::default();
        assert_eq!(config.general.operation, RenameOperation::Move);
    }

    #[test]
    fn default_conflict_strategy_is_skip() {
        let config = Config::default();
        assert_eq!(config.general.conflict_strategy, ConflictStrategy::Skip);
    }

    #[test]
    fn default_output_dir_is_none() {
        let config = Config::default();
        assert!(config.general.output_dir.is_none());
    }

    #[test]
    fn default_create_directories_is_true() {
        let config = Config::default();
        assert!(config.general.create_directories);
    }

    #[test]
    fn default_subtitles_enabled() {
        let config = Config::default();
        assert!(config.subtitles.enabled);
    }

    #[test]
    fn default_discovery_toggles_all_true() {
        let config = Config::default();
        let toggles = &config.subtitles.discovery;
        assert!(toggles.sidecar);
        assert!(toggles.subs_subfolder);
        assert!(toggles.nested_language_folders);
        assert!(toggles.vobsub_pairs);
    }

    #[test]
    fn default_preferred_languages_empty() {
        let config = Config::default();
        assert!(config.subtitles.preferred_languages.is_empty());
    }

    #[test]
    fn default_non_preferred_action_is_ignore() {
        let config = Config::default();
        assert_eq!(
            config.subtitles.non_preferred_action,
            NonPreferredAction::Ignore
        );
    }

    // -- TOML round-trip --

    #[test]
    fn toml_round_trip_default_config() {
        let original = Config::default();
        let toml_str = toml::to_string_pretty(&original).expect("serialize");
        let restored: Config = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(original, restored);
    }

    #[test]
    fn toml_round_trip_custom_config() {
        let config = Config {
            general: GeneralConfig {
                output_dir: Some(PathBuf::from("/media/renamed")),
                operation: RenameOperation::Copy,
                conflict_strategy: ConflictStrategy::NumericSuffix,
                create_directories: false,
            },
            templates: TemplateConfig {
                movie: "{title}/{title}.{ext}".to_string(),
                series: "{title}/S{season:02}/{title} - E{episode:02}.{ext}".to_string(),
                anime: "{title}/{title} - {episode:02}.{ext}".to_string(),
            },
            subtitles: SubtitleConfig {
                enabled: false,
                naming_pattern: "{video_name}.{lang}.{ext}".to_string(),
                discovery: DiscoveryToggles {
                    sidecar: true,
                    subs_subfolder: false,
                    nested_language_folders: false,
                    vobsub_pairs: true,
                },
                preferred_languages: vec!["en".to_string(), "ja".to_string()],
                non_preferred_action: NonPreferredAction::Backup,
                backup_path: Some(PathBuf::from("/media/backup/subs")),
            },
        };

        let toml_str = toml::to_string_pretty(&config).expect("serialize");
        let restored: Config = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(config, restored);
    }

    // -- File I/O tests --

    #[test]
    fn load_missing_file_returns_default() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("nonexistent.toml");
        let config = Config::load(&path).expect("should not error on missing file");
        assert_eq!(config, Config::default());
    }

    #[test]
    fn save_creates_file_and_directories() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("nested").join("deep").join("config.toml");
        let config = Config::default();
        config.save(&path).expect("save should succeed");
        assert!(path.exists(), "config file should exist after save");
    }

    #[test]
    fn load_reads_back_saved_config() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("config.toml");

        let config = Config {
            general: GeneralConfig {
                output_dir: Some(PathBuf::from("/tmp/output")),
                ..GeneralConfig::default()
            },
            templates: TemplateConfig {
                movie: "{title}.{ext}".to_string(),
                ..TemplateConfig::default()
            },
            subtitles: SubtitleConfig {
                preferred_languages: vec!["en".to_string(), "fr".to_string()],
                ..SubtitleConfig::default()
            },
        };

        config.save(&path).expect("save");
        let loaded = Config::load(&path).expect("load");
        assert_eq!(config, loaded);
    }

    // -- Path helper tests --

    #[test]
    fn default_config_path_contains_mediarr() {
        // On any platform where dirs::config_dir() works, the path should
        // end with mediarr/config.toml.
        if let Ok(path) = default_config_path() {
            assert!(path.ends_with("mediarr/config.toml") || path.ends_with("mediarr\\config.toml"));
        }
        // If dirs::config_dir() returns None (unlikely on desktop), the
        // function should return ConfigPathUnavailable -- that's tested
        // implicitly by the error variant existing.
    }

    #[test]
    fn default_data_path_contains_mediarr() {
        if let Ok(path) = default_data_path() {
            assert!(path.ends_with("mediarr/history.db") || path.ends_with("mediarr\\history.db"));
        }
    }

    #[test]
    fn load_malformed_toml_returns_parse_error() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("bad.toml");
        std::fs::write(&path, "this is not valid toml [[[").expect("write");
        let result = Config::load(&path);
        assert!(result.is_err());
        match result.unwrap_err() {
            MediError::ConfigParse(_) => {} // expected
            other => panic!("expected ConfigParse, got: {other:?}"),
        }
    }
}
