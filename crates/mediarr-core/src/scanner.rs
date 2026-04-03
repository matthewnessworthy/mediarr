//! Folder scanning orchestration for Mediarr.
//!
//! Ties together parsing, template rendering, subtitle discovery, and conflict
//! detection into a unified scan pipeline. Takes a folder path and produces a
//! complete list of [`ScanResult`]s ready for the renamer.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tracing::{debug, info, warn};
use walkdir::WalkDir;

use crate::config::Config;
use crate::error::{MediError, Result};
use crate::parser;
use crate::subtitle::SubtitleDiscovery;
use crate::template::TemplateEngine;
use crate::types::{MediaType, ParseConfidence, ScanFilter, ScanResult, ScanStatus};

/// Video file extensions recognised by the scanner.
const VIDEO_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "avi", "m4v", "mov", "wmv", "ts", "flv", "webm",
];

/// Orchestrates folder scanning: parse filenames, render templates, discover
/// subtitles, detect conflicts.
pub struct Scanner {
    config: Config,
    template_engine: TemplateEngine,
}

impl Scanner {
    /// Create a new scanner with the given configuration.
    pub fn new(config: Config) -> Self {
        Self {
            config,
            template_engine: TemplateEngine::new(),
        }
    }

    /// Scan a folder recursively and produce scan results for all video files.
    ///
    /// # Errors
    ///
    /// Returns [`MediError::ScanPathNotFound`] if `path` does not exist.
    /// Returns [`MediError::ScanPathNotDirectory`] if `path` is not a directory.
    pub fn scan_folder(&self, path: &Path) -> Result<Vec<ScanResult>> {
        todo!("implement scan_folder")
    }

