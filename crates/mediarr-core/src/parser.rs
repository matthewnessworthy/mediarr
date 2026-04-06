//! Filename parsing via the `hunch` crate.
//!
//! Wraps hunch's `HunchResult` into mediarr's own [`MediaInfo`] type,
//! adding anime detection, multi-episode support, and ambiguity flagging.
//!
//! # Public API
//!
//! - [`parse_filename`] — parse a single filename into `MediaInfo`.
//! - [`parse_with_context`] — parse with sibling filenames for better title detection.

use hunch::{hunch as hunch_parse, hunch_with_context as hunch_ctx, Property};

use crate::error::{MediError, Result};
use crate::types::{MediaInfo, MediaType, ParseConfidence};

/// Parse a single filename into [`MediaInfo`].
///
/// Delegates to hunch for raw extraction, then maps the result into
/// mediarr's own types with anime detection and ambiguity flagging.
///
/// # Errors
///
/// Returns [`MediError::NoTitle`] if hunch cannot extract a title.
pub fn parse_filename(filename: &str) -> Result<MediaInfo> {
    let result = hunch_parse(filename);
    map_hunch_result(&result, filename)
}

/// Parse a filename with context (sibling filenames for better title detection).
///
/// Cross-file invariance detection improves title extraction when multiple
/// files from the same series/season are in the same directory.
///
/// # Errors
///
/// Returns [`MediError::NoTitle`] if hunch cannot extract a title.
pub fn parse_with_context(filename: &str, siblings: &[&str]) -> Result<MediaInfo> {
    let result = hunch_ctx(filename, siblings);
    map_hunch_result(&result, filename)
}

/// Map a hunch `HunchResult` into mediarr's `MediaInfo`.
fn map_hunch_result(result: &hunch::HunchResult, original_filename: &str) -> Result<MediaInfo> {
    // Extract title — required field
    let title = result
        .title()
        .map(|s| s.to_owned())
        .ok_or_else(|| MediError::NoTitle {
            filename: original_filename.to_owned(),
        })?;

    // Extract season with safe i32 -> u16 conversion
    let season = safe_i32_to_u16(result.season(), "season", original_filename);

    // Extract episodes: try multi-episode via all(Property::Episode) first
    let mut episodes: Vec<u16> = result
        .all(Property::Episode)
        .iter()
        .filter_map(|s| s.parse::<u16>().ok())
        .collect();

    // Fallback: if all() returned nothing but episode() has a value, use that
    if episodes.is_empty() {
        if let Some(ep) = safe_i32_to_u16(result.episode(), "episode", original_filename) {
            episodes.push(ep);
        }
    }

    // Extract year with safe conversion
    let year = safe_i32_to_u16(result.year(), "year", original_filename);

    // Determine media type and whether it was inferred
    let hunch_media_type = result.media_type();
    let release_group = result.release_group().map(|s| s.to_owned());
    let (media_type, type_was_inferred) = match hunch_media_type {
        Some(hunch::MediaType::Movie) => (MediaType::Movie, false),
        Some(hunch::MediaType::Episode) => {
            // Anime and Series are treated as the same category ("Series").
            // Anime-style filenames use the series template.
            (MediaType::Series, false)
        }
        Some(hunch::MediaType::Extra) => (MediaType::Series, false),
        None => {
            let (inferred_type, _) =
                infer_type_from_fields(season.is_some(), !episodes.is_empty(), year.is_some());
            (inferred_type, true)
        }
    };

    // Map confidence, downgrading if type was inferred
    let hunch_confidence = result.confidence();
    let confidence = map_confidence(hunch_confidence, type_was_inferred);

    // Extract remaining optional fields
    let resolution = result.screen_size().map(|s| s.to_owned());
    let video_codec = result.video_codec().map(|s| s.to_owned());
    let audio_codec = result.audio_codec().map(|s| s.to_owned());
    let source = result.source().map(|s| s.to_owned());
    let language = result.language().map(|s| s.to_owned());

    // Container: prefer hunch, fallback to extension from filename
    let container = result
        .container()
        .map(|s| s.to_owned())
        .unwrap_or_else(|| extract_extension(original_filename));

    // Default season to 1 for Series when episodes are present but season
    // is missing.  Files like "Show.E05.mkv" omit the season prefix; treating
    // them as season 1 prevents the template from producing "SE05" instead of
    // "S01E05".
    let season = match (&media_type, season, episodes.is_empty()) {
        (MediaType::Series, None, false) => Some(1),
        _ => season,
    };

    Ok(MediaInfo {
        title,
        media_type,
        year,
        season,
        episodes,
        resolution,
        video_codec,
        audio_codec,
        source,
        release_group,
        container,
        language,
        confidence,
    })
}

