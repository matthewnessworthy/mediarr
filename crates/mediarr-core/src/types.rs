use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Media Types
// ---------------------------------------------------------------------------

/// Type of media content detected by the parser.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MediaType {
    /// Feature film.
    #[default]
    Movie,
    /// TV series episode (includes anime).
    Series,
}

impl fmt::Display for MediaType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MediaType::Movie => write!(f, "Movie"),
            MediaType::Series => write!(f, "Series"),
        }
    }
}

/// Parser confidence in the result.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum ParseConfidence {
    /// High confidence — unambiguous parse.
    #[default]
    High,
    /// Medium confidence — reasonable guess with some ambiguity.
    Medium,
    /// Low confidence — significant uncertainty in the result.
    Low,
}

impl ParseConfidence {
    /// Returns true if self is strictly higher confidence than other.
    /// Used for D-01: confidence-based resolution with file-wins tiebreaker.
    pub fn is_higher_than(&self, other: &Self) -> bool {
        self.rank() > other.rank()
    }

    fn rank(&self) -> u8 {
        match self {
            ParseConfidence::Low => 0,
            ParseConfidence::Medium => 1,
            ParseConfidence::High => 2,
        }
    }
}

/// Parsed metadata from a media filename.
///
/// Produced by the parser module after running `hunch` over a filename.
/// All optional fields are `None` when the parser could not extract them.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MediaInfo {
    /// Extracted title of the media.
    pub title: String,
    /// Detected media type (Movie or Series).
    pub media_type: MediaType,
    /// Release year, if detected.
    pub year: Option<u16>,
    /// Season number, if detected (series).
    pub season: Option<u16>,
    /// Episode numbers. May contain multiple for multi-episode files.
    pub episodes: Vec<u16>,
    /// Video resolution (e.g. "1080p", "2160p").
    pub resolution: Option<String>,
    /// Video codec (e.g. "x264", "x265", "HEVC").
    pub video_codec: Option<String>,
    /// Audio codec (e.g. "AAC", "DTS-HD MA").
    pub audio_codec: Option<String>,
    /// Source type (e.g. "BluRay", "WEB-DL", "HDTV").
    pub source: Option<String>,
    /// Release group name.
    pub release_group: Option<String>,
    /// File container/extension (e.g. "mkv", "mp4").
    pub container: String,
    /// Content language, if detected.
    pub language: Option<String>,
    /// Parser confidence in the overall result.
    pub confidence: ParseConfidence,
}

/// Metadata parsed from parent folder names surrounding a video file.
/// Used to fill gaps in file-level parse results during scanning.
/// Transient -- not stored in ScanResult or serialized.
#[derive(Debug, Clone, Default)]
pub struct FolderContext {
    /// Parsed metadata from the immediate parent directory.
    pub parent: Option<MediaInfo>,
    /// Parsed metadata from the grandparent directory (two levels up from file).
    pub grandparent: Option<MediaInfo>,
}

// ---------------------------------------------------------------------------
// Scan Types
// ---------------------------------------------------------------------------

/// Status of a scan result entry.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ScanStatus {
    /// Parse succeeded with high confidence.
    Ok,
    /// Parse produced a result but with ambiguity.
    Ambiguous,
    /// Target path conflicts with an existing file or another scan result.
    Conflict,
    /// Parse or processing failed.
    Error,
}

/// A single scan result pairing a source file with its proposed rename.
///
/// Contains the parsed metadata, proposed output path, discovered subtitles,
/// and status flags for ambiguity or conflicts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    /// Original file path on disk.
    pub source_path: PathBuf,
    /// Parsed metadata from the filename.
    pub media_info: MediaInfo,
    /// Proposed destination path after template application.
    pub proposed_path: PathBuf,
    /// Subtitle files discovered for this video.
    pub subtitles: Vec<SubtitleMatch>,
    /// Overall status of this scan entry.
    pub status: ScanStatus,
    /// Human-readable reason if status is `Ambiguous`.
    pub ambiguity_reason: Option<String>,
    /// Alternative parse interpretations, if any.
    pub alternatives: Vec<MediaInfo>,
}

// ---------------------------------------------------------------------------
// Subtitle Types
// ---------------------------------------------------------------------------

