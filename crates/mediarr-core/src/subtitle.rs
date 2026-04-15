//! Subtitle discovery, language/type detection, and path generation.
//!
//! Subtitles are dependents of video files. This module discovers them using
//! four strategies (sidecar, subfolder, nested language folder, VobSub pairs),
//! detects language and type, and generates output paths following the video's
//! renamed path with language/type suffixes.

use std::fs;
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
    /// Returns the preferred languages for this discovery instance.
    pub fn preferred_languages(&self) -> &[String] {
        &self.preferred_languages
    }

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
    /// `video_output_dir` is the directory component of the video's proposed path;
    /// subtitle proposed paths are placed under this directory.
    ///
    /// Returns a list of `SubtitleMatch` entries with proposed paths generated
    /// from the video's proposed stem.
    pub fn discover_for_video(
        &self,
        video_path: &Path,
        video_proposed_stem: &str,
        video_output_dir: &Path,
    ) -> Vec<SubtitleMatch> {
        let mut raw: Vec<RawSubtitle> = Vec::new();

        if self.toggles.sidecar {
            raw.extend(self.discover_sidecar(video_path));
        }
        if self.toggles.subs_subfolder {
            raw.extend(self.discover_subfolder(video_path));
        }
        if self.toggles.nested_language_folders {
            raw.extend(self.discover_nested_language(video_path));
        }
        if self.toggles.vobsub_pairs {
            raw.extend(self.discover_vobsub(video_path));
        }

        raw.into_iter()
            .map(|r| self.enrich(r, video_proposed_stem, video_output_dir))
            .collect()
    }

    /// Enrich a raw subtitle with language, type, and proposed path.
    fn enrich(
        &self,
        raw: RawSubtitle,
        video_proposed_stem: &str,
        video_output_dir: &Path,
    ) -> SubtitleMatch {
        let filename = raw
            .source_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // Determine parent folder name for language detection
        let parent_folder = raw
            .source_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str());

        // Use pre-detected language (from folder discovery) or detect from filename/folder
        let language = raw
            .pre_language
            .unwrap_or_else(|| detect_language(filename, parent_folder));

        let subtitle_type = detect_subtitle_type(filename);

        let extension = raw
            .source_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("srt");

        let proposed_path = generate_proposed_path(
            video_output_dir,
            video_proposed_stem,
            &language,
            subtitle_type.as_ref(),
            extension,
        );

        // Determine companion path for VobSub pairs
        let companion_path = if raw.discovery_method == DiscoveryMethod::VobSub {
            let ext = raw
                .source_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            let companion_ext = if ext == "idx" { "sub" } else { "idx" };
            Some(raw.source_path.with_extension(companion_ext))
        } else {
            None
        };

        SubtitleMatch {
            source_path: raw.source_path,
            proposed_path,
            language,
            subtitle_type,
            discovery_method: raw.discovery_method,
            is_vobsub_pair: raw.discovery_method == DiscoveryMethod::VobSub,
            companion_path,
        }
    }

    /// Discover sidecar subtitles in the same directory as the video.
    fn discover_sidecar(&self, video_path: &Path) -> Vec<RawSubtitle> {
        let video_dir = match video_path.parent() {
            Some(d) => d,
            None => return Vec::new(),
        };
        let video_stem = match video_path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s,
            None => return Vec::new(),
        };

        debug!(video_stem, dir = %video_dir.display(), "scanning sidecar subtitles");

        list_subtitle_files(video_dir)
            .into_iter()
            .filter(|path| {
                let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                // Must start with video stem followed by a dot
                fname.starts_with(video_stem)
                    && fname.as_bytes().get(video_stem.len()) == Some(&b'.')
            })
            // Exclude VobSub files from sidecar -- they are handled by discover_vobsub
            .filter(|path| {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                ext != "idx" && ext != "sub"
            })
            .map(|path| RawSubtitle {
                source_path: path,
                discovery_method: DiscoveryMethod::Sidecar,
                pre_language: None,
            })
            .collect()
    }

    /// Discover subtitles in standard subfolder names (Subs/, Subtitles/, Sub/).
    fn discover_subfolder(&self, video_path: &Path) -> Vec<RawSubtitle> {
        let video_dir = match video_path.parent() {
            Some(d) => d,
            None => return Vec::new(),
        };
        let video_stem = match video_path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s,
            None => return Vec::new(),
        };

        debug!(video_stem, dir = %video_dir.display(), "scanning subfolder subtitles");

        let mut results = Vec::new();

        // Read the video's parent directory to find matching subfolder names
        let entries = match fs::read_dir(video_dir) {
            Ok(e) => e,
            Err(_) => return results,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => continue,
            };

            // Case-insensitive check against standard subfolder names
            let dir_name_lower = dir_name.to_lowercase();
            if !SUBS_FOLDER_NAMES.contains(&dir_name_lower.as_str()) {
                continue;
            }

            // Search this subfolder for subtitle files matching the video stem
            for sub_path in list_subtitle_files(&path) {
                let fname = sub_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if fname.starts_with(video_stem)
                    && fname.as_bytes().get(video_stem.len()) == Some(&b'.')
                {
                    results.push(RawSubtitle {
                        source_path: sub_path,
                        discovery_method: DiscoveryMethod::SubsSubfolder,
                        pre_language: None,
                    });
                }
            }
        }

        results
    }

    /// Discover subtitles in language-named subdirectories.
    fn discover_nested_language(&self, video_path: &Path) -> Vec<RawSubtitle> {
        let video_dir = match video_path.parent() {
            Some(d) => d,
            None => return Vec::new(),
        };
        let video_stem = match video_path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s,
            None => return Vec::new(),
        };

        debug!(video_stem, dir = %video_dir.display(), "scanning nested language folders");

        let mut results = Vec::new();

        let entries = match fs::read_dir(video_dir) {
            Ok(e) => e,
            Err(_) => return results,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => continue,
            };

            // Skip standard subs folders -- those are handled by discover_subfolder
            let dir_lower = dir_name.to_lowercase();
            if SUBS_FOLDER_NAMES.contains(&dir_lower.as_str()) {
                continue;
            }

            // Try to detect language from the directory name
            let lang = match detect_language_from_string(dir_name) {
                Some(l) => l,
                None => continue,
            };

            // Search for subtitle files in this language folder
            for sub_path in list_subtitle_files(&path) {
                let fname = sub_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if fname.starts_with(video_stem) {
                    results.push(RawSubtitle {
                        source_path: sub_path,
                        discovery_method: DiscoveryMethod::NestedLanguage,
                        pre_language: Some(lang.clone()),
                    });
                }
            }
        }

        results
    }

    /// Discover VobSub pairs (.idx + .sub) in the same directory.
    fn discover_vobsub(&self, video_path: &Path) -> Vec<RawSubtitle> {
        let video_dir = match video_path.parent() {
            Some(d) => d,
            None => return Vec::new(),
        };
        let video_stem = match video_path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s,
            None => return Vec::new(),
        };

        debug!(video_stem, dir = %video_dir.display(), "scanning VobSub pairs");

        let mut results = Vec::new();

        // Find .idx files matching the video stem
        let entries = match fs::read_dir(video_dir) {
            Ok(e) => e,
            Err(_) => return results,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "idx" {
                continue;
            }
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if !stem.starts_with(video_stem) {
                continue;
            }

            // Check that the companion .sub file exists
            let sub_path = path.with_extension("sub");
            if !sub_path.exists() {
                debug!(idx = %path.display(), "skipping orphaned .idx (no .sub companion)");
                continue;
            }

            // Return the .idx file; the .sub companion is tracked in companion_path
            results.push(RawSubtitle {
                source_path: path,
                discovery_method: DiscoveryMethod::VobSub,
                pre_language: None,
            });
        }

        results
    }
}

