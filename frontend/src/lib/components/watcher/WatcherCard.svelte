<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { Badge } from '$lib/components/ui/badge';
	import { Switch } from '$lib/components/ui/switch';
	import * as AlertDialog from '$lib/components/ui/alert-dialog';
	import { Button } from '$lib/components/ui/button';
	import { Trash2, Settings2 } from '@lucide/svelte';
	import type { WatcherConfig } from '$lib/types';
	import { truncatePath } from '$lib/utils.js';

	const {
		watcher,
		eventCounts = { processed: 0, errors: 0, pending: 0 },
		onRemove,
		onToggled,
		onEdit,
	}: {
		watcher: WatcherConfig;
		eventCounts?: { processed: number; errors: number; pending: number };
		onRemove?: (path: string) => void;
		onToggled?: () => void;
		onEdit?: (path: string) => void;
	} = $props();

	const hasCustomSettings = $derived(
		watcher.settings != null && Object.values(watcher.settings).some(v => v != null)
	);

	let toggling = $state(false);
	let toggleError = $state<string | null>(null);
	const active = $derived(watcher.active);

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

<div class="border-b border-border/50 last:border-b-0 py-3 px-4 transition-colors hover:bg-accent/10" style="transition-duration: var(--duration-fast);">
	<div class="flex items-center gap-3">
		<span
			class="size-2 shrink-0 rounded-full transition-colors {active ? 'bg-green-500' : 'bg-muted-foreground/30'}"
			style="transition-duration: var(--duration-normal);"
		></span>

		<div
			class="flex-1 min-w-0 cursor-pointer"
			role="button"
			tabindex="0"
			onclick={() => onEdit?.(watcher.path)}
			onkeydown={(e) => { if (e.key === 'Enter') onEdit?.(watcher.path); }}
		>
			<span class="block truncate text-sm font-medium text-foreground" title={watcher.path}>
				{truncatePath(watcher.path)}
			</span>
		</div>

		{#if hasCustomSettings}
			<Badge variant="outline" class="shrink-0 text-[10px] font-normal gap-1">
				<Settings2 class="size-2.5" />
				Custom
			</Badge>
		{/if}

		<Badge variant="secondary" class="shrink-0 text-[10px] font-normal">
			{watcher.mode === 'auto' ? 'Auto-rename' : 'Queue for review'}
		</Badge>

		<Switch
			checked={active}
			onCheckedChange={(checked: boolean) => handleToggle(checked)}
			disabled={toggling}
			size="sm"
		/>

		<AlertDialog.Root>
			<AlertDialog.Trigger>
				{#snippet child({ props })}
					<button
						type="button"
						class="shrink-0 p-1 text-muted-foreground/30 hover:text-destructive transition-colors disabled:opacity-30"
						style="transition-duration: var(--duration-fast);"
						aria-label="Remove watched folder"
						disabled={active}
						{...props}
					>
						<Trash2 class="size-3.5" />
					</button>
				{/snippet}
			</AlertDialog.Trigger>
			<AlertDialog.Content>
				<AlertDialog.Header>
					<AlertDialog.Title>Remove watched folder?</AlertDialog.Title>
					<AlertDialog.Description>
						This will stop watching '{watcher.path}' for new media files. Existing rename history is not affected.
					</AlertDialog.Description>
				</AlertDialog.Header>
				<AlertDialog.Footer>
					<AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
					<AlertDialog.Action variant="destructive" onclick={() => onRemove?.(watcher.path)}>Remove</AlertDialog.Action>
				</AlertDialog.Footer>
			</AlertDialog.Content>
		</AlertDialog.Root>
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
