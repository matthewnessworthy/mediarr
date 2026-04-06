<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { Badge } from '$lib/components/ui/badge';
	import { Trash2 } from '@lucide/svelte';
	import type { WatcherConfig } from '$lib/types';

	const {
		watcher,
		eventCounts = { processed: 0, errors: 0, pending: 0 },
		onRemove,
		onToggled,
	}: {
		watcher: WatcherConfig;
		eventCounts?: { processed: number; errors: number; pending: number };
		onRemove?: (path: string) => void;
		onToggled?: () => void;
	} = $props();

	let toggling = $state(false);
	let toggleError = $state<string | null>(null);
	const active = $derived(watcher.active);

	function truncatePath(path: string, maxLen = 45): string {
		if (path.length <= maxLen) return path;
		const parts = path.split('/');
		if (parts.length <= 3) return '...' + path.slice(-(maxLen - 3));
		return parts[0] + '/.../' + parts.slice(-2).join('/');
	}

	async function handleToggle(checked: boolean) {
		toggling = true;
		toggleError = null;
		try {
			const command = checked ? 'start_watcher' : 'stop_watcher';
			await invoke(command, { path: watcher.path });
			// Reload watchers to reflect persisted active state
			onToggled?.();
		} catch (e) {
			console.error('Watcher toggle failed:', e);
			toggleError = e instanceof Error ? e.message : String(e);
		} finally {
			toggling = false;
		}
	}
</script>

<div class="border-b border-border/50 last:border-b-0 py-3 px-1 transition-colors hover:bg-accent/10" style="transition-duration: var(--duration-fast);">
	<div class="flex items-center gap-3">
		<span
			class="size-2 shrink-0 rounded-full transition-colors {active ? 'bg-green-500' : 'bg-muted-foreground/30'}"
			style="transition-duration: var(--duration-normal);"
		></span>

		<div class="flex-1 min-w-0">
			<span class="block truncate text-sm font-medium text-foreground" title={watcher.path}>
				{truncatePath(watcher.path)}
			</span>
		</div>

		<Badge variant="secondary" class="shrink-0 text-[10px] font-normal">
			{watcher.mode === 'auto' ? 'Auto-rename' : 'Queue for review'}
		</Badge>

		<button
			type="button"
			role="switch"
			aria-checked={active}
			disabled={toggling}
			class="shrink-0 relative inline-flex h-5 w-9 items-center rounded-full border border-transparent transition-colors focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 {active ? 'bg-primary' : 'bg-input'}"
			style="transition-duration: var(--duration-normal);"
			onclick={() => handleToggle(!active)}
		>
			<span
				class="pointer-events-none block size-4 rounded-full bg-background shadow-sm transition-transform {active ? 'translate-x-4' : 'translate-x-0'}"
				style="transition-duration: var(--duration-fast);"
			></span>
		</button>

		<button
			type="button"
			class="shrink-0 p-1 text-muted-foreground/30 hover:text-destructive transition-colors disabled:opacity-30"
			style="transition-duration: var(--duration-fast);"
			aria-label="Remove watched folder"
			disabled={active}
			onclick={() => onRemove?.(watcher.path)}
		>
			<Trash2 class="size-3.5" />
		</button>
	</div>

	<div class="mt-2 flex items-center gap-4 pl-5 text-xs text-muted-foreground">
		<span>Processed: <span class="tabular-nums text-foreground/70">{eventCounts.processed}</span></span>
		<span>Errors: <span class="tabular-nums {eventCounts.errors > 0 ? 'text-destructive' : 'text-foreground/70'}">{eventCounts.errors}</span></span>
		<span>Pending: <span class="tabular-nums text-foreground/70">{eventCounts.pending}</span></span>
	</div>

	{#if toggleError}
		<p class="mt-1.5 pl-5 text-[11px] text-destructive">{toggleError}</p>
	{/if}
</div>
