import type { WatcherConfig, WatcherEvent, ReviewQueueEntry } from '$lib/types';

class WatcherState {
	watchers = $state<WatcherConfig[]>([]);
	events = $state<WatcherEvent[]>([]);
	reviewQueue = $state<ReviewQueueEntry[]>([]);
	loading = $state(false);
	error = $state<string | null>(null);
}

export const watcherState = new WatcherState();
