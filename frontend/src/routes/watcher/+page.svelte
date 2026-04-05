<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { listen } from '@tauri-apps/api/event';
	import { Plus, Eye } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';
	import { watcherState } from '$lib/state/watcher.svelte';
	import WatcherCard from '$lib/components/watcher/WatcherCard.svelte';
	import ActivityLog from '$lib/components/watcher/ActivityLog.svelte';
	import ReviewQueue from '$lib/components/watcher/ReviewQueue.svelte';
	import AddWatcherDialog from '$lib/components/watcher/AddWatcherDialog.svelte';
	import type { WatcherConfig, WatcherEvent, ReviewQueueEntry } from '$lib/types';

	let unlisten: (() => void) | null = null;
	let addDialogOpen = $state(false);

	function eventCountsFor(watchPath: string) {
		const matching = watcherState.events.filter((e) => e.watch_path === watchPath);
		return {
			processed: matching.filter((e) => e.action === 'renamed').length,
			errors: matching.filter((e) => e.action === 'error').length,
			pending: matching.filter((e) => e.action === 'queued').length,
		};
	}

	async function loadWatchers() {
		watcherState.loading = true;
		try {
			watcherState.watchers = await invoke<WatcherConfig[]>('list_watchers');
			watcherState.events = await invoke<WatcherEvent[]>('list_watcher_events', {
				watchPath: null,
				limit: 50,
			});
			watcherState.reviewQueue = await invoke<ReviewQueueEntry[]>('list_review_queue', {
				watchPath: null,
			});
		} finally {
			watcherState.loading = false;
		}
	}

	async function refreshReviewQueue() {
		watcherState.reviewQueue = await invoke<ReviewQueueEntry[]>('list_review_queue', {
			watchPath: null,
		});
	}

	onMount(async () => {
		await loadWatchers();

		// Listen for real-time watcher events per D-09
		unlisten = await listen<WatcherEvent>('watcher-event', (event) => {
			watcherState.events = [event.payload, ...watcherState.events].slice(0, 100);
			// Refresh review queue when new items are queued
			if (event.payload.action === 'queued') {
				refreshReviewQueue();
			}
		});
	});

	onDestroy(() => unlisten?.());
</script>

<div class="p-8">
	<div class="mb-6 flex items-center justify-between">
		<div class="flex items-baseline gap-3">
			<h2 class="text-lg font-medium text-foreground">Watcher</h2>
			{#if watcherState.watchers.length > 0}
				<span class="text-xs text-muted-foreground">
					{watcherState.watchers.length} folder{watcherState.watchers.length === 1 ? '' : 's'} monitored
				</span>
			{/if}
		</div>
		<Button variant="outline" size="sm" onclick={() => (addDialogOpen = true)} class="focus-ring">
			<Plus class="size-3.5" />
			Add Folder
		</Button>
	</div>

	{#if watcherState.loading}
		<div class="space-y-1">
			{#each { length: 3 } as _, i}
				<div class="flex items-center gap-3 py-3 border-b border-border/50">
					<div class="skeleton size-2 rounded-full"></div>
					<div class="flex-1 space-y-2">
						<div class="skeleton h-4" style="width: {180 + i * 40}px;"></div>
						<div class="skeleton h-3" style="width: {100 + i * 20}px;"></div>
					</div>
					<div class="skeleton h-5 w-20 rounded-full"></div>
					<div class="skeleton h-5 w-9 rounded-full"></div>
				</div>
			{/each}
		</div>
	{:else if watcherState.watchers.length === 0}
		<div class="flex flex-col items-center justify-center py-24">
			<Eye class="size-10 mb-5 text-muted-foreground/30" />
			<p class="text-sm text-muted-foreground text-center leading-relaxed max-w-xs">
				No folders being watched. Add a folder to start monitoring for new media files.
			</p>
		</div>
	{:else}
		<div class="mb-8">
			{#each watcherState.watchers as watcher (watcher.path)}
				<WatcherCard {watcher} eventCounts={eventCountsFor(watcher.path)} />
			{/each}
		</div>

		{#if watcherState.reviewQueue.filter(e => e.status === 'pending').length > 0}
			<div class="mb-8">
				<h3 class="mb-3 text-sm font-medium text-muted-foreground">Review Queue</h3>
				<ReviewQueue
					entries={watcherState.reviewQueue.filter(e => e.status === 'pending')}
					onChanged={refreshReviewQueue}
				/>
			</div>
		{/if}

		<div>
			<h3 class="mb-3 text-sm font-medium text-muted-foreground">Activity</h3>
			<ActivityLog events={watcherState.events} />
		</div>
	{/if}
</div>

<AddWatcherDialog bind:open={addDialogOpen} onAdded={loadWatchers} />
