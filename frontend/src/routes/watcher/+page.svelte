<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { listen } from '@tauri-apps/api/event';
	import { stat } from '@tauri-apps/plugin-fs';
	import { Plus, Eye, Settings2 } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Sheet from '$lib/components/ui/sheet';
	import { watcherState } from '$lib/state/watcher.svelte';
	import WatcherCard from '$lib/components/watcher/WatcherCard.svelte';
	import ActivityLog from '$lib/components/watcher/ActivityLog.svelte';
	import ReviewQueue from '$lib/components/watcher/ReviewQueue.svelte';
	import AddWatcherDialog from '$lib/components/watcher/AddWatcherDialog.svelte';
	import WatcherSettingsEditor from '$lib/components/watcher/WatcherSettingsEditor.svelte';
	import type { WatcherConfig, WatcherEvent, ReviewQueueEntry, Config, WatcherMode, WatcherSettings } from '$lib/types';

	let unlisten: (() => void) | null = null;
	let unlistenDrag: (() => void) | null = null;
	let addDialogOpen = $state(false);
	let dragInitialPath = $state('');
	let dragOver = $state(false);

	// Edit dialog state
	let editDialogOpen = $state(false);
	let editingWatcher = $state<WatcherConfig | null>(null);
	let editSettings = $state<WatcherSettings>({});
	let editMode = $state<WatcherMode>('auto');
	let editDebounce = $state(5);
	let editSaving = $state(false);
	let editError = $state('');
	let editGlobalConfig = $state<Config | null>(null);

	// Clear drag path when dialog closes (Cancel or sheet dismiss)
	$effect(() => {
		if (!addDialogOpen && dragInitialPath) {
			dragInitialPath = '';
		}
	});

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
				watch_path: null,
				limit: 50,
			});
			watcherState.reviewQueue = await invoke<ReviewQueueEntry[]>('list_review_queue', {
				watch_path: null,
			});
		} catch (e) {
			console.error('Failed to load watcher data:', e);
		} finally {
			watcherState.loading = false;
		}
	}

	async function refreshReviewQueue() {
		watcherState.reviewQueue = await invoke<ReviewQueueEntry[]>('list_review_queue', {
			watch_path: null,
		});
	}

	async function removeWatcher(path: string) {
		try {
			const config = await invoke<import('$lib/types').Config>('get_config');
			config.watchers = config.watchers.filter((w: import('$lib/types').WatcherConfig) => w.path !== path);
			await invoke('update_config', { config });
			await loadWatchers();
		} catch (e) {
			console.error('Failed to remove watcher:', e);
		}
	}

	async function openEditDialog(path: string) {
		const watcher = watcherState.watchers.find(w => w.path === path);
		if (!watcher) return;
		editingWatcher = watcher;
		editMode = watcher.mode;
		editDebounce = watcher.debounce_seconds;
		editSettings = watcher.settings ? { ...watcher.settings } : {};
		editGlobalConfig = await invoke<Config>('get_config');
		editError = '';
		editDialogOpen = true;
	}

	async function handleEditSave() {
		if (!editingWatcher) return;
		editSaving = true;
		editError = '';
		try {
			const config = await invoke<Config>('get_config');
			const hasOverrides = Object.values(editSettings).some(v => v != null);
			config.watchers = config.watchers.map(w =>
				w.path === editingWatcher!.path
					? {
						...w,
						mode: editMode,
						debounce_seconds: editDebounce,
						...(hasOverrides ? { settings: editSettings } : { settings: null }),
					}
					: w
			);
			await invoke('update_config', { config });
			editDialogOpen = false;
			await loadWatchers();
		} catch (e) {
			editError = String(e);
		} finally {
			editSaving = false;
		}
	}

	async function isDirectory(path: string): Promise<boolean> {
		try {
			const info = await stat(path);
			return info.isDirectory;
		} catch {
			// Fallback: if stat fails, assume directory if no file extension
			const name = path.split(/[\\/]/).pop() || '';
			return !/\.\w{2,5}$/.test(name);
		}
	}

	onMount(async () => {
		// Set up event listeners first — these must work even if data loading fails
		unlisten = await listen<WatcherEvent>('watcher-event', (event) => {
			watcherState.events = [event.payload, ...watcherState.events].slice(0, 100);
			// Refresh review queue when new items are queued
			if (event.payload.action === 'queued') {
				refreshReviewQueue();
			}
		});

		unlistenDrag = await listen<{ paths: string[] }>('tauri://drag-drop', async (event) => {
			dragOver = false;
			const paths = event.payload.paths;
			if (!paths || paths.length === 0) return;

			// Find the first directory in the dropped paths
			for (const path of paths) {
				if (await isDirectory(path)) {
					dragInitialPath = path;
					addDialogOpen = true;
					return;
				}
			}
		});

		// Load data after listeners are ready
		await loadWatchers();
	});

	onDestroy(() => {
		unlisten?.();
		unlistenDrag?.();
	});
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="p-8 min-h-full transition-colors {dragOver ? 'bg-accent/10' : ''}"
	style="transition-duration: var(--duration-normal);"
	ondragover={(e) => { e.preventDefault(); dragOver = true; }}
	ondragleave={() => (dragOver = false)}
	ondrop={(e) => e.preventDefault()}