    /// Filter scan results by the given criteria.
    ///
    /// Returns references to results that match all active filter fields.
    pub fn filter_results<'a>(results: &'a [ScanResult], filter: &ScanFilter) -> Vec<&'a ScanResult> {
        todo!("implement filter_results")
    }

    /// Select the appropriate naming template for the given media type.
    fn select_template(&self, media_type: &MediaType) -> &str {
        match media_type {
            MediaType::Movie => &self.config.templates.movie,
            MediaType::Series => &self.config.templates.series,
            MediaType::Anime => &self.config.templates.anime,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::types::{MediaType, ScanFilter, ScanStatus};
    use std::fs;
    use tempfile::TempDir;

    /// Helper: create a Config with output_dir set to given path.
    fn config_with_output(output_dir: &Path) -> Config {
        let mut config = Config::default();
        config.general.output_dir = Some(output_dir.to_path_buf());
        config
    }

    // -----------------------------------------------------------------------
    // scan_folder: finds video files
    // -----------------------------------------------------------------------

    #[test]
    fn scan_finds_video_files() {
        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();

        fs::write(source.path().join("Movie.One.2020.mkv"), b"video1").unwrap();
        fs::write(source.path().join("Movie.Two.2021.mp4"), b"video2").unwrap();

        let scanner = Scanner::new(config_with_output(output.path()));
        let results = scanner.scan_folder(source.path()).unwrap();
        assert_eq!(results.len(), 2, "should find 2 video files");
    }

    #[test]
    fn scan_skips_non_video_files() {
        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();

        fs::write(source.path().join("Movie.2020.mkv"), b"video").unwrap();
        fs::write(source.path().join("info.txt"), b"text").unwrap();
        fs::write(source.path().join("cover.nfo"), b"nfo").unwrap();
        fs::write(source.path().join("poster.jpg"), b"image").unwrap();

        let scanner = Scanner::new(config_with_output(output.path()));
        let results = scanner.scan_folder(source.path()).unwrap();
        assert_eq!(results.len(), 1, "should only find 1 video file");
    }

    #[test]
    fn scan_finds_files_recursively() {
        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();

        fs::write(source.path().join("Movie.2020.mkv"), b"video1").unwrap();
        let sub = source.path().join("subdir");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("Movie.2021.mp4"), b"video2").unwrap();

        let scanner = Scanner::new(config_with_output(output.path()));
        let results = scanner.scan_folder(source.path()).unwrap();
        assert_eq!(results.len(), 2, "should find files in subdirectories");
    }

    #[test]
    fn scan_result_has_media_info_and_proposed_path() {
        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();

        fs::write(
            source.path().join("Inception.2010.1080p.BluRay.x264-GROUP.mkv"),
            b"video",
        )
        .unwrap();

        let scanner = Scanner::new(config_with_output(output.path()));
        let results = scanner.scan_folder(source.path()).unwrap();
        assert_eq!(results.len(), 1);

        let r = &results[0];
        assert_eq!(r.media_info.title, "Inception");
        assert!(!r.proposed_path.as_os_str().is_empty(), "proposed_path should be set");
    }

    // -----------------------------------------------------------------------
    // Conflict detection
    // -----------------------------------------------------------------------

    #[test]
    fn scan_detects_duplicate_target_conflict() {
        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();

        // Two files in different dirs that will render to the same template output
        // Using identical filenames in different subdirs
        let dir_a = source.path().join("a");
        let dir_b = source.path().join("b");
        fs::create_dir_all(&dir_a).unwrap();
        fs::create_dir_all(&dir_b).unwrap();
        fs::write(
            dir_a.join("Inception.2010.1080p.BluRay.x264-GROUP.mkv"),
            b"vid1",
        )
        .unwrap();
        fs::write(
            dir_b.join("Inception.2010.1080p.BluRay.x264-GROUP.mkv"),
            b"vid2",
        )
        .unwrap();

        let scanner = Scanner::new(config_with_output(output.path()));
        let results = scanner.scan_folder(source.path()).unwrap();

        let conflicts: Vec<_> = results
            .iter()
            .filter(|r| r.status == ScanStatus::Conflict)
            .collect();
        assert!(
            conflicts.len() >= 2,
            "both duplicate entries should be marked Conflict, got {}",
            conflicts.len()
        );
        assert!(
            conflicts[0]
                .ambiguity_reason
                .as_ref()
                .unwrap()
                .contains("duplicate target path"),
            "reason should mention duplicate: {:?}",
            conflicts[0].ambiguity_reason
        );
    }

    #[test]
    fn scan_detects_existing_file_conflict() {
        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();

        fs::write(
            source.path().join("Inception.2010.1080p.BluRay.x264-GROUP.mkv"),
            b"video",
        )
        .unwrap();

        // Create an output file at the expected proposed path to simulate conflict
        let scanner = Scanner::new(config_with_output(output.path()));
        // First do a dry scan to see what path it proposes
        let results_pre = scanner.scan_folder(source.path()).unwrap();
        assert!(!results_pre.is_empty());

        // Now create a file at that proposed path
        let proposed = &results_pre[0].proposed_path;
        if let Some(parent) = proposed.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(proposed, b"existing file").unwrap();

        // Re-scan: should detect existing file conflict
        let results = scanner.scan_folder(source.path()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, ScanStatus::Conflict);
        assert!(
            results[0]
                .ambiguity_reason
                .as_ref()
                .unwrap()
                .contains("target file already exists"),
            "reason should mention existing: {:?}",
            results[0].ambiguity_reason
        );
    }

    // -----------------------------------------------------------------------
    // Ambiguous / low-confidence parse
    // -----------------------------------------------------------------------

    #[test]
    fn scan_flags_low_confidence_as_ambiguous() {
        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();

        // A filename that produces low confidence (no season, no episode, no year)
        fs::write(source.path().join("mystery.mkv"), b"video").unwrap();

        let scanner = Scanner::new(config_with_output(output.path()));
        let results = scanner.scan_folder(source.path()).unwrap();

        // Find any result that is flagged ambiguous
        let ambiguous: Vec<_> = results
            .iter()
            .filter(|r| r.status == ScanStatus::Ambiguous)
            .collect();
        assert!(
            !ambiguous.is_empty(),
            "low confidence parse should be flagged Ambiguous"
        );
    }

    // -----------------------------------------------------------------------
    // Filtering
    // -----------------------------------------------------------------------

    #[test]
    fn filter_by_media_type() {
        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();

        fs::write(
            source.path().join("The.Office.S02E03.720p.BluRay.x264-DEMAND.mkv"),
            b"series",
        )
        .unwrap();
        fs::write(
            source.path().join("Inception.2010.1080p.BluRay.x264-GROUP.mkv"),
            b"movie",
        )
        .unwrap();

        let scanner = Scanner::new(config_with_output(output.path()));
        let results = scanner.scan_folder(source.path()).unwrap();

        let movies = Scanner::filter_results(&results, &ScanFilter {
            media_type: Some(MediaType::Movie),
            ..ScanFilter::default()
        });
        assert_eq!(movies.len(), 1);
        assert_eq!(movies[0].media_info.media_type, MediaType::Movie);

        let series = Scanner::filter_results(&results, &ScanFilter {
            media_type: Some(MediaType::Series),
            ..ScanFilter::default()
        });
        assert_eq!(series.len(), 1);
        assert_eq!(series[0].media_info.media_type, MediaType::Series);
    }

    #[test]
    fn filter_by_status() {
        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();

        fs::write(
            source.path().join("Inception.2010.1080p.BluRay.x264-GROUP.mkv"),
            b"video",
        )
        .unwrap();
        // A vague file for ambiguous result
        fs::write(source.path().join("mystery.mkv"), b"video").unwrap();

        let scanner = Scanner::new(config_with_output(output.path()));
        let results = scanner.scan_folder(source.path()).unwrap();

        let ok_only = Scanner::filter_results(&results, &ScanFilter {
            status: Some(ScanStatus::Ok),
            ..ScanFilter::default()
        });
        assert!(
            ok_only.iter().all(|r| r.status == ScanStatus::Ok),
            "filter should only return Ok results"
        );
    }

    #[test]
    fn filter_by_title_search_case_insensitive() {
        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();

        fs::write(
            source.path().join("Inception.2010.1080p.BluRay.x264-GROUP.mkv"),
            b"video1",
        )
        .unwrap();
        fs::write(
            source.path().join("The.Office.S02E03.720p.mkv"),
            b"video2",
        )
        .unwrap();

        let scanner = Scanner::new(config_with_output(output.path()));
        let results = scanner.scan_folder(source.path()).unwrap();

        let filtered = Scanner::filter_results(&results, &ScanFilter {
            title_search: Some("inception".to_string()),
            ..ScanFilter::default()
        });
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].media_info.title, "Inception");
    }

    // -----------------------------------------------------------------------
    // Context-aware parsing
    // -----------------------------------------------------------------------

    #[test]
    fn scan_uses_context_from_sibling_files() {
        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();

        // Multiple episodes in same directory should use context-aware parsing
        fs::write(
            source.path().join("Show.S01E01.720p.mkv"),
            b"ep1",
        )
        .unwrap();
        fs::write(
            source.path().join("Show.S01E02.720p.mkv"),
            b"ep2",
        )
        .unwrap();

        let scanner = Scanner::new(config_with_output(output.path()));
        let results = scanner.scan_folder(source.path()).unwrap();
        assert_eq!(results.len(), 2);
        // Both should parse successfully (context helps)
        for r in &results {
            assert_ne!(r.status, ScanStatus::Error, "should parse with context");
        }
    }

    // -----------------------------------------------------------------------
    // Subtitle discovery
    // -----------------------------------------------------------------------

    #[test]
    fn scan_discovers_subtitles_for_video() {
        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();

        let video_name = "The.Office.S02E03.720p.BluRay.x264-DEMAND.mkv";
        fs::write(source.path().join(video_name), b"video").unwrap();
        fs::write(
            source.path().join("The.Office.S02E03.720p.BluRay.x264-DEMAND.en.srt"),
            b"subtitle",
        )
        .unwrap();

        let scanner = Scanner::new(config_with_output(output.path()));
        let results = scanner.scan_folder(source.path()).unwrap();
        assert_eq!(results.len(), 1);
        assert!(
            !results[0].subtitles.is_empty(),
            "subtitles should be discovered for the video"
        );
    }

    // -----------------------------------------------------------------------
    // Error paths
    // -----------------------------------------------------------------------

    #[test]
    fn scan_nonexistent_path_returns_error() {
        let output = TempDir::new().unwrap();
        let scanner = Scanner::new(config_with_output(output.path()));
        let result = scanner.scan_folder(Path::new("/nonexistent/path/12345"));
        assert!(result.is_err());
        match result.unwrap_err() {
            MediError::ScanPathNotFound { .. } => {}
            other => panic!("expected ScanPathNotFound, got: {other:?}"),
        }
    }

    #[test]
    fn scan_file_path_returns_not_directory_error() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("file.txt");
        fs::write(&file_path, b"not a dir").unwrap();

        let output = TempDir::new().unwrap();
        let scanner = Scanner::new(config_with_output(output.path()));
        let result = scanner.scan_folder(&file_path);
        assert!(result.is_err());
        match result.unwrap_err() {
            MediError::ScanPathNotDirectory { .. } => {}
            other => panic!("expected ScanPathNotDirectory, got: {other:?}"),
        }
    }
}
