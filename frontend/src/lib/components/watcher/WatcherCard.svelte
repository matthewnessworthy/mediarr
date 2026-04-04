<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { Switch } from '$lib/components/ui/switch';
	import { Badge } from '$lib/components/ui/badge';
	import type { WatcherConfig } from '$lib/types';

	const {
		watcher,
		eventCounts = { processed: 0, errors: 0, pending: 0 },
	}: {
		watcher: WatcherConfig;
		eventCounts?: { processed: number; errors: number; pending: number };
	} = $props();

	let toggling = $state(false);
	let active = $state(watcher.active);

	function truncatePath(path: string, maxLen = 45): string {
		if (path.length <= maxLen) return path;
		const parts = path.split('/');
		if (parts.length <= 3) return '...' + path.slice(-(maxLen - 3));
		return parts[0] + '/.../' + parts.slice(-2).join('/');
	}

	async function handleToggle(checked: boolean) {
		toggling = true;
		try {
			const command = checked ? 'start_watcher' : 'stop_watcher';
			await invoke(command, { watchPath: watcher.path });
			active = checked;
		} catch (e) {
			console.error('Watcher toggle failed:', e);
			// Revert to original state on error
			active = !checked;
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

		<Switch
			checked={active}
			disabled={toggling}
			onCheckedChange={handleToggle}
			size="sm"
		/>
	</div>

	<div class="mt-2 flex items-center gap-4 pl-5 text-xs text-muted-foreground">
		<span>Processed: <span class="tabular-nums text-foreground/70">{eventCounts.processed}</span></span>
		<span>Errors: <span class="tabular-nums {eventCounts.errors > 0 ? 'text-destructive' : 'text-foreground/70'}">{eventCounts.errors}</span></span>
		<span>Pending: <span class="tabular-nums text-foreground/70">{eventCounts.pending}</span></span>
	</div>
</div>