/// A discovered subtitle file matched to a parent video.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleMatch {
    /// Original subtitle file path.
    pub source_path: PathBuf,
    /// Proposed destination path (derived from parent video's output name).
    pub proposed_path: PathBuf,
    /// ISO 639 language code (e.g. "en", "und" if undetected).
    pub language: String,
    /// Subtitle type indicator (forced, sdh, etc.), if detected.
    pub subtitle_type: Option<SubtitleType>,
    /// How this subtitle was discovered.
    pub discovery_method: DiscoveryMethod,
    /// Whether this subtitle is part of a VobSub pair (.idx/.sub).
    pub is_vobsub_pair: bool,
    /// For VobSub: the companion file (.idx or .sub).
    pub companion_path: Option<PathBuf>,
}

/// Subtitle type indicators.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SubtitleType {
    /// Forced subtitles (e.g. foreign language segments only).
    Forced,
    /// Subtitles for the deaf and hard of hearing (SDH).
    Sdh,
    /// Hearing impaired subtitles.
    Hi,
    /// Commentary track subtitles.
    Commentary,
}

impl fmt::Display for SubtitleType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SubtitleType::Forced => write!(f, "forced"),
            SubtitleType::Sdh => write!(f, "sdh"),
            SubtitleType::Hi => write!(f, "hi"),
            SubtitleType::Commentary => write!(f, "commentary"),
        }
    }
}

/// How a subtitle was discovered relative to its parent video.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DiscoveryMethod {
    /// Same directory, matching filename stem.
    Sidecar,
    /// Found in a `Subs` or `Subtitles` subfolder.
    SubsSubfolder,
    /// Found in a language-named subfolder (e.g. `English/`).
    NestedLanguage,
    /// VobSub pair discovery (.idx + .sub).
    VobSub,
}

/// Toggles for which subtitle discovery methods are active.
///
/// Lives in `types.rs` so both `config` and `subtitle` modules share one
/// definition, avoiding type duplication.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiscoveryToggles {
    /// Enable sidecar subtitle discovery.
    pub sidecar: bool,
    /// Enable Subs/Subtitles subfolder discovery.
    pub subs_subfolder: bool,
    /// Enable nested language folder discovery.
    pub nested_language_folders: bool,
    /// Enable VobSub pair (.idx/.sub) discovery.
    pub vobsub_pairs: bool,
}

impl Default for DiscoveryToggles {
    fn default() -> Self {
        Self {
            sidecar: true,
            subs_subfolder: true,
            nested_language_folders: true,
            vobsub_pairs: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Rename Types
// ---------------------------------------------------------------------------

/// How to handle conflicting target paths.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConflictStrategy {
    /// Skip conflicting files, leave unprocessed (D-12 default).
    #[default]
    Skip,
    /// Overwrite existing file at target.
    Overwrite,
    /// Append numeric suffix: "file (1).ext", "file (2).ext".
    NumericSuffix,
}

/// Whether to move or copy files during rename.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum RenameOperation {
    /// Move files (D-11 default). EXDEV falls back to copy+verify+remove.
    #[default]
    Move,
    /// Copy files, leaving source in place.
    Copy,
}

/// Result of a single rename operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameResult {
    /// Original file path.
    pub source_path: PathBuf,
    /// Destination file path.
    pub dest_path: PathBuf,
    /// Whether the rename succeeded.
    pub success: bool,
    /// Error message if the rename failed.
    pub error: Option<String>,
}

/// What to do with non-preferred subtitle languages.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum NonPreferredAction {
    /// Leave non-preferred subtitles in place (default).
    #[default]
    Ignore,
    /// Move non-preferred subtitles to a backup path.
    Backup,
    /// Rename all subtitles regardless of preference.
    KeepAll,
    /// Flag non-preferred subtitles for user review.
    Review,
}

// ---------------------------------------------------------------------------
// History Types
// ---------------------------------------------------------------------------

/// A single rename record for history storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameRecord {
    /// Batch ID grouping this rename with others in the same operation.
    pub batch_id: String,
    /// ISO 8601 timestamp of the rename.
    pub timestamp: String,
    /// Original file path before rename.
    pub source_path: PathBuf,
    /// Destination file path after rename.
    pub dest_path: PathBuf,
    /// Parsed metadata at time of rename (stored as JSON in SQLite).
    pub media_info: MediaInfo,
    /// File size in bytes at time of rename.
    pub file_size: u64,
    /// File modification time (ISO 8601) at time of rename.
    pub file_mtime: String,
}