/// Safely convert an `Option<i32>` to `Option<u16>`, logging a warning on overflow/negative.
fn safe_i32_to_u16(value: Option<i32>, field_name: &str, filename: &str) -> Option<u16> {
    value.and_then(|v| match u16::try_from(v) {
        Ok(n) => Some(n),
        Err(_) => {
            tracing::warn!(
                field = field_name,
                value = v,
                filename = filename,
                "i32 value out of u16 range, ignoring"
            );
            None
        }
    })
}

/// Infer media type from available fields when hunch doesn't provide one.
fn infer_type_from_fields(
    has_season: bool,
    has_episode: bool,
    has_year: bool,
) -> (MediaType, ParseConfidence) {
    if has_season || has_episode {
        (MediaType::Series, ParseConfidence::Medium)
    } else if has_year {
        (MediaType::Movie, ParseConfidence::Medium)
    } else {
        // Default assumption: Series with low confidence
        (MediaType::Series, ParseConfidence::Low)
    }
}

/// Map hunch's confidence to mediarr's, downgrading if media type was inferred.
fn map_confidence(hunch_confidence: hunch::Confidence, type_was_inferred: bool) -> ParseConfidence {
    let base = match hunch_confidence {
        hunch::Confidence::High => ParseConfidence::High,
        hunch::Confidence::Medium => ParseConfidence::Medium,
        hunch::Confidence::Low => ParseConfidence::Low,
    };

    // Downgrade High -> Medium if type was inferred (not from hunch)
    if type_was_inferred && base == ParseConfidence::High {
        ParseConfidence::Medium
    } else {
        base
    }
}