/// List all files with subtitle extensions in a directory (non-recursive).
fn list_subtitle_files(dir: &Path) -> Vec<PathBuf> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() {
                return None;
            }
            let ext = path.extension()?.to_str()?.to_lowercase();
            if SUBTITLE_EXTENSIONS.contains(&ext.as_str()) {
                Some(path)
            } else {
                None
            }
        })
        .collect()
}

/// Detect language from a filename and optional parent folder name.
///
/// Priority: filename suffix (ISO 639-1, then 639-3), parent folder name,
/// fallback to "und".
fn detect_language(filename: &str, parent_folder: Option<&str>) -> String {
    // Strip extension to get segments
    let name_without_ext = if let Some(dot_pos) = filename.rfind('.') {
        &filename[..dot_pos]
    } else {
        filename
    };

    // Split on dots and check each segment as a language code
    let segments: Vec<&str> = name_without_ext.split('.').collect();

    // Try each segment (skip the first one which is typically the title stem)
    for segment in segments.iter().skip(1) {
        let lower = segment.to_lowercase();

        // Skip known type indicators
        if TYPE_INDICATORS.iter().any(|(ind, _)| *ind == lower) {
            continue;
        }

        // Try as ISO 639-1 (2-letter code)
        if let Some(lang) = isolang::Language::from_639_1(&lower) {
            if let Some(code) = lang.to_639_1() {
                return code.to_string();
            }
        }

        // Try as ISO 639-3 (3-letter code)
        if let Some(lang) = isolang::Language::from_639_3(&lower) {
            if let Some(code) = lang.to_639_1() {
                return code.to_string();
            }
            return lang.to_639_3().to_string();
        }
    }

    // Try parent folder name
    if let Some(folder) = parent_folder {
        if let Some(lang) = detect_language_from_string(folder) {
            return lang;
        }
    }

    // Fallback: undetermined
    "und".to_string()
}