/// Summary of a rename batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchSummary {
    /// Unique batch identifier.
    pub batch_id: String,
    /// ISO 8601 timestamp of the batch.
    pub timestamp: String,
    /// Number of files in this batch.
    pub file_count: usize,
    /// Individual rename records in this batch.
    pub entries: Vec<RenameRecord>,
}

/// Eligibility status for undoing a batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoEligibility {
    /// Whether the batch can be undone.
    pub eligible: bool,
    /// The batch being checked.
    pub batch_id: String,
    /// Reasons specific files cannot be undone (empty if fully eligible).
    pub ineligible_reasons: Vec<UndoIssue>,
}

/// Reason a specific file in a batch cannot be undone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoIssue {
    /// The destination path that cannot be reversed.
    pub dest_path: PathBuf,
    /// Human-readable reason for ineligibility.
    pub reason: String,
}

// ---------------------------------------------------------------------------
// Template Types
// ---------------------------------------------------------------------------

/// Warning produced during template validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateWarning {
    /// The variable that triggered the warning.
    pub variable: String,
    /// Human-readable warning message.
    pub message: String,
}

// ---------------------------------------------------------------------------
// Filter Types
// ---------------------------------------------------------------------------

/// Filter criteria for scan results (SCAN-05).
///
/// All fields are optional. When set, a `ScanResult` must match all active
/// criteria to pass the filter.
#[derive(Debug, Clone, Default)]
pub struct ScanFilter {
    /// Filter by media type.
    pub media_type: Option<MediaType>,
    /// Filter by scan status.
    pub status: Option<ScanStatus>,
    /// Case-insensitive substring search on the title.
    pub title_search: Option<String>,
}

impl ScanFilter {
    /// Returns true if the given `ScanResult` matches all active filter criteria.
    pub fn matches(&self, result: &ScanResult) -> bool {
        if let Some(mt) = self.media_type {
            if result.media_info.media_type != mt {
                return false;
            }
        }
        if let Some(st) = self.status {
            if result.status != st {
                return false;
            }
        }
        if let Some(ref search) = self.title_search {
            let search_lower = search.to_lowercase();
            if !result
                .media_info
                .title
                .to_lowercase()
                .contains(&search_lower)
            {
                return false;
            }
        }
        true
    }
}

// ---------------------------------------------------------------------------
// Watcher Types
// ---------------------------------------------------------------------------

/// Configuration for a single watched folder (per D-04, D-08).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WatcherConfig {
    /// Path to watch for new files.
    pub path: PathBuf,
    /// Operating mode: auto-rename or queue for review.
    pub mode: WatcherMode,
    /// Whether this watcher is active.
    pub active: bool,
    /// Debounce duration in seconds (default 5).
    pub debounce_seconds: u64,
    /// Per-watcher setting overrides. None or absent = use all global defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<WatcherSettings>,
}

/// Per-watcher setting overrides. Every field is optional; None means
/// "use the global default from Config". Stored as `[watchers.settings]`
/// in TOML.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct WatcherSettings {
    /// Override output directory. None = use global. Empty string "" = force in-place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_dir: Option<String>,
    /// Override rename operation (Move/Copy).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation: Option<RenameOperation>,
    /// Override conflict strategy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflict_strategy: Option<ConflictStrategy>,
    /// Override create_directories.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub create_directories: Option<bool>,
    /// Override movie template.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub movie_template: Option<String>,
    /// Override series template.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series_template: Option<String>,
    /// Override subtitle enabled toggle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitles_enabled: Option<bool>,
    /// Override preferred languages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_languages: Option<Vec<String>>,
    /// Override non-preferred action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub non_preferred_action: Option<NonPreferredAction>,
}

impl WatcherSettings {
    /// Returns true if no fields are overridden.
    pub fn is_empty(&self) -> bool {
        self.output_dir.is_none()
            && self.operation.is_none()
            && self.conflict_strategy.is_none()
            && self.create_directories.is_none()
            && self.movie_template.is_none()
            && self.series_template.is_none()
            && self.subtitles_enabled.is_none()
            && self.preferred_languages.is_none()
            && self.non_preferred_action.is_none()
    }
}