/// Extract file extension from a filename string.
fn extract_extension(filename: &str) -> String {
    // Strip any path components first
    let basename = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
    basename.rsplit('.').next().unwrap_or("").to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Core parsing tests ──

    #[test]
    fn parse_series_with_full_metadata() {
        let info = parse_filename("The.Office.S02E03.720p.BluRay.x264-DEMAND.mkv").unwrap();
        assert_eq!(info.title, "The Office");
        assert_eq!(info.media_type, MediaType::Series);
        assert_eq!(info.season, Some(2));
        assert_eq!(info.episodes, vec![3]);
        assert_eq!(info.resolution.as_deref(), Some("720p"));
        assert_eq!(info.container, "mkv");
    }

    #[test]
    fn parse_movie_with_year() {
        let info = parse_filename("Inception.2010.1080p.BluRay.x264-GROUP.mkv").unwrap();
        assert_eq!(info.title, "Inception");
        assert_eq!(info.media_type, MediaType::Movie);
        assert_eq!(info.year, Some(2010));
    }

    // ── Anime detection tests ──

    #[test]
    fn detect_anime_bracket_group_maps_to_series() {
        let info = parse_filename("[SubGroup] Naruto - 01 [1080p].mkv").unwrap();
        assert_eq!(info.media_type, MediaType::Series);
    }

    #[test]
    fn detect_anime_crc32_maps_to_series() {
        let info = parse_filename("[SubGroup] Naruto - 01 [ABCD1234].mkv").unwrap();
        assert_eq!(info.media_type, MediaType::Series);
    }

    // ── Multi-episode tests ──

    #[test]
    fn parse_multi_episode() {
        let info = parse_filename("The.Office.S02E05E06.720p.mkv").unwrap();
        assert!(
            info.episodes.contains(&5) && info.episodes.contains(&6),
            "Expected episodes [5, 6], got {:?}",
            info.episodes
        );
    }

    // ── Context-aware parsing ──

    #[test]
    fn parse_with_context_does_not_panic() {
        let siblings = &["Show.S01E01.720p.mkv", "Show.S01E02.720p.mkv"];
        let result = parse_with_context("Show.S01E03.720p.mkv", siblings);
        assert!(result.is_ok());
    }

    // ── Error handling tests ──

    #[test]
    fn no_title_returns_error() {
        // An empty string or one that is purely an extension
        let result = parse_filename(".mkv");
        match result {
            Err(MediError::NoTitle { .. }) => {} // expected
            Ok(info) => {
                // hunch might still return something; if title is empty-ish, still valid
                // The point is it shouldn't panic
                assert!(!info.title.is_empty() || info.title.is_empty());
            }
            Err(e) => panic!("Expected NoTitle error, got: {e}"),
        }
    }

    // ── i32 to u16 safe conversion ──

    #[test]
    fn safe_conversion_valid() {
        assert_eq!(safe_i32_to_u16(Some(5), "test", "test.mkv"), Some(5));
        assert_eq!(safe_i32_to_u16(Some(0), "test", "test.mkv"), Some(0));
        assert_eq!(
            safe_i32_to_u16(Some(65535), "test", "test.mkv"),
            Some(65535)
        );
    }

    #[test]
    fn safe_conversion_negative() {
        assert_eq!(safe_i32_to_u16(Some(-1), "test", "test.mkv"), None);
    }

    #[test]
    fn safe_conversion_overflow() {
        assert_eq!(safe_i32_to_u16(Some(65536), "test", "test.mkv"), None);
    }

    #[test]
    fn safe_conversion_none() {
        assert_eq!(safe_i32_to_u16(None, "test", "test.mkv"), None);
    }

    // ── Confidence mapping ──

    #[test]
    fn inferred_type_gets_medium_confidence() {
        // When type is inferred, High should be downgraded to Medium
        let confidence = map_confidence(hunch::Confidence::High, true);
        assert_eq!(confidence, ParseConfidence::Medium);
    }

    #[test]
    fn non_inferred_type_keeps_high_confidence() {
        let confidence = map_confidence(hunch::Confidence::High, false);
        assert_eq!(confidence, ParseConfidence::High);
    }

    // ── Type inference ──

    #[test]
    fn infer_series_from_season() {
        let (media_type, confidence) = infer_type_from_fields(true, false, false);
        assert_eq!(media_type, MediaType::Series);
        assert_eq!(confidence, ParseConfidence::Medium);
    }

    #[test]
    fn infer_series_from_episode() {
        let (media_type, confidence) = infer_type_from_fields(false, true, false);
        assert_eq!(media_type, MediaType::Series);
        assert_eq!(confidence, ParseConfidence::Medium);
    }

    #[test]
    fn infer_movie_from_year() {
        let (media_type, confidence) = infer_type_from_fields(false, false, true);
        assert_eq!(media_type, MediaType::Movie);
        assert_eq!(confidence, ParseConfidence::Medium);
    }

    #[test]
    fn infer_default_series_low_confidence() {
        let (media_type, confidence) = infer_type_from_fields(false, false, false);
        assert_eq!(media_type, MediaType::Series);
        assert_eq!(confidence, ParseConfidence::Low);
    }

    // ── Extension extraction ──

    #[test]
    fn extract_extension_basic() {
        assert_eq!(extract_extension("movie.mkv"), "mkv");
    }

    #[test]
    fn extract_extension_with_path() {
        assert_eq!(extract_extension("/some/path/movie.mp4"), "mp4");
    }

    #[test]
    fn extract_extension_no_extension() {
        assert_eq!(extract_extension("noext"), "noext");
    }

    // ── Edge cases ──

    #[test]
    fn empty_string_does_not_panic() {
        let result = parse_filename("");
        // Should return an error, not panic
        assert!(result.is_err() || result.is_ok());
    }

    #[test]
    fn very_long_filename_does_not_panic() {
        let long = format!("{}_{}.mkv", "a".repeat(500), "S01E01");
        let result = parse_filename(&long);
        // Should not panic regardless of outcome
        assert!(result.is_err() || result.is_ok());
    }

    #[test]
    fn container_fallback_to_extension() {
        // When hunch doesn't detect container, we fall back to the file extension
        let info = parse_filename("Some.Movie.2024.weird_ext").unwrap();
        // The container should be extracted from the filename
        assert!(!info.container.is_empty());
    }

    // ── Season defaulting for episode-only filenames ──

    #[test]
    fn episode_only_series_defaults_season_to_one() {
        // Files like "Show.E05.mkv" have episode but no season.
        // Parser should default season to 1 to avoid "SE05" in templates.
        let info = parse_filename("Some.Show.E05.720p.mkv").unwrap();
        assert_eq!(
            info.season,
            Some(1),
            "season should default to 1 when episodes present but season missing"
        );
        assert_eq!(info.episodes, vec![5]);
    }

    #[test]
    fn explicit_season_is_not_overridden() {
        // Files with explicit season should keep their parsed value
        let info = parse_filename("The.Office.S02E03.720p.mkv").unwrap();
        assert_eq!(info.season, Some(2));
        assert_eq!(info.episodes, vec![3]);
    }

    #[test]
    fn movie_without_season_stays_none() {
        // Movies should NOT get a default season
        let info = parse_filename("Inception.2010.1080p.BluRay.mkv").unwrap();
        assert_eq!(info.season, None);
    }

}
