import type {
	MediaInfo,
	ScanResult,
	Config,
	BatchSummary,
	WatcherConfig,
	WatcherEvent,
	UndoEligibility,
	RenameResult,
} from '$lib/types';

export function mockMediaInfo(overrides: Partial<MediaInfo> = {}): MediaInfo {
	return {
		title: 'Test Movie',
		media_type: 'Movie',
		year: 2024,
		season: null,
		episodes: [],
		resolution: '1080p',
		video_codec: 'x264',
		audio_codec: null,
		source: 'BluRay',
		release_group: 'GROUP',
		container: 'mkv',
		language: null,
		confidence: 'High',
		...overrides,
	};
}

export function mockScanResult(overrides: Partial<ScanResult> = {}): ScanResult {
	return {
		source_path: '/media/Test.Movie.2024.1080p.mkv',
		media_info: mockMediaInfo(),
		proposed_path: '/output/Test Movie (2024)/Test Movie (2024).mkv',
		subtitles: [],
		status: 'Ok',
		ambiguity_reason: null,
		alternatives: [],
		...overrides,
	};
}

export function mockConfig(overrides: Partial<Config> = {}): Config {
	return {
		general: {
			output_dir: '/output',
			operation: 'Move',
			conflict_strategy: 'Skip',
			create_directories: true,
		},
		templates: {
			movie: '{title} ({year})/{title} ({year}).{ext}',
			series: '{title}/Season {season:02}/{title} - S{season:02}E{episode:02}.{ext}',
		},
		subtitles: {
			enabled: true,
			naming_pattern: '{title}.{language}.{type}.{ext}',
			discovery: {
				sidecar: true,
				subs_subfolder: true,
				nested_language_folders: true,
				vobsub_pairs: true,
			},
			preferred_languages: ['eng'],
			non_preferred_action: 'Ignore',
			backup_path: null,
		},
		watchers: [],
		...overrides,
	};
}

export function mockBatchSummary(overrides: Partial<BatchSummary> = {}): BatchSummary {
	return {
		batch_id: 'batch-001',
		timestamp: '2024-01-15T10:30:00Z',
		file_count: 3,
		entries: [],
		...overrides,
	};
}

export function mockWatcherConfig(overrides: Partial<WatcherConfig> = {}): WatcherConfig {
	return {
		path: '/watch/movies',
		mode: 'auto',
		active: true,
		debounce_seconds: 5,
		...overrides,
	};
}

export function mockWatcherEvent(overrides: Partial<WatcherEvent> = {}): WatcherEvent {
	return {
		id: 1,
		timestamp: '2024-01-15T10:30:00Z',
		watch_path: '/watch/movies',
		filename: 'Test.Movie.mkv',
		action: 'renamed',
		detail: null,
		batch_id: 'batch-001',
		...overrides,
	};
}

export function mockUndoEligibility(overrides: Partial<UndoEligibility> = {}): UndoEligibility {
	return {
		eligible: true,
		batch_id: 'batch-001',
		ineligible_reasons: [],
		...overrides,
	};
}

export function mockRenameResult(overrides: Partial<RenameResult> = {}): RenameResult {
	return {
		source_path: '/media/file.mkv',
		dest_path: '/output/file.mkv',
		success: true,
		error: null,
		...overrides,
	};
}