/// Watcher operating mode (per WATC-02, WATC-03).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WatcherMode {
    /// Scan and rename automatically.
    Auto,
    /// Scan and queue for user review.
    Review,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            mode: WatcherMode::Auto,
            active: true,
            debounce_seconds: 5,
            settings: None,
        }
    }
}

impl fmt::Display for WatcherMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WatcherMode::Auto => write!(f, "auto"),
            WatcherMode::Review => write!(f, "review"),
        }
    }
}

/// Action taken by the watcher on a file event (per D-06).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WatcherAction {
    /// File was renamed successfully.
    Renamed,
    /// File was queued for review.
    Queued,
    /// An error occurred processing the file.
    Error,
}

impl fmt::Display for WatcherAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WatcherAction::Renamed => write!(f, "renamed"),
            WatcherAction::Queued => write!(f, "queued"),
            WatcherAction::Error => write!(f, "error"),
        }
    }
}

/// A logged watcher event (per D-06, WATC-05).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherEvent {
    /// Database row ID (None for new events not yet inserted).
    pub id: Option<i64>,
    /// ISO 8601 timestamp of the event.
    pub timestamp: String,
    /// Path of the watched folder that triggered this event.
    pub watch_path: PathBuf,
    /// Filename that was detected.
    pub filename: String,
    /// Action taken on the file.
    pub action: WatcherAction,
    /// Detail string (target path for renamed, error message for errors).
    pub detail: Option<String>,
    /// Associated rename batch ID, if applicable.
    pub batch_id: Option<String>,
}

/// Review queue status values (per D-10).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReviewStatus {
    /// Awaiting user review.
    Pending,
    /// User approved the rename.
    Approved,
    /// User rejected the rename.
    Rejected,
}

impl fmt::Display for ReviewStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReviewStatus::Pending => write!(f, "pending"),
            ReviewStatus::Approved => write!(f, "approved"),
            ReviewStatus::Rejected => write!(f, "rejected"),
        }
    }
}

