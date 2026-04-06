//! Template engine for rendering naming templates into output file paths.
//!
//! Templates use `{variable}` syntax with optional modifiers like `{season:02}`
//! for zero-padding. The engine renders templates against `MediaInfo` data,
//! validates templates per media type, and sanitizes output paths for
//! cross-platform compatibility.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::error::{MediError, Result};
use crate::types::{MediaInfo, MediaType, TemplateWarning};

/// Regex for matching `{variable}` or `{variable:modifier}` placeholders.
static TEMPLATE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{(\w+)(?::(\w+))?\}").expect("template regex is valid"));

/// Known template variable names.
const KNOWN_VARIABLES: &[&str] = &[
    "title",
    "Title",
    "year",
    "season",
    "episode",
    "ext",
    "resolution",
    "video_codec",
    "audio_codec",
    "source",
    "release_group",
    "language",
];

/// Stateless template engine for rendering naming templates into output paths.
///
/// Templates use `{variable}` syntax with optional modifiers:
/// - `{title}` — raw value
/// - `{season:02}` — zero-padded to 2 digits
/// - `{episode:02}` — zero-padded; multi-episode produces `05E06E07`
pub struct TemplateEngine;

impl TemplateEngine {
    /// Create a new template engine instance.
    pub fn new() -> Self {
        Self
    }

    /// Render a template string against parsed media info, producing an output path.
    ///
    /// Template variables are replaced with values from `info`. Path separators
    /// (`/`) split the template into path components, each sanitized independently.
    ///
    /// # Errors
    ///
    /// Returns `MediError::UnknownVariable` if the template references a variable
    /// not in the known set. Returns `MediError::InvalidModifier` if a format
    /// modifier is unrecognised.
    pub fn render(&self, template: &str, info: &MediaInfo) -> Result<PathBuf> {
        let vars = build_vars(info);

        // Process the template, replacing each variable match.
        // Manual scan so we can return errors on unknown variables.
        let mut result = String::new();
        let mut last_end = 0;

        for caps in TEMPLATE_RE.captures_iter(template) {
            let full_match = caps.get(0).expect("capture group 0 always exists");
            let var_name = &caps[1];
            let modifier = caps.get(2).map(|m| m.as_str());

            // Check known variable
            if !KNOWN_VARIABLES.contains(&var_name) {
                return Err(MediError::UnknownVariable {
                    name: var_name.to_string(),
                });
            }

            // Append text before this match
            result.push_str(&template[last_end..full_match.start()]);

            // Resolve value
            let raw_value = if var_name == "episode" {
                resolve_episode(&info.episodes, modifier)?
            } else {
                let val = vars.get(var_name).cloned().unwrap_or_default();
                match modifier {
                    Some(m) => apply_modifier(&val, m)?,
                    None => val,
                }
            };

            result.push_str(&raw_value);
            last_end = full_match.end();
        }

        // Append any trailing text
        result.push_str(&template[last_end..]);

        // Defense-in-depth: reject path traversal BEFORE any sanitization.
        // Check raw rendered output for ".." or "." path components.
        // collapse_dots and sanitize_component would mask these as empty strings,
        // so we must check before those functions run.
        for component in result.split('/') {
            let trimmed = component.trim();
            if trimmed == ".." || trimmed == "." {
                return Err(MediError::InvalidTemplate(
                    "template produces path traversal component ('.' or '..')".to_string(),
                ));
            }
        }

        // Post-process: collapse consecutive dots
        let result = collapse_dots(&result);

        // Split on '/' into path components, sanitize each, filter empties
        let components: Vec<String> = result
            .split('/')
            .map(sanitize_component)
            .filter(|c| !c.is_empty())
            .collect();

        // Build PathBuf from components
        let mut path = PathBuf::new();
        for component in &components {
            path.push(component);
        }

        Ok(path)
    }