>
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
				No folders being watched. Drop a folder here or click Add Folder to start monitoring for new media files.
			</p>
		</div>
	{:else}
		<div class="mb-8">
			{#each watcherState.watchers as watcher (watcher.path)}
				<WatcherCard {watcher} eventCounts={eventCountsFor(watcher.path)} onRemove={removeWatcher} onToggled={loadWatchers} onEdit={openEditDialog} />
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

<AddWatcherDialog
	bind:open={addDialogOpen}
	initialPath={dragInitialPath}
	onAdded={() => { dragInitialPath = ''; loadWatchers(); }}
/>

<Sheet.Root bind:open={editDialogOpen}>
	<Sheet.Content side="right" class="overflow-y-auto">
		<Sheet.Header>
			<Sheet.Title>Edit Watcher</Sheet.Title>
			<Sheet.Description>
				{editingWatcher?.path ?? ''}
			</Sheet.Description>
		</Sheet.Header>

		<div class="flex flex-col gap-5 px-6 py-4">
			<div class="flex flex-col gap-1.5">
				<span class="text-xs font-medium text-muted-foreground">Mode</span>
				<div class="flex items-center gap-4">
					<label class="flex items-center gap-2 text-sm cursor-pointer">
						<input type="radio" name="edit-watcher-mode" value="auto" bind:group={editMode} class="accent-primary" />
						Auto-rename
					</label>
					<label class="flex items-center gap-2 text-sm cursor-pointer">
						<input type="radio" name="edit-watcher-mode" value="review" bind:group={editMode} class="accent-primary" />
						Queue for review
					</label>
				</div>
			</div>

			<div class="flex flex-col gap-1.5">
				<label for="edit-settle-time" class="text-xs font-medium text-muted-foreground">Settle time (seconds)</label>
				<input
					id="edit-settle-time"
					type="number"
					min="1"
					max="60"
					bind:value={editDebounce}
					class="w-24 rounded-md border border-input bg-background px-3 py-1.5 text-sm text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
				/>
			</div>

			<div class="border-t border-border/50 pt-4">
				<h3 class="text-xs font-medium text-muted-foreground mb-3 flex items-center gap-1.5">
					<Settings2 class="size-3" />
					Custom Settings
				</h3>
				{#if editGlobalConfig}
					<WatcherSettingsEditor bind:settings={editSettings} globalConfig={editGlobalConfig} />
				{/if}
			</div>

			{#if editingWatcher?.active}
				<p class="text-[11px] text-amber-500">Settings changes require restarting the watcher to take effect.</p>
			{/if}

			{#if editError}
				<p class="text-xs text-destructive">{editError}</p>
			{/if}
		</div>

		<Sheet.Footer>
			<Button variant="outline" onclick={() => (editDialogOpen = false)}>Cancel</Button>
			<Button disabled={editSaving} onclick={handleEditSave}>
				{editSaving ? 'Saving...' : 'Save Changes'}
			</Button>
		</Sheet.Footer>
	</Sheet.Content>
</Sheet.Root>
