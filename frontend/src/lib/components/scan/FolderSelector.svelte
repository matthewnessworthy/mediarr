<script lang="ts">
	import { open } from '@tauri-apps/plugin-dialog';
	import { stat } from '@tauri-apps/plugin-fs';
	import { listen } from '@tauri-apps/api/event';
	import { onMount, onDestroy } from 'svelte';
	import { scanState } from '$lib/state/scan.svelte.js';
	import { cn } from '$lib/utils.js';
	import { Button } from '$lib/components/ui/button';
	import * as Popover from '$lib/components/ui/popover';
	import { FolderOpen, ChevronDown, Clock } from '@lucide/svelte';

	const {
		onSelect,
		compact = false,
	}: {
		onSelect: (path: string) => void;
		compact?: boolean;
	} = $props();

	let dragOver = $state(false);
	let unlisten: (() => void) | null = null;
	let destroyed = false;
	let recentOpen = $state(false);

	/**
	 * Classify a dropped path as either a file or a directory.
	 * Returns { type: 'file' | 'dir', path } so the caller can
	 * route files to scan_files and directories to scan_folder.
	 */
	async function classifyPath(path: string): Promise<{ type: 'file' | 'dir'; path: string }> {
		try {
			const info = await stat(path);
			return { type: info.isDirectory ? 'dir' : 'file', path };
		} catch {
			// stat may fail outside the fs plugin's allowed scope.
			// Fall back to extension heuristic.
			const name = path.split(/[\\/]/).pop() || '';
			return { type: /\.\w{2,5}$/.test(name) ? 'file' : 'dir', path };
		}
	}

	onMount(async () => {
		const fn = await listen<{ paths: string[] }>('tauri://drag-drop', async (event) => {
			dragOver = false;
			const paths = event.payload.paths;
			if (paths && paths.length > 0) {
				const classified = await Promise.all(paths.map(classifyPath));
				const folders = [...new Set(classified.filter(c => c.type === 'dir').map(c => c.path))];
				const files = [...new Set(classified.filter(c => c.type === 'file').map(c => c.path))];

				// Add folders to scan state and track in recent paths
				for (const folder of folders) {
					scanState.addFolder(folder);
					if (!scanState.recentPaths.includes(folder)) {
						scanState.recentPaths = [folder, ...scanState.recentPaths].slice(0, 5);
					}
				}
				// Add individual files to scan state
				for (const file of files) {
					scanState.addFile(file);
				}
				// Trigger scan
				onSelect(folders[0] ?? files[0]);
			}
		});
		// If component was destroyed while listen() was resolving, clean up immediately
		if (destroyed) {
			fn();
		} else {
			unlisten = fn;
		}
	});

	onDestroy(() => {
		destroyed = true;
		if (unlisten) unlisten();
	});

	async function openDialog() {
		const selected = await open({
			directory: true,
			multiple: false,
			title: 'Select media folder',
		});
		if (selected && typeof selected === 'string') {
			scanState.addFolder(selected);
			onSelect(selected);
		}
	}

	function selectRecent(path: string) {
		recentOpen = false;
		scanState.addFolder(path);
		onSelect(path);
	}
</script>

{#if compact}
	<!-- Compact mode: button + recent dropdown -->
	<div class="flex items-center gap-1.5">
		<Button variant="outline" size="sm" onclick={openDialog} class="focus-ring">
			<FolderOpen class="size-3.5" data-icon="inline-start" />
			Open Folder
		</Button>

		{#if scanState.recentPaths.length > 0}
			<Popover.Root bind:open={recentOpen}>
				<Popover.Trigger>
					<Button variant="ghost" size="icon-sm" class="focus-ring">
						<Clock class="size-3.5" />
					</Button>
				</Popover.Trigger>
				<Popover.Content class="w-72 p-1" align="start">
					{#each scanState.recentPaths as path}
						<button
							type="button"
							class="w-full text-left px-2 py-1.5 text-xs font-mono text-muted-foreground hover:bg-accent hover:text-foreground rounded-sm break-all transition-colors"
							style="transition-duration: var(--duration-fast);"
							title={path}
							onclick={() => selectRecent(path)}
						>
							{path}
						</button>
					{/each}
				</Popover.Content>
			</Popover.Root>
		{/if}
	</div>
{:else}
	<!-- Full mode: drop zone + button + recent -->
	<div class="flex flex-col items-center gap-6">
		<div
			class={cn(
				'w-full max-w-lg rounded-lg border border-dashed border-border/60 px-8 py-12 text-center transition-colors',
				dragOver && 'border-foreground/40 bg-accent/20'
			)}
			style="transition-duration: var(--duration-normal);"
			ondragover={(e) => {
				e.preventDefault();
				dragOver = true;
			}}
			ondragleave={() => (dragOver = false)}
			ondrop={(e) => e.preventDefault()}
			role="region"
			aria-label="Drop zone for media folders"
		>
			<FolderOpen class="size-8 mx-auto mb-4 text-muted-foreground/30" />
			<p class="text-sm text-muted-foreground mb-4">Drop files or folders here to scan</p>
			<Button variant="outline" size="sm" onclick={openDialog} class="focus-ring">
				<FolderOpen class="size-3.5" data-icon="inline-start" />
				Browse
			</Button>
		</div>

		{#if scanState.recentPaths.length > 0}
			<div class="w-full max-w-lg">
				<p class="text-xs font-medium text-muted-foreground/60 mb-2 uppercase tracking-wide">Recent</p>
				<div class="space-y-0.5">
					{#each scanState.recentPaths as path}
						<button
							type="button"
							class="w-full text-left px-3 py-1.5 text-xs font-mono text-muted-foreground hover:bg-accent hover:text-foreground rounded-md break-all transition-colors focus-ring"
							style="transition-duration: var(--duration-fast);"
							title={path}
							onclick={() => { scanState.addFolder(path); onSelect(path); }}
						>
							{path}
						</button>
					{/each}
				</div>
			</div>
		{/if}
	</div>
{/if}
