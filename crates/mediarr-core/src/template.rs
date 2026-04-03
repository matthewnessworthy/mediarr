//! Template engine for rendering naming templates into output file paths.
//!
//! Templates use `{variable}` syntax with optional modifiers like `{season:02}`
//! for zero-padding. The engine renders templates against `MediaInfo` data,
//! validates templates per media type, and sanitizes output paths for
//! cross-platform compatibility.

// Implementation will be added in the GREEN phase.

#[cfg(test)]
mod tests {
    use crate::error::MediError;
    use crate::types::{MediaInfo, MediaType, ParseConfidence, TemplateWarning};
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

    fn anime_info() -> MediaInfo {
        MediaInfo {
            title: "Attack on Titan".to_string(),
            media_type: MediaType::Anime,
            year: Some(2013),
            season: Some(1),
            episodes: vec![5],
            resolution: Some("1080p".to_string()),
            video_codec: Some("HEVC".to_string()),
            audio_codec: Some("FLAC".to_string()),
            source: Some("BluRay".to_string()),
            release_group: Some("SubsPlease".to_string()),
            container: "mkv".to_string(),
            language: Some("Japanese".to_string()),
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
        assert_eq!(
            result,
            PathBuf::from("The Office - S02E03.mkv")
        );
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
            PathBuf::from(
                "Inception (2010) [1080p] [x264] [DTS] [BluRay] - FGT.mkv"
            )
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
        let result = engine
            .render("{title}.{resolution}.{ext}", &info)
            .unwrap();
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
        let result = engine
            .render("{title}.{episode}.{ext}", &info)
            .unwrap();
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
        let expected =
            PathBuf::from("Inception (2010)").join("Inception (2010).mkv");
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
        let result = engine
            .render("{title}/{title}.{ext}", &info)
            .unwrap();
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
        let warnings =
            engine.validate("{title} - E{episode:02}.{ext}", &MediaType::Series);
        assert!(
            warnings.iter().any(|w| w.variable == "season"),
            "expected warning about missing season"
        );
    }

    #[test]
    fn validate_series_template_missing_episode() {
        let engine = super::TemplateEngine::new();
        let warnings =
            engine.validate("{title} - S{season:02}.{ext}", &MediaType::Series);
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
        assert!(warnings.is_empty(), "expected no warnings, got: {:?}", warnings);
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
        let warnings =
            engine.validate("{title} ({year}).{ext}", &MediaType::Movie);
        assert!(warnings.is_empty(), "expected no warnings, got: {:?}", warnings);
    }

    // -----------------------------------------------------------------------
    // Validation: anime
    // -----------------------------------------------------------------------

    #[test]
    fn validate_anime_template_missing_season_and_episode() {
        let engine = super::TemplateEngine::new();
        let warnings = engine.validate("{title}.{ext}", &MediaType::Anime);
        assert!(
            warnings.iter().any(|w| w.variable == "season"),
            "expected warning about missing season"
        );
        assert!(
            warnings.iter().any(|w| w.variable == "episode"),
            "expected warning about missing episode"
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
        let warnings =
            engine.validate("{title}.{unknown_thing}.{ext}", &MediaType::Movie);
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
        assert!(result.is_ok(), "render should succeed despite validation warnings");
    }

    // -----------------------------------------------------------------------
    // Default template patterns from decisions D-01, D-02, D-03
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
        let expected =
            PathBuf::from("Inception (2010)").join("Inception (2010).mkv");
        assert_eq!(result, expected);
    }

    #[test]
    fn render_default_anime_template() {
        let engine = super::TemplateEngine::new();
        let info = anime_info();
        // D-03 default: "{title}/Season {season:02}/{title} - S{season:02}E{episode:02}.{ext}"
        let result = engine
            .render(
                "{title}/Season {season:02}/{title} - S{season:02}E{episode:02}.{ext}",
                &info,
            )
            .unwrap();
        let expected = PathBuf::from("Attack on Titan")
            .join("Season 01")
            .join("Attack on Titan - S01E05.mkv");
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
        let result = engine
            .render("E{episode}.{ext}", &info)
            .unwrap();
        assert_eq!(result, PathBuf::from("E3.mkv"));
    }
}