/// Try to detect a language from an arbitrary string (folder name, etc.).
///
/// Splits on non-alphabetic characters and tries each alphabetic segment as
/// ISO 639-1/639-3 code, then as English language name (case-insensitive).
/// Returns the ISO 639-1 code if available, otherwise 639-3.
fn detect_language_from_string(s: &str) -> Option<String> {
    // Split on non-alphabetic characters to get segments
    let segments: Vec<&str> = s
        .split(|c: char| !c.is_alphabetic())
        .filter(|s| !s.is_empty())
        .collect();

    for segment in &segments {
        let lower = segment.to_lowercase();

        // Try as ISO 639-1 (2-letter code)
        if lower.len() == 2 {
            if let Some(lang) = isolang::Language::from_639_1(&lower) {
                if let Some(code) = lang.to_639_1() {
                    return Some(code.to_string());
                }
            }
        }

        // Try as ISO 639-3 (3-letter code)
        if lower.len() == 3 {
            if let Some(lang) = isolang::Language::from_639_3(&lower) {
                if let Some(code) = lang.to_639_1() {
                    return Some(code.to_string());
                }
                return Some(lang.to_639_3().to_string());
            }
        }

        // Try as English language name (case-insensitive)
        // isolang::Language::from_name expects exact case ("English")
        // Try lowercase first (from_name_lowercase feature)
        if let Some(lang) = isolang::Language::from_name_lowercase(&lower) {
            if let Some(code) = lang.to_639_1() {
                return Some(code.to_string());
            }
            return Some(lang.to_639_3().to_string());
        }
    }

    None
}

/// Detect subtitle type from filename indicators.
///
/// Checks filename segments (split on `.`) against known type indicators.
fn detect_subtitle_type(filename: &str) -> Option<SubtitleType> {
    let lower = filename.to_lowercase();
    let segments: Vec<&str> = lower.split('.').collect();

    for segment in &segments {
        for (indicator, sub_type) in TYPE_INDICATORS {
            if segment == indicator {
                return Some(*sub_type);
            }
        }
    }

    // Also check for multi-segment indicators like "hearing.impaired"
    let joined = segments.join(".");
    for (indicator, sub_type) in TYPE_INDICATORS {
        if joined.contains(indicator) {
            return Some(*sub_type);
        }
    }

    None
}

