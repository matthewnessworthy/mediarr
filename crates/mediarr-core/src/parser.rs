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
use crate::types::{FolderContext, MediaInfo, MediaType, ParseConfidence};

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
    match basename.rfind('.') {
        Some(pos) => basename[pos + 1..].to_lowercase(),
        None => String::new(),
    }
}

/// Merge parent and grandparent folder metadata into a single best-of view.
///
/// Field-by-field resolution order (addresses review concern about underspecification):
/// - **title**: Parent title wins UNLESS parent title matches season-only pattern
///   (case-insensitive `^season\s+\d+$`), in which case grandparent title is used.
///   This handles Plex convention: `Show Name/Season 02/file.mkv`.
/// - **season**: Parent season always wins (closer to file = more relevant).
///   If parent has no season, grandparent season is used.
/// - **year**: Parent year wins. If parent has no year, grandparent year is used.
/// - **media_type**: Parent type wins (it reflects the folder closer to the file).
/// - **episodes**: Parent episodes win (rare in folder names, but parent takes precedence).
/// - **confidence**: Parent confidence wins (closer folder = more reliable context).
/// - **All other fields** (resolution, codec, source, release_group, language, container):
///   Parent wins; grandparent fills None gaps only.
fn merge_folder_levels(
    parent: &Option<MediaInfo>,
    grandparent: &Option<MediaInfo>,
) -> Option<MediaInfo> {
    match (parent, grandparent) {
        (None, None) => None,
        (Some(p), None) => Some(p.clone()),
        (None, Some(g)) => Some(g.clone()),
        (Some(p), Some(g)) => {
            let mut merged = p.clone();

            // Title: discard parent title if it looks like a season-only folder name
            // e.g., "Season 02", "season 5" -> use grandparent title instead.
            // Also use grandparent title if parent title is empty.
            if !g.title.is_empty()
                && (is_season_only_title(&merged.title) || merged.title.is_empty())
            {
                merged.title = g.title.clone();
            }

            // Season: parent wins (already in merged). Grandparent fills gap only.
            if merged.season.is_none() {
                merged.season = g.season;
            }

            // Year: parent wins. Grandparent fills gap only.
            if merged.year.is_none() {
                merged.year = g.year;
            }

            // Other optional fields: parent wins, grandparent fills None gaps
            if merged.resolution.is_none() {
                merged.resolution = g.resolution.clone();
            }
            if merged.video_codec.is_none() {
                merged.video_codec = g.video_codec.clone();
            }
            if merged.audio_codec.is_none() {
                merged.audio_codec = g.audio_codec.clone();
            }
            if merged.source.is_none() {
                merged.source = g.source.clone();
            }
            if merged.release_group.is_none() {
                merged.release_group = g.release_group.clone();
            }
            if merged.language.is_none() {
                merged.language = g.language.clone();
            }

            Some(merged)
        }
    }
}

/// Check if a folder title is just a season indicator like "Season 02" or "season 5".
/// Used to skip bad titles from Plex-convention season folders.
/// Does NOT use regex crate -- simple string operations only.
fn is_season_only_title(title: &str) -> bool {
    let lower = title.to_lowercase();
    if let Some(rest) = lower.strip_prefix("season") {
        let trimmed = rest.trim();
        !trimmed.is_empty() && trimmed.chars().all(|c| c.is_ascii_digit())
    } else {
        false
    }
}

