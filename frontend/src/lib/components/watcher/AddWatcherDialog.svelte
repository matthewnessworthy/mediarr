<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { open as openDialog } from '@tauri-apps/plugin-dialog';
	import { FolderOpen } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Sheet from '$lib/components/ui/sheet';
	import type { Config, WatcherMode } from '$lib/types';

	let {
		open = $bindable(false),
		onAdded,
	}: {
		open: boolean;
		onAdded?: () => void;
	} = $props();

	let folderPath = $state('');
	let mode = $state<WatcherMode>('auto');
	let debounceSeconds = $state(5);
	let saving = $state(false);
	let error = $state('');

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
			config.watchers = [
				...config.watchers,
				{
					path: folderPath,
					mode,
					active: false,
					debounce_seconds: debounceSeconds,
				},
			];
			await invoke('update_config', { config });
			folderPath = '';
			mode = 'auto';
			debounceSeconds = 5;
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
				<label for="debounce" class="text-xs font-medium text-muted-foreground">
					Debounce (seconds)
				</label>
				<input
					id="debounce"
					type="number"
					min="1"
					max="60"
					bind:value={debounceSeconds}
					class="w-24 rounded-md border border-input bg-background px-3 py-1.5 text-sm text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
				/>
			</div>

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
