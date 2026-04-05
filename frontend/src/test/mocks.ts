import type { ScanResult, Config, BatchSummary, WatcherConfig } from '$lib/types';
import {
	mockScanResult,
	mockConfig,
	mockUndoEligibility,
	mockRenameResult,
} from './fixtures';

/**
 * IPC handler factories for use with @tauri-apps/api/mocks mockIPC().
 * Each function returns a Record<string, handler> mapping command names to mock handlers.
 */

export function mockScanHandlers(results: ScanResult[] = [mockScanResult()]) {
	return {
		scan_folder: () => results,
		scan_folder_streaming: () => null,
		dry_run_renames: () => results.map(() => mockRenameResult()),
		execute_renames: () => results.map(() => mockRenameResult()),
	};
}

export function mockConfigHandlers(config: Config = mockConfig()) {
	return {
		get_config: () => config,
		update_config: () => null,
		preview_template: (_args: { template: string; media_info: { title: string } }) =>
			`Preview: ${_args.media_info.title}`,
		validate_template: () => [],
	};
}

export function mockHistoryHandlers(batches: BatchSummary[] = []) {
	return {
		list_batches: () => batches,
		check_undo: () => mockUndoEligibility(),
		execute_undo: () => [],
	};
}

export function mockWatcherHandlers(watchers: WatcherConfig[] = []) {
	return {
		list_watchers: () => watchers,
		list_watcher_events: () => [],
		list_review_queue: () => [],
		start_watcher: () => null,
		stop_watcher: () => null,
	};
}

export function allMockHandlers(
	overrides: Partial<Record<string, (args: any) => any>> = {},
): Record<string, (args: any) => any> {
	return {
		...mockScanHandlers(),
		...mockConfigHandlers(),
		...mockHistoryHandlers(),
		...mockWatcherHandlers(),
		...overrides,
	};
}
