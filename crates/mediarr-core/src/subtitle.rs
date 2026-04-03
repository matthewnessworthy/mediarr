//! Subtitle discovery, language/type detection, and path generation.
//!
//! Subtitles are dependents of video files. This module discovers them using
//! four strategies (sidecar, subfolder, nested language folder, VobSub pairs),
//! detects language and type, and generates output paths following the video's
//! renamed path with language/type suffixes.

use std::path::{Path, PathBuf};

use tracing::debug;

use crate::types::{DiscoveryMethod, DiscoveryToggles, SubtitleMatch, SubtitleType};

/// Known subtitle file extensions.
const SUBTITLE_EXTENSIONS: &[&str] = &["srt", "ass", "ssa", "sub", "idx", "sup", "vtt"];

/// Standard subfolder names that may contain subtitle files.
const SUBS_FOLDER_NAMES: &[&str] = &["subs", "subtitles", "sub"];

/// Mapping of filename indicators to subtitle types.
const TYPE_INDICATORS: &[(&str, SubtitleType)] = &[
    ("forced", SubtitleType::Forced),
    ("sdh", SubtitleType::Sdh),
    ("hi", SubtitleType::Hi),
    ("hearing.impaired", SubtitleType::Hi),
    ("commentary", SubtitleType::Commentary),
];

/// Intermediate representation before enrichment with language/type/path.
struct RawSubtitle {
    /// Path to the discovered subtitle file.
    source_path: PathBuf,
    /// How it was discovered.
    discovery_method: DiscoveryMethod,
    /// Pre-detected language (e.g. from folder name), if any.
    pre_language: Option<String>,
}

/// Discovers, classifies, and generates rename paths for subtitle files
/// associated with video files.
pub struct SubtitleDiscovery {
    toggles: DiscoveryToggles,
    preferred_languages: Vec<String>,
}

impl SubtitleDiscovery {
    /// Create a new subtitle discovery instance.
    ///
    /// `toggles` controls which discovery methods are active.
    /// `preferred_languages` is a list of ISO 639-1 language codes
    /// in priority order.
    pub fn new(toggles: DiscoveryToggles, preferred_languages: Vec<String>) -> Self {
        Self {
            toggles,
            preferred_languages,
        }
    }

    /// Discover all subtitle files for a given video file.
    ///
    /// `video_path` is the original video file path on disk.
    /// `video_proposed_stem` is the proposed output stem (without extension)
    /// that subtitles should follow.
    ///
    /// Returns a list of `SubtitleMatch` entries with proposed paths generated
    /// from the video's proposed stem.
    pub fn discover_for_video(
        &self,
        video_path: &Path,
        video_proposed_stem: &str,
    ) -> Vec<SubtitleMatch> {
        todo!("implement discover_for_video")
    }
}

/// Detect language from a filename and optional parent folder name.
///
/// Priority: filename suffix (ISO 639-1, then 639-3), parent folder name,
/// fallback to "und".
fn detect_language(filename: &str, parent_folder: Option<&str>) -> String {
    todo!("implement detect_language")
}

/// Try to detect a language from an arbitrary string (folder name, etc.).
///
/// Strips non-alphabetic characters, tries each alphabetic segment as
/// ISO 639-1/639-3 code, then as English language name (case-insensitive).
/// Returns the ISO 639-1 code if available, otherwise 639-3.
fn detect_language_from_string(s: &str) -> Option<String> {
    todo!("implement detect_language_from_string")
}

/// Detect subtitle type from filename indicators.
///
/// Checks filename segments (split on `.`) against known type indicators.
fn detect_subtitle_type(filename: &str) -> Option<SubtitleType> {
    todo!("implement detect_subtitle_type")
}