/// An entry in the review queue (per D-10, D-11).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewQueueEntry {
    /// Database row ID (None for new entries not yet inserted).
    pub id: Option<i64>,
    /// ISO 8601 timestamp when the entry was queued.
    pub timestamp: String,
    /// Path of the watched folder that triggered this entry.
    pub watch_path: PathBuf,
    /// Original source file path.
    pub source_path: PathBuf,
    /// Proposed destination path after template application.
    pub proposed_path: PathBuf,
    /// Serialised MediaInfo as JSON string.
    pub media_info_json: String,
    /// Serialised subtitle matches as JSON string.
    pub subtitles_json: String,
    /// Current review status.
    pub status: ReviewStatus,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // -----------------------------------------------------------------------
    // Helper: build a minimal ScanResult for filter tests
    // -----------------------------------------------------------------------

    fn scan_result(title: &str, media_type: MediaType, status: ScanStatus) -> ScanResult {
        ScanResult {
            source_path: PathBuf::from(format!("/src/{title}.mkv")),
            media_info: MediaInfo {
                title: title.to_string(),
                media_type,
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
            },
            proposed_path: PathBuf::from(format!("/dst/{title}.mkv")),
            subtitles: vec![],
            status,
            ambiguity_reason: None,
            alternatives: vec![],
        }
    }

    // -----------------------------------------------------------------------
    // ScanFilter::matches
    // -----------------------------------------------------------------------

    #[test]
    fn filter_default_matches_everything() {
        let filter = ScanFilter::default();
        let result = scan_result("Inception", MediaType::Movie, ScanStatus::Ok);
        assert!(filter.matches(&result));
    }

    #[test]
    fn filter_by_media_type_matches() {
        let filter = ScanFilter {
            media_type: Some(MediaType::Movie),
            ..ScanFilter::default()
        };
        let movie = scan_result("Inception", MediaType::Movie, ScanStatus::Ok);
        let series = scan_result("The Office", MediaType::Series, ScanStatus::Ok);
        assert!(filter.matches(&movie));
        assert!(!filter.matches(&series));
    }

    #[test]
    fn filter_by_status_matches() {
        let filter = ScanFilter {
            status: Some(ScanStatus::Conflict),
            ..ScanFilter::default()
        };
        let ok = scan_result("Inception", MediaType::Movie, ScanStatus::Ok);
        let conflict = scan_result("Inception", MediaType::Movie, ScanStatus::Conflict);
        assert!(!filter.matches(&ok));
        assert!(filter.matches(&conflict));
    }

    #[test]
    fn filter_by_title_search_case_insensitive() {
        let filter = ScanFilter {
            title_search: Some("inception".to_string()),
            ..ScanFilter::default()
        };
        let result = scan_result("Inception", MediaType::Movie, ScanStatus::Ok);
        assert!(filter.matches(&result));
    }

    #[test]
    fn filter_by_title_search_substring() {
        let filter = ScanFilter {
            title_search: Some("cept".to_string()),
            ..ScanFilter::default()
        };
        let result = scan_result("Inception", MediaType::Movie, ScanStatus::Ok);
        assert!(filter.matches(&result));
    }

    #[test]
    fn filter_by_title_search_no_match() {
        let filter = ScanFilter {
            title_search: Some("nonexistent".to_string()),
            ..ScanFilter::default()
        };
        let result = scan_result("Inception", MediaType::Movie, ScanStatus::Ok);
        assert!(!filter.matches(&result));
    }

    #[test]
    fn filter_combined_all_criteria_must_match() {
        let filter = ScanFilter {
            media_type: Some(MediaType::Movie),
            status: Some(ScanStatus::Ok),
            title_search: Some("inception".to_string()),
        };
        let matching = scan_result("Inception", MediaType::Movie, ScanStatus::Ok);
        assert!(filter.matches(&matching));

        // Wrong media type
        let wrong_type = scan_result("Inception", MediaType::Series, ScanStatus::Ok);
        assert!(!filter.matches(&wrong_type));

        // Wrong status
        let wrong_status = scan_result("Inception", MediaType::Movie, ScanStatus::Conflict);
        assert!(!filter.matches(&wrong_status));

        // Wrong title
        let wrong_title = scan_result("The Office", MediaType::Movie, ScanStatus::Ok);
        assert!(!filter.matches(&wrong_title));
    }

    // -----------------------------------------------------------------------
    // Display implementations
    // -----------------------------------------------------------------------

    #[test]
    fn media_type_display() {
        assert_eq!(MediaType::Movie.to_string(), "Movie");
        assert_eq!(MediaType::Series.to_string(), "Series");
    }

    #[test]
    fn subtitle_type_display() {
        assert_eq!(SubtitleType::Forced.to_string(), "forced");
        assert_eq!(SubtitleType::Sdh.to_string(), "sdh");
        assert_eq!(SubtitleType::Hi.to_string(), "hi");
        assert_eq!(SubtitleType::Commentary.to_string(), "commentary");
    }

    #[test]
    fn watcher_mode_display() {
        assert_eq!(WatcherMode::Auto.to_string(), "auto");
        assert_eq!(WatcherMode::Review.to_string(), "review");
    }

    #[test]
    fn watcher_action_display() {
        assert_eq!(WatcherAction::Renamed.to_string(), "renamed");
        assert_eq!(WatcherAction::Queued.to_string(), "queued");
        assert_eq!(WatcherAction::Error.to_string(), "error");
    }

    #[test]
    fn review_status_display() {
        assert_eq!(ReviewStatus::Pending.to_string(), "pending");
        assert_eq!(ReviewStatus::Approved.to_string(), "approved");
        assert_eq!(ReviewStatus::Rejected.to_string(), "rejected");
    }

    // -----------------------------------------------------------------------
    // Default implementations
    // -----------------------------------------------------------------------

    #[test]
    fn discovery_toggles_default_all_true() {
        let toggles = DiscoveryToggles::default();
        assert!(toggles.sidecar);
        assert!(toggles.subs_subfolder);
        assert!(toggles.nested_language_folders);
        assert!(toggles.vobsub_pairs);
    }

    #[test]
    fn conflict_strategy_default_is_skip() {
        assert_eq!(ConflictStrategy::default(), ConflictStrategy::Skip);
    }

    #[test]
    fn rename_operation_default_is_move() {
        assert_eq!(RenameOperation::default(), RenameOperation::Move);
    }

    #[test]
    fn non_preferred_action_default_is_ignore() {
        assert_eq!(NonPreferredAction::default(), NonPreferredAction::Ignore);
    }

    #[test]
    fn watcher_config_default_values() {
        let wc = WatcherConfig::default();
        assert_eq!(wc.path, PathBuf::new());
        assert_eq!(wc.mode, WatcherMode::Auto);
        assert!(wc.active);
        assert_eq!(wc.debounce_seconds, 5);
        assert!(wc.settings.is_none());
    }

    // -----------------------------------------------------------------------
    // WatcherSettings tests
    // -----------------------------------------------------------------------

    #[test]
    fn watcher_settings_default_all_none() {
        let s = WatcherSettings::default();
        assert!(s.output_dir.is_none());
        assert!(s.operation.is_none());
        assert!(s.conflict_strategy.is_none());
        assert!(s.create_directories.is_none());
        assert!(s.movie_template.is_none());
        assert!(s.series_template.is_none());
        assert!(s.subtitles_enabled.is_none());
        assert!(s.preferred_languages.is_none());
        assert!(s.non_preferred_action.is_none());
    }

    #[test]
    fn watcher_settings_is_empty_when_all_none() {
        let s = WatcherSettings::default();
        assert!(s.is_empty());
    }

    #[test]
    fn watcher_settings_is_empty_false_when_any_set() {
        let mut s = WatcherSettings::default();
        s.output_dir = Some("/override".to_string());
        assert!(!s.is_empty());

        let mut s2 = WatcherSettings::default();
        s2.subtitles_enabled = Some(false);
        assert!(!s2.is_empty());
    }

    #[test]
    fn watcher_config_without_settings_toml_backward_compat() {
        let toml_str = r#"
path = "/watch/test"
mode = "auto"
active = true
debounce_seconds = 5
"#;
        let wc: WatcherConfig = toml::from_str(toml_str).expect("backward compat parse");
        assert_eq!(wc.path, PathBuf::from("/watch/test"));
        assert!(wc.settings.is_none());
    }

    #[test]
    fn watcher_config_with_settings_toml_round_trip() {
        let wc = WatcherConfig {
            path: PathBuf::from("/watch/movies"),
            mode: WatcherMode::Auto,
            active: true,
            debounce_seconds: 5,
            settings: Some(WatcherSettings {
                output_dir: Some("/custom/output".to_string()),
                operation: Some(RenameOperation::Copy),
                ..WatcherSettings::default()
            }),
        };

        let toml_str = toml::to_string_pretty(&wc).expect("serialize");
        let restored: WatcherConfig = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(wc, restored);
        assert!(restored.settings.is_some());
        let s = restored.settings.unwrap();
        assert_eq!(s.output_dir, Some("/custom/output".to_string()));
        assert_eq!(s.operation, Some(RenameOperation::Copy));
        assert!(s.conflict_strategy.is_none());
    }

    // -----------------------------------------------------------------------
    // ParseConfidence ordering tests
    // -----------------------------------------------------------------------

    #[test]
    fn confidence_high_is_higher_than_medium() {
        assert!(ParseConfidence::High.is_higher_than(&ParseConfidence::Medium));
    }

    #[test]
    fn confidence_medium_is_higher_than_low() {
        assert!(ParseConfidence::Medium.is_higher_than(&ParseConfidence::Low));
    }

    #[test]
    fn confidence_equal_is_not_higher() {
        assert!(!ParseConfidence::High.is_higher_than(&ParseConfidence::High));
        assert!(!ParseConfidence::Medium.is_higher_than(&ParseConfidence::Medium));
        assert!(!ParseConfidence::Low.is_higher_than(&ParseConfidence::Low));
    }

    #[test]
    fn confidence_low_is_not_higher_than_medium() {
        assert!(!ParseConfidence::Low.is_higher_than(&ParseConfidence::Medium));
    }

    #[test]
    fn folder_context_default_is_empty() {
        let ctx = FolderContext::default();
        assert!(ctx.parent.is_none());
        assert!(ctx.grandparent.is_none());
    }
}
