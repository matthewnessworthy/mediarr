<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { open as openDialog } from '@tauri-apps/plugin-dialog';
	import { FolderOpen, Settings2 } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Sheet from '$lib/components/ui/sheet';
	import WatcherSettingsEditor from './WatcherSettingsEditor.svelte';
	import type { Config, WatcherMode, WatcherSettings } from '$lib/types';

	let {
		open = $bindable(false),
		initialPath = '',
		onAdded,
	}: {
		open: boolean;
		initialPath?: string;
		onAdded?: () => void;
	} = $props();

	let folderPath = $state('');
	let mode = $state<WatcherMode>('auto');
	let debounceSeconds = $state(5);
	let saving = $state(false);
	let error = $state('');
	let showSettings = $state(false);
	let watcherSettings = $state<WatcherSettings>({});
	let globalConfig = $state<Config | null>(null);

	$effect(() => {
		if (open && !globalConfig) {
			invoke<Config>('get_config').then(c => { globalConfig = c; });
		}
	});

	// Pre-fill path when initialPath changes (e.g. from drag-and-drop)
	$effect(() => {
		if (initialPath) {
			folderPath = initialPath;
		}
	});

	async function browsePath() {
		const selected = await openDialog({ directory: true, multiple: false });
		if (selected) {
			folderPath = selected as string;
		}
	}

	async function handleSave() {
		if (!folderPath) {
			error = 'Please select a folder path';
			return;
		}
		saving = true;
		error = '';
		try {
			const config = await invoke<Config>('get_config');
			const hasOverrides = Object.values(watcherSettings).some(v => v != null);
			const newWatcher = {
				path: folderPath,
				mode,
				active: false,
				debounce_seconds: debounceSeconds,
				...(hasOverrides ? { settings: watcherSettings } : {}),
			};
			config.watchers = [...config.watchers, newWatcher];
			await invoke('update_config', { config });
			folderPath = '';
			mode = 'auto';
			debounceSeconds = 5;
			showSettings = false;
			watcherSettings = {};
			open = false;
			onAdded?.();
		} catch (e) {
			error = String(e);
		} finally {
			saving = false;
		}
	}
</script>

<Sheet.Root bind:open>
	<Sheet.Content side="right">
		<Sheet.Header>
			<Sheet.Title>Add Watch Folder</Sheet.Title>
			<Sheet.Description>
				Configure a new folder to monitor for media files.
			</Sheet.Description>
		</Sheet.Header>

		<div class="flex flex-col gap-5 px-6 py-4">
			<div class="flex flex-col gap-1.5">
				<label for="watch-path" class="text-xs font-medium text-muted-foreground">Folder Path</label>
				<div class="flex items-center gap-2">
					<input
						id="watch-path"
						type="text"
						readonly
						value={folderPath}
						placeholder="Select a folder..."
						class="flex-1 rounded-md border border-input bg-background px-3 py-1.5 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
					/>
					<Button variant="outline" size="sm" onclick={browsePath}>
						<FolderOpen class="size-3.5" />
						Browse
					</Button>
				</div>
			</div>

			<div class="flex flex-col gap-1.5">
				<span class="text-xs font-medium text-muted-foreground">Mode</span>
				<div class="flex items-center gap-4">
					<label class="flex items-center gap-2 text-sm cursor-pointer">
						<input
							type="radio"
							name="watcher-mode"
							value="auto"
							bind:group={mode}
							class="accent-primary"
						/>
						Auto-rename
					</label>
					<label class="flex items-center gap-2 text-sm cursor-pointer">
						<input
							type="radio"
							name="watcher-mode"
							value="review"
							bind:group={mode}
							class="accent-primary"
						/>
						Queue for review
					</label>
				</div>
			</div>

			<div class="flex flex-col gap-1.5">
				<label for="settle-time" class="text-xs font-medium text-muted-foreground">
					Settle time (seconds)
				</label>
				<input
					id="settle-time"
					type="number"
					min="1"
					max="60"
					bind:value={debounceSeconds}
					class="w-24 rounded-md border border-input bg-background px-3 py-1.5 text-sm text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
				/>
				<p class="text-[11px] text-muted-foreground/60">
					How long to wait after the last file change before processing. Increase if files arrive slowly (e.g. torrents).
				</p>
			</div>

			<button
				type="button"
				class="flex items-center gap-2 text-xs text-muted-foreground hover:text-foreground transition-colors"
				onclick={() => (showSettings = !showSettings)}
			>
				<Settings2 class="size-3" />
				{showSettings ? 'Hide' : 'Show'} custom settings
			</button>

			{#if showSettings && globalConfig}
				<div class="border-t border-border/50 pt-4 mt-2">
					<WatcherSettingsEditor bind:settings={watcherSettings} {globalConfig} />
				</div>
			{/if}

			{#if error}
				<p class="text-xs text-destructive">{error}</p>
			{/if}
		</div>

		<Sheet.Footer>
			<Button variant="outline" onclick={() => (open = false)}>Cancel</Button>
			<Button disabled={saving || !folderPath} onclick={handleSave}>
				{saving ? 'Saving...' : 'Add Folder'}
			</Button>
		</Sheet.Footer>
	</Sheet.Content>
</Sheet.Root>
