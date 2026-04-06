<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Clock, Trash2 } from '@lucide/svelte';
	import { historyState } from '$lib/state/history.svelte';
	import { Button } from '$lib/components/ui/button';
	import BatchCard from '$lib/components/history/BatchCard.svelte';
	import type { BatchSummary, UndoEligibility } from '$lib/types';

	let clearing = $state(false);

	async function loadBatches() {
		historyState.loading = true;
		try {
			historyState.batches = await invoke<BatchSummary[]>('list_batches', { limit: 50 });
			// Check undo eligibility for most recent batches (first 5)
			for (const batch of historyState.batches.slice(0, 5)) {
				try {
					const elig = await invoke<UndoEligibility>('check_undo', { batchId: batch.batch_id });
					historyState.undoEligibility.set(batch.batch_id, elig);
					historyState.undoEligibility = new Map(historyState.undoEligibility);
				} catch (e) {
					console.warn(`Failed to check undo for batch ${batch.batch_id}:`, e);
				}
			}
		} finally {
			historyState.loading = false;
		}
	}

	async function clearHistory() {
		clearing = true;
		try {
			await invoke('clear_history');
			historyState.batches = [];
			historyState.undoEligibility = new Map();
			historyState.expandedBatchIds = new Set();
		} catch (e) {
			console.error('Failed to clear history:', e);
		} finally {
			clearing = false;
		}
	}

	onMount(() => {
		loadBatches();
	});
</script>

<div class="p-8 max-w-4xl">
	<div class="mb-6 flex items-center justify-between">
		<div class="flex items-baseline gap-3">
			<h2 class="text-lg font-medium text-foreground">History</h2>
			{#if historyState.batches.length > 0}
				<span class="text-xs text-muted-foreground">
					{historyState.batches.length} batch{historyState.batches.length === 1 ? '' : 'es'}
				</span>
			{/if}
		</div>
		{#if historyState.batches.length > 0}
			<Button
				variant="ghost"
				size="sm"
				disabled={clearing}
				onclick={clearHistory}
				class="text-muted-foreground hover:text-destructive"
			>
				<Trash2 class="size-3.5" />
				{clearing ? 'Clearing...' : 'Clear history'}
			</Button>
		{/if}
	</div>

	{#if historyState.loading}
		<div class="space-y-1">
			{#each { length: 5 } as _, i}
				<div class="flex items-center gap-4 py-3 border-b border-border/50">
					<div class="space-y-2 flex-1">
						<div class="skeleton h-4" style="width: {140 + i * 20}px;"></div>
						<div class="skeleton h-3" style="width: {80 + i * 10}px;"></div>
					</div>
					<div class="skeleton h-7 w-16"></div>
				</div>
			{/each}
		</div>
	{:else if historyState.batches.length === 0}
		<div class="flex flex-col items-center justify-center py-24">
			<Clock class="size-10 mb-5 text-muted-foreground/30" />
			<p class="text-sm text-muted-foreground text-center leading-relaxed max-w-xs">
				No rename history yet. Your renamed files will appear here with full undo support.
			</p>
		</div>
	{:else}
		<div>
			{#each historyState.batches as batch (batch.batch_id)}
				<BatchCard
					{batch}
					expanded={historyState.isExpanded(batch.batch_id)}
					onToggle={() => historyState.toggleExpanded(batch.batch_id)}
					undoEligibility={historyState.undoEligibility.get(batch.batch_id) ?? null}
					onUndoComplete={loadBatches}
				/>
			{/each}
		</div>
	{/if}
</div>