/// Merge folder-level metadata into a file-level parse result.
///
/// Implements decisions D-01 through D-08 from Phase 11 CONTEXT:
/// - D-01: Confidence-based resolution, file wins on tie
/// - D-02: Folder context fills gaps only (secondary unless strictly higher confidence)
/// - D-05/D-06: Movie->Series promotion when folder has season
/// - D-07/D-08: Ambiguity flagging for season-inherited-episode-missing
///
/// Returns the merged MediaInfo and an optional ambiguity reason string.
/// When folder merge produces an ambiguity reason, it should take priority over
/// generic confidence-based ambiguity in the scanner (addresses review concern
/// about double ambiguity flagging).
pub fn merge_folder_context(
    mut file_info: MediaInfo,
    ctx: &FolderContext,
) -> (MediaInfo, Option<String>) {
    let mut reasons: Vec<String> = Vec::new();

    // Combine parent + grandparent into single best-of folder view
    let folder = merge_folder_levels(&ctx.parent, &ctx.grandparent);
    let Some(folder) = folder else {
        return (file_info, None); // No folder context available
    };

    // D-05/D-06: Media type promotion (Movie -> Series when folder has season)
    // Only promote when folder has season metadata (D-06: title-only folder does NOT promote)
    if file_info.media_type == MediaType::Movie && folder.season.is_some() {
        file_info.media_type = MediaType::Series;
        file_info.confidence = ParseConfidence::Medium;
        reasons
            .push("media type promoted from Movie to Series (folder has season metadata)".into());
    }

    // D-01/D-02: Season -- gap-fill or confidence-based override
    if file_info.season.is_none() {
        if let Some(s) = folder.season {
            file_info.season = Some(s);
            reasons.push(format!("season {} inherited from folder", s));
        }
    } else if let Some(fs) = folder.season {
        // File already has season -- only override if folder confidence is STRICTLY higher (D-01)
        if folder.confidence.is_higher_than(&file_info.confidence) {
            file_info.season = Some(fs);
            reasons.push(format!(
                "season overridden to {} from folder (higher confidence)",
                fs
            ));
        }
        // Equal confidence: file wins (D-01 tiebreaker) -- no change
    }

    // D-01/D-02: Title gap-fill
    // Only replace if file title is empty (gap fill). Non-empty file title always wins
    // at equal or lower folder confidence.
    if file_info.title.is_empty() {
        if !folder.title.is_empty() {
            file_info.title = folder.title.clone();
        }
    } else if !folder.title.is_empty() && folder.confidence.is_higher_than(&file_info.confidence) {
        file_info.title = folder.title.clone();
    }

    // D-01/D-02: Year gap-fill or confidence-based override
    if file_info.year.is_none() {
        if let Some(y) = folder.year {
            file_info.year = Some(y);
        }
    } else if folder.year.is_some() && folder.confidence.is_higher_than(&file_info.confidence) {
        file_info.year = folder.year;
    }

    // D-07/D-08: Missing episode after season inheritance
    // Flag when we have a series with season but no episodes, AND we inherited something
    if file_info.season.is_some()
        && file_info.episodes.is_empty()
        && file_info.media_type == MediaType::Series
        && !reasons.is_empty()
    {
        // Only add this if we actually inherited season or promoted type
        if reasons
            .iter()
            .any(|r| r.contains("season") || r.contains("promoted"))
        {
            reasons.push("season inherited from folder, episode missing".into());
        }
    }

    let ambiguity = if reasons.is_empty() {
        None
    } else {
        Some(reasons.join("; "))
    };
    (file_info, ambiguity)
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
                panic!(
                    "Expected NoTitle error for bare extension, but got Ok with title {:?}",
                    info.title
                );
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
        assert_eq!(extract_extension("noext"), "");
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

    // -----------------------------------------------------------------------
    // merge_folder_levels tests
    // -----------------------------------------------------------------------

    #[test]
    fn merge_folder_levels_both_none_returns_none() {
        let result = merge_folder_levels(&None, &None);
        assert!(result.is_none());
    }

    #[test]
    fn merge_folder_levels_parent_only() {
        let parent = MediaInfo {
            title: "Show".into(),
            season: Some(2),
            ..Default::default()
        };
        let result = merge_folder_levels(&Some(parent), &None).unwrap();
        assert_eq!(result.title, "Show");
        assert_eq!(result.season, Some(2));
    }

    #[test]
    fn merge_folder_levels_grandparent_only() {
        let gp = MediaInfo {
            title: "Show".into(),
            year: Some(2020),
            ..Default::default()
        };
        let result = merge_folder_levels(&None, &Some(gp)).unwrap();
        assert_eq!(result.title, "Show");
        assert_eq!(result.year, Some(2020));
    }

    #[test]
    fn merge_folder_levels_season_only_parent_uses_grandparent_title() {
        // Plex convention: "Game of Thrones/Season 02/file.mkv"
        let parent = MediaInfo {
            title: "Season 02".into(),
            season: Some(2),
            ..Default::default()
        };
        let gp = MediaInfo {
            title: "Game of Thrones".into(),
            year: Some(2011),
            ..Default::default()
        };
        let result = merge_folder_levels(&Some(parent), &Some(gp)).unwrap();
        assert_eq!(result.title, "Game of Thrones"); // grandparent title
        assert_eq!(result.season, Some(2)); // parent season
        assert_eq!(result.year, Some(2011)); // grandparent year (parent had None)
    }

    #[test]
    fn merge_folder_levels_parent_season_wins_over_grandparent() {
        // Conflicting seasons: parent=2, grandparent=5 -> parent wins
        let parent = MediaInfo {
            title: "Fire Country".into(),
            season: Some(2),
            ..Default::default()
        };
        let gp = MediaInfo {
            title: "Shows".into(),
            season: Some(5),
            ..Default::default()
        };
        let result = merge_folder_levels(&Some(parent), &Some(gp)).unwrap();
        assert_eq!(result.season, Some(2)); // parent season wins
        assert_eq!(result.title, "Fire Country"); // parent title wins (not season-only)
    }

    #[test]
    fn merge_folder_levels_parent_year_wins_over_grandparent() {
        let parent = MediaInfo {
            title: "Show".into(),
            year: Some(2024),
            ..Default::default()
        };
        let gp = MediaInfo {
            title: "Collection".into(),
            year: Some(2020),
            ..Default::default()
        };
        let result = merge_folder_levels(&Some(parent), &Some(gp)).unwrap();
        assert_eq!(result.year, Some(2024)); // parent year wins
    }

    #[test]
    fn is_season_only_title_detects_patterns() {
        assert!(is_season_only_title("Season 02"));
        assert!(is_season_only_title("season 5"));
        assert!(is_season_only_title("Season  3")); // extra space
        assert!(!is_season_only_title("Fire Country S2"));
        assert!(!is_season_only_title("Breaking Bad"));
        assert!(!is_season_only_title("Season")); // no number
        assert!(!is_season_only_title("")); // empty
    }

    // -----------------------------------------------------------------------
    // merge_folder_context tests
    // -----------------------------------------------------------------------

    #[test]
    fn merge_empty_folder_context_returns_unchanged() {
        let file = MediaInfo {
            title: "Test".into(),
            ..Default::default()
        };
        let ctx = FolderContext::default();
        let (result, ambiguity) = merge_folder_context(file, &ctx);
        assert_eq!(result.title, "Test");
        assert!(ambiguity.is_none());
    }

    #[test]
    fn merge_fills_season_gap_from_folder() {
        // D-02: file has no season, folder has season=2 -> inherited
        let file = MediaInfo {
            title: "Fire Country".into(),
            media_type: MediaType::Series,
            episodes: vec![1],
            ..Default::default()
        };
        let folder_parent = MediaInfo {
            title: "Fire Country".into(),
            season: Some(2),
            confidence: ParseConfidence::Medium,
            ..Default::default()
        };
        let ctx = FolderContext {
            parent: Some(folder_parent),
            grandparent: None,
        };
        let (result, ambiguity) = merge_folder_context(file, &ctx);
        assert_eq!(result.season, Some(2));
        assert!(ambiguity
            .as_ref()
            .unwrap()
            .contains("season 2 inherited from folder"));
    }

    #[test]
    fn merge_file_wins_tiebreaker_on_equal_confidence() {
        // D-01: file Medium + folder Medium -> file wins
        let file = MediaInfo {
            title: "Show".into(),
            season: Some(3),
            confidence: ParseConfidence::Medium,
            ..Default::default()
        };
        let folder_parent = MediaInfo {
            season: Some(2),
            confidence: ParseConfidence::Medium,
            ..Default::default()
        };
        let ctx = FolderContext {
            parent: Some(folder_parent),
            grandparent: None,
        };
        let (result, _) = merge_folder_context(file, &ctx);
        assert_eq!(result.season, Some(3)); // file season kept
    }

    #[test]
    fn merge_folder_wins_on_higher_confidence() {
        // D-01: file Medium + folder High -> folder wins
        let file = MediaInfo {
            title: "Show".into(),
            season: Some(3),
            confidence: ParseConfidence::Medium,
            ..Default::default()
        };
        let folder_parent = MediaInfo {
            season: Some(2),
            confidence: ParseConfidence::High,
            ..Default::default()
        };
        let ctx = FolderContext {
            parent: Some(folder_parent),
            grandparent: None,
        };
        let (result, ambiguity) = merge_folder_context(file, &ctx);
        assert_eq!(result.season, Some(2)); // folder season wins
        assert!(ambiguity.as_ref().unwrap().contains("overridden"));
    }

    #[test]
    fn merge_movie_promoted_to_series_when_folder_has_season() {
        // D-05: Movie + folder.season -> Series, flagged ambiguous
        let file = MediaInfo {
            title: "Fire Country".into(),
            media_type: MediaType::Movie,
            confidence: ParseConfidence::Medium,
            ..Default::default()
        };
        let folder_parent = MediaInfo {
            title: "Fire Country S2".into(),
            season: Some(2),
            confidence: ParseConfidence::Medium,
            ..Default::default()
        };
        let ctx = FolderContext {
            parent: Some(folder_parent),
            grandparent: None,
        };
        let (result, ambiguity) = merge_folder_context(file, &ctx);
        assert_eq!(result.media_type, MediaType::Series);
        assert_eq!(result.season, Some(2));
        assert!(ambiguity.as_ref().unwrap().contains("promoted"));
        assert!(ambiguity.as_ref().unwrap().contains("Movie to Series"));
    }

    #[test]
    fn merge_no_promotion_without_season() {
        // D-06: folder has title only (no season) -> no promotion
        let file = MediaInfo {
            title: "Breaking Bad".into(),
            media_type: MediaType::Movie,
            confidence: ParseConfidence::Low,
            ..Default::default()
        };
        let folder_parent = MediaInfo {
            title: "Breaking Bad".into(),
            confidence: ParseConfidence::Low,
            ..Default::default()
        };
        let ctx = FolderContext {
            parent: Some(folder_parent),
            grandparent: None,
        };
        let (result, ambiguity) = merge_folder_context(file, &ctx);
        assert_eq!(result.media_type, MediaType::Movie); // stays Movie
        assert!(ambiguity.is_none());
    }

    #[test]
    fn merge_flags_missing_episode_after_season_inheritance() {
        // D-07/D-08: season inherited, no episode -> specific ambiguity reason
        let file = MediaInfo {
            title: "Fire Country".into(),
            media_type: MediaType::Movie, // will be promoted
            episodes: vec![],
            confidence: ParseConfidence::Low,
            ..Default::default()
        };
        let folder_parent = MediaInfo {
            title: "Fire Country".into(),
            season: Some(2),
            confidence: ParseConfidence::Medium,
            ..Default::default()
        };
        let ctx = FolderContext {
            parent: Some(folder_parent),
            grandparent: None,
        };
        let (result, ambiguity) = merge_folder_context(file, &ctx);
        assert_eq!(result.media_type, MediaType::Series); // promoted
        let reason = ambiguity.unwrap();
        assert!(
            reason.contains("season inherited from folder, episode missing"),
            "got: {reason}"
        );
    }

    #[test]
    fn merge_grandparent_title_used_for_season_folder() {
        // D-03/D-04: Plex convention with two-level context
        let file = MediaInfo {
            title: "Game of Thrones".into(),
            episodes: vec![5],
            media_type: MediaType::Series,
            confidence: ParseConfidence::High,
            ..Default::default()
        };
        let parent = MediaInfo {
            title: "Season 02".into(),
            season: Some(2),
            confidence: ParseConfidence::Medium,
            ..Default::default()
        };
        let grandparent = MediaInfo {
            title: "Game of Thrones".into(),
            year: Some(2011),
            confidence: ParseConfidence::Low,
            ..Default::default()
        };
        let ctx = FolderContext {
            parent: Some(parent),
            grandparent: Some(grandparent),
        };
        let (result, _) = merge_folder_context(file, &ctx);
        // File already had season=None, so folder fills gap
        assert_eq!(result.season, Some(2));
        // Title stays from file (High confidence file title beats folder)
        assert_eq!(result.title, "Game of Thrones");
    }

    #[test]
    fn merge_year_gap_filled_from_folder() {
        // Year gap-fill: file has no year, folder has year -> inherited
        let file = MediaInfo {
            title: "Hostage".into(),
            media_type: MediaType::Movie,
            confidence: ParseConfidence::Medium,
            ..Default::default()
        };
        let folder_parent = MediaInfo {
            title: "Hostage".into(),
            year: Some(2020),
            confidence: ParseConfidence::Medium,
            ..Default::default()
        };
        let ctx = FolderContext {
            parent: Some(folder_parent),
            grandparent: None,
        };
        let (result, _) = merge_folder_context(file, &ctx);
        assert_eq!(result.year, Some(2020)); // year inherited
    }

    #[test]
    fn merge_title_gap_filled_when_file_title_empty() {
        let file = MediaInfo {
            title: String::new(),
            confidence: ParseConfidence::Low,
            ..Default::default()
        };
        let folder_parent = MediaInfo {
            title: "My Show".into(),
            confidence: ParseConfidence::Medium,
            ..Default::default()
        };
        let ctx = FolderContext {
            parent: Some(folder_parent),
            grandparent: None,
        };
        let (result, _) = merge_folder_context(file, &ctx);
        assert_eq!(result.title, "My Show");
    }
}