/// Generate a proposed subtitle path from video stem, language, type, and extension.
///
/// Format: `{video_proposed_stem}.{lang}.{type}.{ext}` when type is present,
/// or `{video_proposed_stem}.{lang}.{ext}` when no type.
/// Collapses adjacent dots to prevent ".." in output.
fn generate_proposed_path(
    video_proposed_stem: &str,
    language: &str,
    sub_type: Option<&SubtitleType>,
    extension: &str,
) -> PathBuf {
    todo!("implement generate_proposed_path")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper: create a file at the given path with empty content.
    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, "").unwrap();
    }

    /// Helper: create a SubtitleDiscovery with all toggles enabled.
    fn discovery_all_enabled() -> SubtitleDiscovery {
        SubtitleDiscovery::new(DiscoveryToggles::default(), vec!["en".into()])
    }

    // -----------------------------------------------------------------------
    // Discovery: Sidecar
    // -----------------------------------------------------------------------

    #[test]
    fn sidecar_finds_subtitle_with_language_suffix() {
        let dir = TempDir::new().unwrap();
        touch(&dir.path().join("Movie.mkv"));
        touch(&dir.path().join("Movie.en.srt"));

        let disc = discovery_all_enabled();
        let results = disc.discover_for_video(&dir.path().join("Movie.mkv"), "Movie");

        let sidecar: Vec<_> = results
            .iter()
            .filter(|s| s.discovery_method == DiscoveryMethod::Sidecar)
            .collect();
        assert!(!sidecar.is_empty(), "should find sidecar subtitle");
        assert_eq!(sidecar[0].language, "en");
    }

    #[test]
    fn sidecar_finds_subtitle_without_language() {
        let dir = TempDir::new().unwrap();
        touch(&dir.path().join("Movie.mkv"));
        touch(&dir.path().join("Movie.srt"));

        let disc = discovery_all_enabled();
        let results = disc.discover_for_video(&dir.path().join("Movie.mkv"), "Movie");

        let sidecar: Vec<_> = results
            .iter()
            .filter(|s| s.discovery_method == DiscoveryMethod::Sidecar)
            .collect();
        assert!(!sidecar.is_empty(), "should find sidecar subtitle without lang");
        assert_eq!(sidecar[0].language, "und");
    }

    // -----------------------------------------------------------------------
    // Discovery: Subfolder
    // -----------------------------------------------------------------------

    #[test]
    fn subfolder_finds_subtitle_in_subs_dir() {
        let dir = TempDir::new().unwrap();
        touch(&dir.path().join("Movie.mkv"));
        let subs_dir = dir.path().join("Subs");
        touch(&subs_dir.join("Movie.en.srt"));

        let disc = discovery_all_enabled();
        let results = disc.discover_for_video(&dir.path().join("Movie.mkv"), "Movie");

        let subfolder: Vec<_> = results
            .iter()
            .filter(|s| s.discovery_method == DiscoveryMethod::SubsSubfolder)
            .collect();
        assert!(!subfolder.is_empty(), "should find subtitle in Subs/ folder");
    }

    #[test]
    fn subfolder_checks_case_insensitive_names() {
        let dir = TempDir::new().unwrap();
        touch(&dir.path().join("Movie.mkv"));

        // Create subtitles in "subtitles" (lowercase)
        let sub_dir = dir.path().join("subtitles");
        touch(&sub_dir.join("Movie.en.srt"));

        let disc = discovery_all_enabled();
        let results = disc.discover_for_video(&dir.path().join("Movie.mkv"), "Movie");

        let subfolder: Vec<_> = results
            .iter()
            .filter(|s| s.discovery_method == DiscoveryMethod::SubsSubfolder)
            .collect();
        assert!(
            !subfolder.is_empty(),
            "should find subtitle in case-insensitive subfolder"
        );
    }

    // -----------------------------------------------------------------------
    // Discovery: Nested Language Folders
    // -----------------------------------------------------------------------

    #[test]
    fn nested_language_finds_subtitle_in_english_folder() {
        let dir = TempDir::new().unwrap();
        touch(&dir.path().join("Movie.mkv"));
        let eng_dir = dir.path().join("English");
        touch(&eng_dir.join("Movie.srt"));

        let disc = discovery_all_enabled();
        let results = disc.discover_for_video(&dir.path().join("Movie.mkv"), "Movie");

        let nested: Vec<_> = results
            .iter()
            .filter(|s| s.discovery_method == DiscoveryMethod::NestedLanguage)
            .collect();
        assert!(!nested.is_empty(), "should find subtitle in English/ folder");
        assert_eq!(nested[0].language, "en");
    }

    #[test]
    fn nested_language_finds_subtitle_in_iso_code_folder() {
        let dir = TempDir::new().unwrap();
        touch(&dir.path().join("Movie.mkv"));
        let en_dir = dir.path().join("en");
        touch(&en_dir.join("Movie.srt"));

        let disc = discovery_all_enabled();
        let results = disc.discover_for_video(&dir.path().join("Movie.mkv"), "Movie");

        let nested: Vec<_> = results
            .iter()
            .filter(|s| s.discovery_method == DiscoveryMethod::NestedLanguage)
            .collect();
        assert!(!nested.is_empty(), "should find subtitle in en/ folder");
        assert_eq!(nested[0].language, "en");
    }

    // -----------------------------------------------------------------------
    // Discovery: VobSub
    // -----------------------------------------------------------------------

    #[test]
    fn vobsub_finds_paired_idx_and_sub() {
        let dir = TempDir::new().unwrap();
        touch(&dir.path().join("Movie.mkv"));
        touch(&dir.path().join("Movie.idx"));
        touch(&dir.path().join("Movie.sub"));

        let disc = discovery_all_enabled();
        let results = disc.discover_for_video(&dir.path().join("Movie.mkv"), "Movie");

        let vobsub: Vec<_> = results
            .iter()
            .filter(|s| s.discovery_method == DiscoveryMethod::VobSub)
            .collect();
        assert!(!vobsub.is_empty(), "should find VobSub pair");
        assert!(vobsub.iter().all(|s| s.is_vobsub_pair));
    }

    #[test]
    fn vobsub_ignores_orphaned_idx_without_sub() {
        let dir = TempDir::new().unwrap();
        touch(&dir.path().join("Movie.mkv"));
        touch(&dir.path().join("Movie.idx"));
        // No .sub file

        let disc = discovery_all_enabled();
        let results = disc.discover_for_video(&dir.path().join("Movie.mkv"), "Movie");

        let vobsub: Vec<_> = results
            .iter()
            .filter(|s| s.discovery_method == DiscoveryMethod::VobSub)
            .collect();
        assert!(vobsub.is_empty(), "should not find orphaned .idx");
    }

    // -----------------------------------------------------------------------
    // Discovery: Toggle Control
    // -----------------------------------------------------------------------

    #[test]
    fn disabled_toggle_skips_method() {
        let dir = TempDir::new().unwrap();
        touch(&dir.path().join("Movie.mkv"));
        touch(&dir.path().join("Movie.en.srt"));

        let toggles = DiscoveryToggles {
            sidecar: false,
            subs_subfolder: true,
            nested_language_folders: true,
            vobsub_pairs: true,
        };
        let disc = SubtitleDiscovery::new(toggles, vec!["en".into()]);
        let results = disc.discover_for_video(&dir.path().join("Movie.mkv"), "Movie");

        let sidecar: Vec<_> = results
            .iter()
            .filter(|s| s.discovery_method == DiscoveryMethod::Sidecar)
            .collect();
        assert!(sidecar.is_empty(), "sidecar should be disabled");
    }

    // -----------------------------------------------------------------------
    // Language Detection
    // -----------------------------------------------------------------------

    #[test]
    fn language_from_639_1_suffix() {
        let lang = detect_language("Movie.en.srt", None);
        assert_eq!(lang, "en");
    }

    #[test]
    fn language_from_639_3_suffix() {
        // "eng" is ISO 639-3 for English
        let lang = detect_language("Movie.eng.srt", None);
        assert_eq!(lang, "en");
    }

    #[test]
    fn language_from_folder_name() {
        let lang = detect_language("Movie.srt", Some("English"));
        assert_eq!(lang, "en");
    }

    #[test]
    fn language_from_lowercase_folder() {
        let lang = detect_language("Movie.srt", Some("english"));
        assert_eq!(lang, "en");
    }

    #[test]
    fn language_from_uppercase_folder() {
        let lang = detect_language("Movie.srt", Some("ENGLISH"));
        assert_eq!(lang, "en");
    }

    #[test]
    fn language_fallback_to_und() {
        let lang = detect_language("Movie.srt", None);
        assert_eq!(lang, "und");
    }

    #[test]
    fn language_from_string_strips_non_alpha() {
        // "Subtitles-English" should try segments, find "English"
        let result = detect_language_from_string("Subtitles-English");
        assert_eq!(result, Some("en".to_string()));
    }

    // -----------------------------------------------------------------------
    // Type Detection
    // -----------------------------------------------------------------------

    #[test]
    fn type_detects_forced() {
        let t = detect_subtitle_type("Movie.en.forced.srt");
        assert_eq!(t, Some(SubtitleType::Forced));
    }

    #[test]
    fn type_detects_sdh() {
        let t = detect_subtitle_type("Movie.en.sdh.srt");
        assert_eq!(t, Some(SubtitleType::Sdh));
    }

    #[test]
    fn type_detects_hi() {
        let t = detect_subtitle_type("Movie.en.hi.srt");
        assert_eq!(t, Some(SubtitleType::Hi));
    }

    #[test]
    fn type_detects_commentary() {
        let t = detect_subtitle_type("Movie.en.commentary.srt");
        assert_eq!(t, Some(SubtitleType::Commentary));
    }

    #[test]
    fn type_none_when_absent() {
        let t = detect_subtitle_type("Movie.en.srt");
        assert_eq!(t, None);
    }

    // -----------------------------------------------------------------------
    // Path Generation
    // -----------------------------------------------------------------------

    #[test]
    fn path_without_type() {
        let path = generate_proposed_path("Movie", "en", None, "srt");
        assert_eq!(path, PathBuf::from("Movie.en.srt"));
    }

    #[test]
    fn path_with_type() {
        let path = generate_proposed_path(
            "Movie",
            "en",
            Some(&SubtitleType::Forced),
            "srt",
        );
        assert_eq!(path, PathBuf::from("Movie.en.forced.srt"));
    }

    #[test]
    fn path_uses_iso_639_1_code() {
        let path = generate_proposed_path("Movie", "en", None, "srt");
        // Should use 2-letter code
        assert!(path.to_str().unwrap().contains(".en."));
    }

    #[test]
    fn path_collapses_empty_type_dots() {
        // Simulate a case where type would be empty string
        let result = generate_proposed_path("Movie", "en", None, "srt");
        let s = result.to_str().unwrap();
        assert!(!s.contains(".."), "should not have double dots");
    }
}
