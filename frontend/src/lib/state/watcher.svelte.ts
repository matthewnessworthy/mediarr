import type { WatcherConfig, WatcherEvent, ReviewQueueEntry } from '$lib/types';

class WatcherState {
	watchers = $state<WatcherConfig[]>([]);
	events = $state<WatcherEvent[]>([]);
	reviewQueue = $state<ReviewQueueEntry[]>([]);
	loading = $state(false);
}

export const watcherState = new WatcherState();