    /// Validate a template string for the given media type.
    ///
    /// Returns warnings about missing recommended variables. Does NOT check
    /// for unknown variables (that is `render`'s responsibility).
    ///
    /// Warnings do not block rendering — they are advisory.
    pub fn validate(&self, template: &str, media_type: &MediaType) -> Vec<TemplateWarning> {
        let present: Vec<String> = TEMPLATE_RE
            .captures_iter(template)
            .map(|c| c[1].to_string())
            .collect();

        let mut warnings = Vec::new();

        // Required variables per media type
        let required: Vec<(&str, &str)> = match media_type {
            MediaType::Movie => vec![
                ("title", "Movie templates should include {title}"),
                ("year", "Movie templates should include {year}"),
                ("ext", "Templates should include {ext} for file extension"),
            ],
            MediaType::Series => vec![
                ("title", "Series templates should include {title}"),
                ("season", "Series templates should include {season}"),
                ("episode", "Series templates should include {episode}"),
                ("ext", "Templates should include {ext} for file extension"),
            ],
        };

        for (var, msg) in required {
            let satisfied = if var == "title" {
                present.iter().any(|p| p == "title" || p == "Title")
            } else {
                present.iter().any(|p| p == var)
            };
            if !satisfied {
                warnings.push(TemplateWarning {
                    variable: var.to_string(),
                    message: msg.to_string(),
                });
            }
        }

        warnings
    }
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a string to Title Case (first letter of each word uppercase, rest lowercase).
fn to_title_case(s: &str) -> String {
    s.split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    first.to_uppercase().to_string() + &chars.as_str().to_lowercase()
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Build a variable lookup map from MediaInfo (excludes episode — handled separately).
fn build_vars(info: &MediaInfo) -> HashMap<String, String> {
    let mut vars = HashMap::new();

    vars.insert("title".to_string(), info.title.clone());
    vars.insert("Title".to_string(), to_title_case(&info.title));
    vars.insert(
        "year".to_string(),
        info.year.map(|y| y.to_string()).unwrap_or_default(),
    );
    vars.insert(
        "season".to_string(),
        info.season.map(|s| s.to_string()).unwrap_or_default(),
    );
    vars.insert("ext".to_string(), info.container.clone());
    vars.insert(
        "resolution".to_string(),
        info.resolution.clone().unwrap_or_default(),
    );
    vars.insert(
        "video_codec".to_string(),
        info.video_codec.clone().unwrap_or_default(),
    );
    vars.insert(
        "audio_codec".to_string(),
        info.audio_codec.clone().unwrap_or_default(),
    );
    vars.insert(
        "source".to_string(),
        info.source.clone().unwrap_or_default(),
    );
    vars.insert(
        "release_group".to_string(),
        info.release_group.clone().unwrap_or_default(),
    );
    vars.insert(
        "language".to_string(),
        info.language.clone().unwrap_or_default(),
    );

    vars
}

/// Resolve the `{episode}` variable with special multi-episode handling.
///
/// - Empty: returns `""`
/// - Single: formatted with modifier (e.g., `"03"` with `:02`)
/// - Multi: first bare, subsequent E-prefixed (e.g., `"05E06"` for [5,6])
fn resolve_episode(episodes: &[u16], modifier: Option<&str>) -> Result<String> {
    if episodes.is_empty() {
        return Ok(String::new());
    }

    let format_ep = |ep: u16| -> Result<String> {
        let raw = ep.to_string();
        match modifier {
            Some(m) => apply_modifier(&raw, m),
            None => Ok(raw),
        }
    };

    let mut result = format_ep(episodes[0])?;
    for &ep in &episodes[1..] {
        result.push('E');
        result.push_str(&format_ep(ep)?);
    }

    Ok(result)
}

/// Apply a format modifier to a value string.
///
/// Currently supports zero-padding modifiers like `02`, `03` etc.
/// The modifier must start with `0` followed by a width digit.
fn apply_modifier(value: &str, modifier: &str) -> Result<String> {
    // Zero-padding modifier: "02", "03", etc.
    if let Some(width_str) = modifier.strip_prefix('0') {
        let width: usize = width_str.parse().map_err(|_| MediError::InvalidModifier {
            modifier: modifier.to_string(),
        })?;

        // Try to parse value as number for zero-padding
        if let Ok(num) = value.parse::<u64>() {
            return Ok(format!("{:0>width$}", num, width = width));
        }
        // Non-numeric value with zero-pad modifier: just return as-is
        return Ok(value.to_string());
    }

    Err(MediError::InvalidModifier {
        modifier: modifier.to_string(),
    })
}

/// Collapse consecutive dots into single dots, and strip leading/trailing dots
/// from each path component segment (separated by `/`).
fn collapse_dots(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut last_was_dot = false;

    for ch in input.chars() {
        if ch == '.' {
            if !last_was_dot {
                result.push('.');
            }
            last_was_dot = true;
        } else {
            last_was_dot = false;
            result.push(ch);
        }
    }

    // Strip leading/trailing dots from each path component
    result
        .split('/')
        .map(|component| component.trim_start_matches('.').trim_end_matches('.'))
        .collect::<Vec<_>>()
        .join("/")
}

/// Sanitize a single path component for cross-platform compatibility.
///
/// Uses the `sanitize-filename` crate to handle Windows reserved names
/// and illegal characters.
fn sanitize_component(component: &str) -> String {
    let opts = sanitize_filename::Options {
        truncate: true,
        windows: true,
        replacement: "",
    };
    let sanitized = sanitize_filename::sanitize_with_options(component, opts);
    // Trim trailing dots/spaces (Windows restriction)
    sanitized
        .trim_end_matches('.')
        .trim_end_matches(' ')
        .to_string()
}

#[cfg(test)]
mod tests {
    use crate::error::MediError;
    use crate::types::{MediaInfo, MediaType, ParseConfidence};
    use std::path::PathBuf;

    // -----------------------------------------------------------------------
    // Helper: build a MediaInfo for testing
    // -----------------------------------------------------------------------
    fn movie_info() -> MediaInfo {
        MediaInfo {
            title: "Inception".to_string(),
            media_type: MediaType::Movie,
            year: Some(2010),
            season: None,
            episodes: vec![],
            resolution: Some("1080p".to_string()),
            video_codec: Some("x264".to_string()),
            audio_codec: Some("DTS".to_string()),
            source: Some("BluRay".to_string()),
            release_group: Some("FGT".to_string()),
            container: "mkv".to_string(),
            language: Some("English".to_string()),
            confidence: ParseConfidence::High,
        }
    }

    fn series_info() -> MediaInfo {
        MediaInfo {
            title: "The Office".to_string(),
            media_type: MediaType::Series,
            year: Some(2005),
            season: Some(2),
            episodes: vec![3],
            resolution: Some("720p".to_string()),
            video_codec: Some("H.264".to_string()),
            audio_codec: Some("AAC".to_string()),
            source: Some("WEB-DL".to_string()),
            release_group: Some("LOL".to_string()),
            container: "mkv".to_string(),
            language: None,
            confidence: ParseConfidence::High,
        }
    }

    fn multi_episode_info() -> MediaInfo {
        MediaInfo {
            title: "Breaking Bad".to_string(),
            media_type: MediaType::Series,
            year: Some(2008),
            season: Some(2),
            episodes: vec![5, 6],
            resolution: Some("1080p".to_string()),
            video_codec: None,
            audio_codec: None,
            source: None,
            release_group: None,
            container: "mkv".to_string(),
            language: None,
            confidence: ParseConfidence::High,
        }
    }

    fn triple_episode_info() -> MediaInfo {
        MediaInfo {
            title: "Breaking Bad".to_string(),
            media_type: MediaType::Series,
            year: Some(2008),
            season: Some(1),
            episodes: vec![1, 2, 3],
            resolution: None,
            video_codec: None,
            audio_codec: None,
            source: None,
            release_group: None,
            container: "mkv".to_string(),
            language: None,
            confidence: ParseConfidence::Medium,
        }
    }

    // -----------------------------------------------------------------------
    // Rendering: single value variables
    // -----------------------------------------------------------------------

    #[test]
    fn render_simple_title() {
        let engine = super::TemplateEngine::new();
        let info = movie_info();
        let result = engine.render("{title}.{ext}", &info).unwrap();
        assert_eq!(result, PathBuf::from("Inception.mkv"));
    }

    #[test]
    fn render_movie_template_with_year() {
        let engine = super::TemplateEngine::new();
        let info = movie_info();
        let result = engine
            .render("{title} ({year})/{title} ({year}).{ext}", &info)
            .unwrap();
        assert_eq!(
            result,
            PathBuf::from("Inception (2010)").join("Inception (2010).mkv")
        );
    }

    #[test]
    fn render_series_template_with_zero_padding() {
        let engine = super::TemplateEngine::new();
        let info = series_info();
        let result = engine
            .render("{title} - S{season:02}E{episode:02}.{ext}", &info)
            .unwrap();
        assert_eq!(result, PathBuf::from("The Office - S02E03.mkv"));
    }

    #[test]
    fn render_all_optional_fields() {
        let engine = super::TemplateEngine::new();
        let info = movie_info();
        let result = engine
            .render(
                "{title} ({year}) [{resolution}] [{video_codec}] [{audio_codec}] [{source}] - {release_group}.{ext}",
                &info,
            )
            .unwrap();
        assert_eq!(
            result,
            PathBuf::from("Inception (2010) [1080p] [x264] [DTS] [BluRay] - FGT.mkv")
        );
    }

    #[test]
    fn render_language_field() {
        let engine = super::TemplateEngine::new();
        let info = movie_info();
        let result = engine.render("{title}.{language}.{ext}", &info).unwrap();
        assert_eq!(result, PathBuf::from("Inception.English.mkv"));
    }

    #[test]
    fn render_ext_without_modifier() {
        let engine = super::TemplateEngine::new();
        let info = movie_info();
        let result = engine.render("{title}.{ext}", &info).unwrap();
        assert_eq!(result, PathBuf::from("Inception.mkv"));
    }

    // -----------------------------------------------------------------------
    // Rendering: empty optional fields / dot collapse
    // -----------------------------------------------------------------------

    #[test]
    fn render_empty_year_no_double_dots() {
        let engine = super::TemplateEngine::new();
        let mut info = movie_info();
        info.year = None;
        // "{title}.{year}.{ext}" with no year should collapse to "Inception.mkv"
        let result = engine.render("{title}.{year}.{ext}", &info).unwrap();
        assert_eq!(result, PathBuf::from("Inception.mkv"));
    }

    #[test]
    fn render_empty_language_dot_collapse() {
        let engine = super::TemplateEngine::new();
        let mut info = movie_info();
        info.language = None;
        let result = engine.render("{title}.{language}.{ext}", &info).unwrap();
        assert_eq!(result, PathBuf::from("Inception.mkv"));
    }

    #[test]
    fn render_subtitle_type_empty_collapse() {
        // Simulates "name.{type}.srt" where type is empty -> "name.srt"
        let engine = super::TemplateEngine::new();
        let mut info = series_info();
        info.resolution = None;
        // Use resolution as a stand-in for testing empty collapse
        let result = engine.render("{title}.{resolution}.{ext}", &info).unwrap();
        assert_eq!(result, PathBuf::from("The Office.mkv"));
    }

    // -----------------------------------------------------------------------
    // Rendering: multi-episode (CRITICAL)
    // -----------------------------------------------------------------------

    #[test]
    fn render_multi_episode_two() {
        let engine = super::TemplateEngine::new();
        let info = multi_episode_info();
        let result = engine
            .render("S{season:02}E{episode:02}.{ext}", &info)
            .unwrap();
        // episodes=[5,6] -> "S02E05E06.mkv"
        assert_eq!(result, PathBuf::from("S02E05E06.mkv"));
    }

    #[test]
    fn render_multi_episode_three() {
        let engine = super::TemplateEngine::new();
        let info = triple_episode_info();
        let result = engine
            .render("S{season:02}E{episode:02}.{ext}", &info)
            .unwrap();
        // episodes=[1,2,3] -> "S01E01E02E03.mkv"
        assert_eq!(result, PathBuf::from("S01E01E02E03.mkv"));
    }

    #[test]
    fn render_single_episode() {
        let engine = super::TemplateEngine::new();
        let info = series_info();
        let result = engine
            .render("S{season:02}E{episode:02}.{ext}", &info)
            .unwrap();
        // episodes=[3] -> "S02E03.mkv"
        assert_eq!(result, PathBuf::from("S02E03.mkv"));
    }

    #[test]
    fn render_empty_episodes() {
        let engine = super::TemplateEngine::new();
        let mut info = movie_info();
        info.episodes = vec![];
        // episode with no episodes -> empty string
        let result = engine.render("{title}.{episode}.{ext}", &info).unwrap();
        // should collapse dots: "Inception.mkv"
        assert_eq!(result, PathBuf::from("Inception.mkv"));
    }

    // -----------------------------------------------------------------------
    // Rendering: zero-padding modifier
    // -----------------------------------------------------------------------

    #[test]
    fn render_season_zero_padded() {
        let engine = super::TemplateEngine::new();
        let info = series_info();
        let result = engine.render("S{season:02}", &info).unwrap();
        assert_eq!(result, PathBuf::from("S02"));
    }

    #[test]
    fn render_season_three_digit_padding() {
        let engine = super::TemplateEngine::new();
        let mut info = series_info();
        info.season = Some(7);
        let result = engine.render("S{season:03}", &info).unwrap();
        assert_eq!(result, PathBuf::from("S007"));
    }

    #[test]
    fn render_year_no_padding_needed() {
        let engine = super::TemplateEngine::new();
        let info = movie_info();
        let result = engine.render("{year}", &info).unwrap();
        assert_eq!(result, PathBuf::from("2010"));
    }

    // -----------------------------------------------------------------------
    // Rendering: path components
    // -----------------------------------------------------------------------

    #[test]
    fn render_path_with_folder_separator() {
        let engine = super::TemplateEngine::new();
        let info = series_info();
        let result = engine
            .render(
                "{title}/Season {season:02}/{title} - S{season:02}E{episode:02}.{ext}",
                &info,
            )
            .unwrap();
        let expected = PathBuf::from("The Office")
            .join("Season 02")
            .join("The Office - S02E03.mkv");
        assert_eq!(result, expected);
    }

    #[test]
    fn render_movie_path_with_folder() {
        let engine = super::TemplateEngine::new();
        let info = movie_info();
        let result = engine
            .render("{title} ({year})/{title} ({year}).{ext}", &info)
            .unwrap();
        let expected = PathBuf::from("Inception (2010)").join("Inception (2010).mkv");
        assert_eq!(result, expected);
    }

    // -----------------------------------------------------------------------
    // Rendering: sanitization
    // -----------------------------------------------------------------------

    #[test]
    fn render_sanitizes_illegal_chars() {
        let engine = super::TemplateEngine::new();
        let mut info = movie_info();
        info.title = "What: The <Movie>?".to_string();
        let result = engine.render("{title}.{ext}", &info).unwrap();
        let rendered = result.to_string_lossy();
        // Illegal chars should be removed
        assert!(!rendered.contains(':'));
        assert!(!rendered.contains('<'));
        assert!(!rendered.contains('>'));
        assert!(!rendered.contains('?'));
    }

    #[test]
    fn render_sanitizes_windows_reserved_names() {
        let engine = super::TemplateEngine::new();
        let mut info = movie_info();
        info.title = "CON".to_string();
        let result = engine.render("{title}.{ext}", &info).unwrap();
        let rendered = result.to_string_lossy();
        // The raw filename should NOT be exactly "CON.mkv"
        // sanitize-filename will modify reserved names
        assert_ne!(rendered, "CON.mkv");
    }

    #[test]
    fn render_strips_trailing_dots_from_components() {
        let engine = super::TemplateEngine::new();
        let mut info = movie_info();
        info.title = "Title.".to_string();
        info.year = None;
        let result = engine.render("{title}/{title}.{ext}", &info).unwrap();
        // Trailing dots should be stripped from path components
        let rendered = result.to_string_lossy();
        assert!(!rendered.starts_with("Title./") && !rendered.starts_with("Title.\\"));
    }

    // -----------------------------------------------------------------------
    // Error: unknown variable
    // -----------------------------------------------------------------------

    #[test]
    fn render_unknown_variable_returns_error() {
        let engine = super::TemplateEngine::new();
        let info = movie_info();
        let result = engine.render("{title}.{foo}.{ext}", &info);
        assert!(result.is_err());
        match result.unwrap_err() {
            MediError::UnknownVariable { name } => {
                assert_eq!(name, "foo");
            }
            other => panic!("expected UnknownVariable, got: {:?}", other),
        }
    }

    #[test]
    fn render_multiple_unknown_variables_returns_first_error() {
        let engine = super::TemplateEngine::new();
        let info = movie_info();
        let result = engine.render("{bar}.{baz}", &info);
        assert!(result.is_err());
        match result.unwrap_err() {
            MediError::UnknownVariable { .. } => {}
            other => panic!("expected UnknownVariable, got: {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Error: invalid modifier
    // -----------------------------------------------------------------------

    #[test]
    fn render_invalid_modifier_returns_error() {
        let engine = super::TemplateEngine::new();
        let info = series_info();
        let result = engine.render("{season:abc}", &info);
        assert!(result.is_err());
        match result.unwrap_err() {
            MediError::InvalidModifier { modifier } => {
                assert_eq!(modifier, "abc");
            }
            other => panic!("expected InvalidModifier, got: {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Validation: series
    // -----------------------------------------------------------------------

    #[test]
    fn validate_series_template_missing_season() {
        let engine = super::TemplateEngine::new();
        let warnings = engine.validate("{title} - E{episode:02}.{ext}", &MediaType::Series);
        assert!(
            warnings.iter().any(|w| w.variable == "season"),
            "expected warning about missing season"
        );
    }

    #[test]
    fn validate_series_template_missing_episode() {
        let engine = super::TemplateEngine::new();
        let warnings = engine.validate("{title} - S{season:02}.{ext}", &MediaType::Series);
        assert!(
            warnings.iter().any(|w| w.variable == "episode"),
            "expected warning about missing episode"
        );
    }

    #[test]
    fn validate_series_complete_no_warnings() {
        let engine = super::TemplateEngine::new();
        let warnings = engine.validate(
            "{title} - S{season:02}E{episode:02}.{ext}",
            &MediaType::Series,
        );
        assert!(
            warnings.is_empty(),
            "expected no warnings, got: {:?}",
            warnings
        );
    }

    // -----------------------------------------------------------------------
    // Validation: movie
    // -----------------------------------------------------------------------

    #[test]
    fn validate_movie_template_missing_year() {
        let engine = super::TemplateEngine::new();
        let warnings = engine.validate("{title}.{ext}", &MediaType::Movie);
        assert!(
            warnings.iter().any(|w| w.variable == "year"),
            "expected warning about missing year"
        );
    }

    #[test]
    fn validate_movie_complete_no_warnings() {
        let engine = super::TemplateEngine::new();
        let warnings = engine.validate("{title} ({year}).{ext}", &MediaType::Movie);
        assert!(
            warnings.is_empty(),
            "expected no warnings, got: {:?}",
            warnings
        );
    }

    // -----------------------------------------------------------------------
    // Validation: missing ext warning
    // -----------------------------------------------------------------------

    #[test]
    fn validate_missing_ext_produces_warning() {
        let engine = super::TemplateEngine::new();
        let warnings = engine.validate("{title} ({year})", &MediaType::Movie);
        assert!(
            warnings.iter().any(|w| w.variable == "ext"),
            "expected warning about missing ext"
        );
    }

    // -----------------------------------------------------------------------
    // Validation does NOT check unknown variables (that's render's job)
    // -----------------------------------------------------------------------

    #[test]
    fn validate_does_not_error_on_unknown_variables() {
        let engine = super::TemplateEngine::new();
        let warnings = engine.validate("{title}.{unknown_thing}.{ext}", &MediaType::Movie);
        // validate returns warnings, not errors. Unknown vars are render's job.
        // Should only warn about missing year for Movie type.
        assert!(
            warnings.iter().any(|w| w.variable == "year"),
            "expected year warning"
        );
        // There should be no panic or error
    }

    // -----------------------------------------------------------------------
    // Validation is independent of rendering
    // -----------------------------------------------------------------------

    #[test]
    fn validate_warnings_do_not_block_render() {
        let engine = super::TemplateEngine::new();
        let info = movie_info();
        // Template missing year for movie -> should warn but render should still work
        let warnings = engine.validate("{title}.{ext}", &MediaType::Movie);
        assert!(!warnings.is_empty(), "expected warnings for missing year");
        // Rendering the same template should succeed
        let result = engine.render("{title}.{ext}", &info);
        assert!(
            result.is_ok(),
            "render should succeed despite validation warnings"
        );
    }

    // -----------------------------------------------------------------------
    // Default template patterns from decisions D-01, D-02
    // -----------------------------------------------------------------------

    #[test]
    fn render_default_series_template() {
        let engine = super::TemplateEngine::new();
        let info = series_info();
        // D-01 default: "{title}/Season {season:02}/{title} - S{season:02}E{episode:02}.{ext}"
        let result = engine
            .render(
                "{title}/Season {season:02}/{title} - S{season:02}E{episode:02}.{ext}",
                &info,
            )
            .unwrap();
        let expected = PathBuf::from("The Office")
            .join("Season 02")
            .join("The Office - S02E03.mkv");
        assert_eq!(result, expected);
    }

    #[test]
    fn render_default_movie_template() {
        let engine = super::TemplateEngine::new();
        let info = movie_info();
        // D-02 default: "{title} ({year})/{title} ({year}).{ext}"
        let result = engine
            .render("{title} ({year})/{title} ({year}).{ext}", &info)
            .unwrap();
        let expected = PathBuf::from("Inception (2010)").join("Inception (2010).mkv");
        assert_eq!(result, expected);
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn render_template_with_no_variables() {
        let engine = super::TemplateEngine::new();
        let info = movie_info();
        let result = engine.render("static-name.mkv", &info).unwrap();
        assert_eq!(result, PathBuf::from("static-name.mkv"));
    }

    #[test]
    fn render_template_adjacent_variables() {
        let engine = super::TemplateEngine::new();
        let info = movie_info();
        let result = engine.render("{title}{year}.{ext}", &info).unwrap();
        assert_eq!(result, PathBuf::from("Inception2010.mkv"));
    }

    #[test]
    fn render_season_none_produces_empty() {
        let engine = super::TemplateEngine::new();
        let mut info = movie_info();
        info.season = None;
        let result = engine.render("{title}.S{season}.{ext}", &info).unwrap();
        // season is None -> "S" then empty, so "Inception.S.mkv"
        // Since there's no dot-collapse issue here (S is not a dot), it stays
        assert_eq!(result, PathBuf::from("Inception.S.mkv"));
    }

    #[test]
    fn render_episode_modifier_without_padding() {
        let engine = super::TemplateEngine::new();
        let info = series_info();
        // No modifier on episode
        let result = engine.render("E{episode}.{ext}", &info).unwrap();
        assert_eq!(result, PathBuf::from("E3.mkv"));
    }

    #[test]
    fn render_rejects_path_traversal() {
        let engine = super::TemplateEngine::new();
        let info = MediaInfo {
            title: "..".to_string(),
            media_type: MediaType::Movie,
            year: Some(2024),
            season: None,
            episodes: vec![],
            resolution: None,
            video_codec: None,
            audio_codec: None,
            source: None,
            release_group: None,
            container: "mkv".to_string(),
            language: None,
            confidence: ParseConfidence::High,
        };
        let result = engine.render("{title}/{title}.{ext}", &info);
        assert!(result.is_err(), "Expected Err but got: {result:?}");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("path traversal"),
            "Error should mention path traversal: {err}"
        );
    }

    // -----------------------------------------------------------------------
    // {Title} variable: Title Case rendering
    // -----------------------------------------------------------------------

    #[test]
    fn render_title_case_lowercase_input() {
        let engine = super::TemplateEngine::new();
        let mut info = series_info();
        info.title = "the office".to_string();
        let result = engine.render("{Title}.{ext}", &info).unwrap();
        assert_eq!(result, PathBuf::from("The Office.mkv"));
    }

    #[test]
    fn render_title_case_uppercase_input() {
        let engine = super::TemplateEngine::new();
        let mut info = movie_info();
        info.title = "INCEPTION".to_string();
        let result = engine.render("{Title}.{ext}", &info).unwrap();
        assert_eq!(result, PathBuf::from("Inception.mkv"));
    }

    #[test]
    fn render_title_case_already_correct() {
        let engine = super::TemplateEngine::new();
        let info = series_info(); // title is "The Office"
        let result = engine.render("{Title}.{ext}", &info).unwrap();
        assert_eq!(result, PathBuf::from("The Office.mkv"));
    }

    #[test]
    fn render_title_lowercase_still_works() {
        // Backward compat: {title} unchanged
        let engine = super::TemplateEngine::new();
        let mut info = movie_info();
        info.title = "the office".to_string();
        let result = engine.render("{title}.{ext}", &info).unwrap();
        assert_eq!(result, PathBuf::from("the office.mkv"));
    }

    #[test]
    fn validate_title_case_satisfies_movie_title_requirement() {
        let engine = super::TemplateEngine::new();
        let warnings = engine.validate("{Title} ({year}).{ext}", &MediaType::Movie);
        assert!(
            !warnings.iter().any(|w| w.variable == "title"),
            "expected no title warning when {{Title}} is used, got: {:?}",
            warnings
        );
    }

    #[test]
    fn validate_title_case_satisfies_series_title_requirement() {
        let engine = super::TemplateEngine::new();
        let warnings = engine.validate(
            "{Title} - S{season:02}E{episode:02}.{ext}",
            &MediaType::Series,
        );
        assert!(
            !warnings.iter().any(|w| w.variable == "title"),
            "expected no title warning when {{Title}} is used, got: {:?}",
            warnings
        );
    }

    #[test]
    fn render_default_movie_template_title_case() {
        let engine = super::TemplateEngine::new();
        let mut info = movie_info();
        info.title = "the dark knight".to_string();
        let result = engine
            .render("{Title} ({year})/{Title} ({year}).{ext}", &info)
            .unwrap();
        let expected = PathBuf::from("The Dark Knight (2010)")
            .join("The Dark Knight (2010).mkv");
        assert_eq!(result, expected);
    }

    #[test]
    fn render_default_series_template_title_case() {
        let engine = super::TemplateEngine::new();
        let mut info = series_info();
        info.title = "the office".to_string();
        let result = engine
            .render("{Title}/{Title} - S{season:02}E{episode:02}.{ext}", &info)
            .unwrap();
        let expected = PathBuf::from("The Office")
            .join("The Office - S02E03.mkv");
        assert_eq!(result, expected);
    }

    #[test]
    fn render_rejects_dot_component() {
        let engine = super::TemplateEngine::new();
        let info = MediaInfo {
            title: ".".to_string(),
            media_type: MediaType::Movie,
            year: Some(2024),
            season: None,
            episodes: vec![],
            resolution: None,
            video_codec: None,
            audio_codec: None,
            source: None,
            release_group: None,
            container: "mkv".to_string(),
            language: None,
            confidence: ParseConfidence::High,
        };
        let result = engine.render("{title}/{title}.{ext}", &info);
        assert!(result.is_err());
    }
}