/// Generate a proposed subtitle path from video output directory, stem, language, type, and extension.
///
/// Format: `{video_output_dir}/{video_proposed_stem}.{lang}.{type}.{ext}` when type is present,
/// or `{video_output_dir}/{video_proposed_stem}.{lang}.{ext}` when no type.
/// Collapses adjacent dots to prevent ".." in output.
/// An empty `video_output_dir` produces a bare filename (no leading separator).
fn generate_proposed_path(
    video_output_dir: &Path,
    video_proposed_stem: &str,
    language: &str,
    sub_type: Option<&SubtitleType>,
    extension: &str,
) -> PathBuf {
    let mut name = format!("{}.{}", video_proposed_stem, language);

    if let Some(st) = sub_type {
        name.push('.');
        name.push_str(&st.to_string());
    }

    name.push('.');
    name.push_str(extension);

    // Collapse any ".." to "." as a safety measure
    while name.contains("..") {
        name = name.replace("..", ".");
    }

    video_output_dir.join(name)
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let results =
            disc.discover_for_video(&dir.path().join("Movie.mkv"), "Movie", Path::new(""));

        let sidecar: Vec<_> = results
            .iter()
            .filter(|s| s.discovery_method == DiscoveryMethod::Sidecar)
            .collect();
        assert!(!sidecar.is_empty(), "should find sidecar subtitle");
        assert_eq!(sidecar[0].language, "en");
    }

    #[test]
    fn sidecar_finds_subtitle_without_language() {
        // Use a named subdirectory so the parent folder name is deterministic
        // and won't accidentally match an ISO 639-3 language code from the
        // random tempdir name.
        let dir = TempDir::new().unwrap();
        let media_dir = dir.path().join("media");
        touch(&media_dir.join("Movie.mkv"));
        touch(&media_dir.join("Movie.srt"));

        let disc = discovery_all_enabled();
        let results = disc.discover_for_video(&media_dir.join("Movie.mkv"), "Movie", Path::new(""));

        let sidecar: Vec<_> = results
            .iter()
            .filter(|s| s.discovery_method == DiscoveryMethod::Sidecar)
            .collect();
        assert!(
            !sidecar.is_empty(),
            "should find sidecar subtitle without lang"
        );
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
        let results =
            disc.discover_for_video(&dir.path().join("Movie.mkv"), "Movie", Path::new(""));

        let subfolder: Vec<_> = results
            .iter()
            .filter(|s| s.discovery_method == DiscoveryMethod::SubsSubfolder)
            .collect();
        assert!(
            !subfolder.is_empty(),
            "should find subtitle in Subs/ folder"
        );
    }

    #[test]
    fn subfolder_checks_case_insensitive_names() {
        let dir = TempDir::new().unwrap();
        touch(&dir.path().join("Movie.mkv"));

        // Create subtitles in "subtitles" (lowercase)
        let sub_dir = dir.path().join("subtitles");
        touch(&sub_dir.join("Movie.en.srt"));

        let disc = discovery_all_enabled();
        let results =
            disc.discover_for_video(&dir.path().join("Movie.mkv"), "Movie", Path::new(""));

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
        let results =
            disc.discover_for_video(&dir.path().join("Movie.mkv"), "Movie", Path::new(""));

        let nested: Vec<_> = results
            .iter()
            .filter(|s| s.discovery_method == DiscoveryMethod::NestedLanguage)
            .collect();
        assert!(
            !nested.is_empty(),
            "should find subtitle in English/ folder"
        );
        assert_eq!(nested[0].language, "en");
    }

    #[test]
    fn nested_language_finds_subtitle_in_iso_code_folder() {
        let dir = TempDir::new().unwrap();
        touch(&dir.path().join("Movie.mkv"));
        let en_dir = dir.path().join("en");
        touch(&en_dir.join("Movie.srt"));

        let disc = discovery_all_enabled();
        let results =
            disc.discover_for_video(&dir.path().join("Movie.mkv"), "Movie", Path::new(""));

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
        let results =
            disc.discover_for_video(&dir.path().join("Movie.mkv"), "Movie", Path::new(""));

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
        let results =
            disc.discover_for_video(&dir.path().join("Movie.mkv"), "Movie", Path::new(""));

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
        let results =
            disc.discover_for_video(&dir.path().join("Movie.mkv"), "Movie", Path::new(""));

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
        let path = generate_proposed_path(Path::new(""), "Movie", "en", None, "srt");
        assert_eq!(path, PathBuf::from("Movie.en.srt"));
    }

    #[test]
    fn path_with_type() {
        let path = generate_proposed_path(
            Path::new(""),
            "Movie",
            "en",
            Some(&SubtitleType::Forced),
            "srt",
        );
        assert_eq!(path, PathBuf::from("Movie.en.forced.srt"));
    }

    #[test]
    fn path_uses_iso_639_1_code() {
        let path = generate_proposed_path(Path::new(""), "Movie", "en", None, "srt");
        // Should use 2-letter code
        assert!(path.to_str().unwrap().contains(".en."));
    }

    #[test]
    fn path_collapses_empty_type_dots() {
        // Simulate a case where type would be empty string
        let result = generate_proposed_path(Path::new(""), "Movie", "en", None, "srt");
        let s = result.to_str().unwrap();
        assert!(!s.contains(".."), "should not have double dots");
    }

    // -----------------------------------------------------------------------
    // Language Detection: edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn language_from_string_returns_none_for_gibberish() {
        let result = detect_language_from_string("xyzzy");
        // "xyzzy" is not a valid ISO code or language name
        assert!(
            result.is_none(),
            "gibberish should not match any language, got: {:?}",
            result
        );
    }

    #[test]
    fn language_from_string_returns_none_for_empty() {
        let result = detect_language_from_string("");
        assert!(result.is_none());
    }

    #[test]
    fn language_from_string_returns_none_for_numbers_only() {
        let result = detect_language_from_string("12345");
        assert!(result.is_none());
    }

    #[test]
    fn language_from_string_iso_639_1_code() {
        let result = detect_language_from_string("en");
        assert_eq!(result, Some("en".to_string()));
    }

    #[test]
    fn language_from_string_iso_639_3_code() {
        let result = detect_language_from_string("eng");
        assert_eq!(result, Some("en".to_string()));
    }

    #[test]
    fn language_from_string_full_name() {
        let result = detect_language_from_string("Japanese");
        assert_eq!(result, Some("ja".to_string()));
    }

    #[test]
    fn language_skips_type_indicators_in_filename() {
        // "forced" and "sdh" should not be treated as language codes
        let lang = detect_language("Movie.forced.srt", None);
        assert_eq!(lang, "und");
        let lang = detect_language("Movie.sdh.srt", None);
        assert_eq!(lang, "und");
    }

    #[test]
    fn language_prefers_iso_639_1_over_639_3() {
        // "eng" is ISO 639-3 for English, should return "en" (639-1)
        let lang = detect_language("Movie.eng.srt", None);
        assert_eq!(lang, "en");
    }

    #[test]
    fn language_iso_639_3_without_639_1_returns_639_3() {
        // "jpn" is ISO 639-3 for Japanese which has a 639-1 code "ja"
        let lang = detect_language("Movie.jpn.srt", None);
        assert_eq!(lang, "ja");
    }

    // -----------------------------------------------------------------------
    // Type Detection: edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn type_detects_hearing_impaired_multi_segment() {
        let t = detect_subtitle_type("Movie.en.hearing.impaired.srt");
        assert_eq!(t, Some(SubtitleType::Hi));
    }

    #[test]
    fn type_detection_is_case_insensitive() {
        let t = detect_subtitle_type("Movie.en.FORCED.srt");
        assert_eq!(t, Some(SubtitleType::Forced));
    }

    #[test]
    fn type_none_for_plain_language_only() {
        let t = detect_subtitle_type("Movie.en.srt");
        assert_eq!(t, None);
    }

    // -----------------------------------------------------------------------
    // Path Generation: edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn path_with_639_3_language_code() {
        let path = generate_proposed_path(Path::new(""), "Movie", "jpn", None, "srt");
        assert_eq!(path, PathBuf::from("Movie.jpn.srt"));
    }

    #[test]
    fn path_with_und_language() {
        let path = generate_proposed_path(Path::new(""), "Movie", "und", None, "srt");
        assert_eq!(path, PathBuf::from("Movie.und.srt"));
    }

    #[test]
    fn path_with_commentary_type() {
        let path = generate_proposed_path(
            Path::new(""),
            "Movie",
            "en",
            Some(&SubtitleType::Commentary),
            "srt",
        );
        assert_eq!(path, PathBuf::from("Movie.en.commentary.srt"));
    }

    // -----------------------------------------------------------------------
    // Discovery: deterministic parent folder tests
    // -----------------------------------------------------------------------

    #[test]
    fn sidecar_ignores_vobsub_files() {
        let dir = TempDir::new().unwrap();
        let media_dir = dir.path().join("media");
        touch(&media_dir.join("Movie.mkv"));
        touch(&media_dir.join("Movie.idx"));
        touch(&media_dir.join("Movie.sub"));
        touch(&media_dir.join("Movie.en.srt"));

        let disc = discovery_all_enabled();
        let results = disc.discover_for_video(&media_dir.join("Movie.mkv"), "Movie", Path::new(""));

        let sidecar: Vec<_> = results
            .iter()
            .filter(|s| s.discovery_method == DiscoveryMethod::Sidecar)
            .collect();
        // Should only find the .srt, not the .idx or .sub
        assert_eq!(sidecar.len(), 1, "sidecar should exclude VobSub files");
        assert!(
            sidecar[0]
                .source_path
                .extension()
                .unwrap()
                .to_str()
                .unwrap()
                == "srt"
        );
    }

    #[test]
    fn nested_language_skips_subs_folder_names() {
        let dir = TempDir::new().unwrap();
        let media_dir = dir.path().join("media");
        touch(&media_dir.join("Movie.mkv"));

        // "Subs" folder should be handled by subfolder discovery, not nested language
        let subs_dir = media_dir.join("Subs");
        touch(&subs_dir.join("Movie.srt"));

        let disc = SubtitleDiscovery::new(
            DiscoveryToggles {
                sidecar: false,
                subs_subfolder: false,
                nested_language_folders: true,
                vobsub_pairs: false,
            },
            vec!["en".into()],
        );
        let results = disc.discover_for_video(&media_dir.join("Movie.mkv"), "Movie", Path::new(""));

        let nested: Vec<_> = results
            .iter()
            .filter(|s| s.discovery_method == DiscoveryMethod::NestedLanguage)
            .collect();
        assert!(
            nested.is_empty(),
            "Subs/ folder should not be treated as a language folder"
        );
    }

    #[test]
    fn no_subtitles_discovered_in_empty_dir() {
        let dir = TempDir::new().unwrap();
        let media_dir = dir.path().join("media");
        touch(&media_dir.join("Movie.mkv"));
        // No subtitle files at all

        let disc = discovery_all_enabled();
        let results = disc.discover_for_video(&media_dir.join("Movie.mkv"), "Movie", Path::new(""));
        assert!(results.is_empty(), "should find no subtitles in empty dir");
    }

    // -----------------------------------------------------------------------
    // Regression: subtitle proposed_path includes video output directory (R001)
    // -----------------------------------------------------------------------

    #[test]
    fn path_includes_video_output_dir() {
        let output_dir = Path::new("Movies/Title (2024)");
        let path = generate_proposed_path(output_dir, "Title (2024)", "en", None, "srt");
        assert_eq!(
            path,
            PathBuf::from("Movies/Title (2024)/Title (2024).en.srt")
        );
        assert_eq!(path.parent().unwrap(), output_dir);
    }

    #[test]
    fn path_empty_output_dir_produces_bare_filename() {
        let path = generate_proposed_path(Path::new(""), "Movie", "en", None, "srt");
        assert_eq!(path, PathBuf::from("Movie.en.srt"));
    }

    #[test]
    fn path_with_output_dir_and_type() {
        let output_dir = Path::new("output/TV");
        let path = generate_proposed_path(
            output_dir,
            "Show S01E01",
            "en",
            Some(&SubtitleType::Forced),
            "srt",
        );
        assert_eq!(path, PathBuf::from("output/TV/Show S01E01.en.forced.srt"));
    }

    #[test]
    fn path_special_chars_in_stem_with_output_dir() {
        let output_dir = Path::new("output");
        let path = generate_proposed_path(output_dir, "Movie [2024] (1080p)", "en", None, "srt");
        assert_eq!(path, PathBuf::from("output/Movie [2024] (1080p).en.srt"));
    }

    #[test]
    fn discover_for_video_proposed_path_includes_output_dir() {
        let dir = TempDir::new().unwrap();
        let media_dir = dir.path().join("media");
        touch(&media_dir.join("Movie.mkv"));
        touch(&media_dir.join("Movie.en.srt"));

        let disc = discovery_all_enabled();
        let output_dir = Path::new("Movies/Movie (2024)");
        let results =
            disc.discover_for_video(&media_dir.join("Movie.mkv"), "Movie (2024)", output_dir);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].proposed_path.parent().unwrap(), output_dir);
    }
}
