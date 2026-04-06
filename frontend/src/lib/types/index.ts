// TypeScript mirrors of all Rust types used in Tauri IPC.
// Field names use snake_case to match Rust serde output (no rename_all = "camelCase").

// ---------------------------------------------------------------------------
// Enum types (matching Rust serde serialization)
// ---------------------------------------------------------------------------

export type MediaType = 'Movie' | 'Series' | 'Anime';
export type ScanStatus = 'Ok' | 'Ambiguous' | 'Conflict' | 'Error';
export type ParseConfidence = 'High' | 'Medium' | 'Low';
export type SubtitleType = 'Forced' | 'Sdh' | 'Hi' | 'Commentary';
export type DiscoveryMethod = 'Sidecar' | 'SubsSubfolder' | 'NestedLanguage' | 'VobSub';
export type RenameOperation = 'Move' | 'Copy';
export type ConflictStrategy = 'Skip' | 'Overwrite' | 'NumericSuffix';
export type NonPreferredAction = 'Ignore' | 'Backup' | 'KeepAll' | 'Review';

// Lowercase-serialized enums (Rust has #[serde(rename_all = "lowercase")])
export type WatcherMode = 'auto' | 'review';
export type WatcherAction = 'renamed' | 'queued' | 'error';
export type ReviewStatus = 'pending' | 'approved' | 'rejected';

// ---------------------------------------------------------------------------
// Media types
// ---------------------------------------------------------------------------

export interface MediaInfo {
	title: string;
	media_type: MediaType;
	year: number | null;
	season: number | null;
	episodes: number[];
	resolution: string | null;
	video_codec: string | null;
	audio_codec: string | null;
	source: string | null;
	release_group: string | null;
	container: string;
	language: string | null;
	confidence: ParseConfidence;
}

// ---------------------------------------------------------------------------
// Scan types
// ---------------------------------------------------------------------------

export interface SubtitleMatch {
	source_path: string;
	proposed_path: string;
	language: string;
	subtitle_type: SubtitleType | null;
	discovery_method: DiscoveryMethod;
	is_vobsub_pair: boolean;
	companion_path: string | null;
}

export interface ScanResult {
	source_path: string;
	media_info: MediaInfo;
	proposed_path: string;
	subtitles: SubtitleMatch[];
	status: ScanStatus;
	ambiguity_reason: string | null;
	alternatives: MediaInfo[];
}

// ---------------------------------------------------------------------------
// Rename types
// ---------------------------------------------------------------------------

export interface RenameResult {
	source_path: string;
	dest_path: string;
	success: boolean;
	error: string | null;
}

// ---------------------------------------------------------------------------
// History types
// ---------------------------------------------------------------------------

export interface RenameRecord {
	batch_id: string;
	timestamp: string;
	source_path: string;
	dest_path: string;
	media_info: MediaInfo;
	file_size: number;
	file_mtime: string;
}

export interface BatchSummary {
	batch_id: string;
	timestamp: string;
	file_count: number;
	entries: RenameRecord[];
}

export interface UndoIssue {
	dest_path: string;
	reason: string;
}

export interface UndoEligibility {
	eligible: boolean;
	batch_id: string;
	ineligible_reasons: UndoIssue[];
}

// ---------------------------------------------------------------------------
// Template types
// ---------------------------------------------------------------------------

export interface TemplateWarning {
	variable: string;
	message: string;
}

// ---------------------------------------------------------------------------
// Watcher types
// ---------------------------------------------------------------------------

export interface WatcherSettings {
	output_dir?: string | null;
	operation?: RenameOperation | null;
	conflict_strategy?: ConflictStrategy | null;
	create_directories?: boolean | null;
	movie_template?: string | null;
	series_template?: string | null;
	anime_template?: string | null;
	subtitles_enabled?: boolean | null;
	preferred_languages?: string[] | null;
	non_preferred_action?: NonPreferredAction | null;
}

export interface WatcherConfig {
	path: string;
	mode: WatcherMode;
	active: boolean;
	debounce_seconds: number;
	settings?: WatcherSettings | null;
}

export interface WatcherEvent {
	id: number | null;
	timestamp: string;
	watch_path: string;
	filename: string;
	action: WatcherAction;
	detail: string | null;
	batch_id: string | null;
}

export interface ReviewQueueEntry {
	id: number | null;
	timestamp: string;
	watch_path: string;
	source_path: string;
	proposed_path: string;
	media_info_json: string;
	subtitles_json: string;
	status: ReviewStatus;
}

// ---------------------------------------------------------------------------
// Config types
// ---------------------------------------------------------------------------

export interface DiscoveryToggles {
	sidecar: boolean;
	subs_subfolder: boolean;
	nested_language_folders: boolean;
	vobsub_pairs: boolean;
}

export interface GeneralConfig {
	output_dir: string | null;
	operation: RenameOperation;
	conflict_strategy: ConflictStrategy;
	create_directories: boolean;
}

export interface TemplateConfig {
	movie: string;
	series: string;
	anime: string;
}

export interface SubtitleConfig {
	enabled: boolean;
	naming_pattern: string;
	discovery: DiscoveryToggles;
	preferred_languages: string[];
	non_preferred_action: NonPreferredAction;
	backup_path: string | null;
}

export interface Config {
	general: GeneralConfig;
	templates: TemplateConfig;
	subtitles: SubtitleConfig;
	watchers: WatcherConfig[];
}

// ---------------------------------------------------------------------------
// Scan streaming event types (match ScanEvent enum from Rust)
// ---------------------------------------------------------------------------

export type ScanEvent =
	| { event: 'progress'; data: { scanned: number; total_estimate: number } }
	| { event: 'result'; data: { scan_result: ScanResult } }
	| { event: 'complete'; data: { total: number } }
	| { event: 'error'; data: { message: string } };
