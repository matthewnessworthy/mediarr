<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { listen } from '@tauri-apps/api/event';
	import { Plus } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';
	import { watcherState } from '$lib/state/watcher.svelte';
	import WatcherCard from '$lib/components/watcher/WatcherCard.svelte';
	import ActivityLog from '$lib/components/watcher/ActivityLog.svelte';
	import AddWatcherDialog from '$lib/components/watcher/AddWatcherDialog.svelte';
	import type { WatcherConfig, WatcherEvent } from '$lib/types';

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
		} finally {
			watcherState.loading = false;
		}
	}

	onMount(async () => {
		await loadWatchers();

		// Listen for real-time watcher events per D-09
		unlisten = await listen<WatcherEvent>('watcher-event', (event) => {
			watcherState.events = [event.payload, ...watcherState.events].slice(0, 100);
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
		<Button variant="outline" size="sm" onclick={() => (addDialogOpen = true)}>
			<Plus class="size-3.5" />
			Add Folder
		</Button>
	</div>

	{#if watcherState.loading}
		<div class="space-y-4">
			{#each { length: 3 } as _}
				<div class="flex items-center gap-3 py-3">
					<div class="size-2 animate-pulse rounded-full bg-muted"></div>
					<div class="flex-1 space-y-2">
						<div class="h-4 w-56 animate-pulse rounded bg-muted"></div>
						<div class="h-3 w-32 animate-pulse rounded bg-muted"></div>
					</div>
					<div class="h-5 w-14 animate-pulse rounded bg-muted"></div>
				</div>
			{/each}
		</div>
	{:else if watcherState.watchers.length === 0}
		<div class="py-16 text-center">
			<p class="text-sm text-muted-foreground">
				No folders being watched. Add a folder to start monitoring for new media files.
			</p>
		</div>
	{:else}
		<div class="mb-8">
			{#each watcherState.watchers as watcher (watcher.path)}
				<WatcherCard {watcher} eventCounts={eventCountsFor(watcher.path)} />
			{/each}
		</div>

		<div>
			<h3 class="mb-3 text-sm font-medium text-muted-foreground">Activity</h3>
			<ActivityLog events={watcherState.events} />
		</div>
	{/if}
</div>

<AddWatcherDialog bind:open={addDialogOpen} onAdded={loadWatchers} />
