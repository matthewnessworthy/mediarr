import { describe, it, expect, beforeEach } from 'vitest';
import { flushSync } from 'svelte';
import { watcherState } from './watcher.svelte';
import { mockWatcherConfig, mockWatcherEvent } from '../../test/fixtures';

beforeEach(() => {
	watcherState.watchers = [];
	watcherState.events = [];
	watcherState.reviewQueue = [];
	watcherState.loading = false;
});

describe('WatcherState', () => {
	it('initial state has empty arrays and loading false', () => {
		expect(watcherState.watchers).toHaveLength(0);
		expect(watcherState.events).toHaveLength(0);
		expect(watcherState.reviewQueue).toHaveLength(0);
		expect(watcherState.loading).toBe(false);
	});

	it('watchers can be set and read back', () => {
		const w = mockWatcherConfig({ path: '/watch/series' });
		watcherState.watchers = [w];
		flushSync();
		expect(watcherState.watchers).toHaveLength(1);
		expect(watcherState.watchers[0].path).toBe('/watch/series');
	});

	it('events can be set and read back', () => {
		const e = mockWatcherEvent({ filename: 'new-file.mkv' });
		watcherState.events = [e];
		flushSync();
		expect(watcherState.events).toHaveLength(1);
		expect(watcherState.events[0].filename).toBe('new-file.mkv');
	});

	it('reviewQueue can be set and read back', () => {
		watcherState.reviewQueue = [
			{
				id: 1,
				timestamp: '2024-01-15T10:30:00Z',
				watch_path: '/watch',
				source_path: '/watch/file.mkv',
				proposed_path: '/output/file.mkv',
				media_info_json: '{}',
				subtitles_json: '[]',
				status: 'pending',
			},
		];
		flushSync();
		expect(watcherState.reviewQueue).toHaveLength(1);
		expect(watcherState.reviewQueue[0].status).toBe('pending');
	});
});
