<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { historyState } from '$lib/state/history.svelte';
	import BatchCard from '$lib/components/history/BatchCard.svelte';
	import type { BatchSummary, UndoEligibility } from '$lib/types';

	async function loadBatches() {
		historyState.loading = true;
		try {
			historyState.batches = await invoke<BatchSummary[]>('list_batches', { limit: 50 });
			// Check undo eligibility for most recent batches (first 5)
			for (const batch of historyState.batches.slice(0, 5)) {
				const elig = await invoke<UndoEligibility>('check_undo', { batchId: batch.batch_id });
				historyState.undoEligibility.set(batch.batch_id, elig);
				historyState.undoEligibility = new Map(historyState.undoEligibility);
			}
		} finally {
			historyState.loading = false;
		}
	}

	onMount(() => {
		loadBatches();
	});
</script>

<div class="p-8">
	<div class="mb-6 flex items-baseline gap-3">
		<h2 class="text-lg font-medium text-foreground">History</h2>
		{#if historyState.batches.length > 0}
			<span class="text-xs text-muted-foreground">
				{historyState.batches.length} batch{historyState.batches.length === 1 ? '' : 'es'}
			</span>
		{/if}
	</div>

	{#if historyState.loading}
		<div class="space-y-4">
			{#each { length: 5 } as _}
				<div class="flex items-center gap-4 py-3">
					<div class="space-y-2 flex-1">
						<div class="h-4 w-40 animate-pulse rounded bg-muted"></div>
						<div class="h-3 w-24 animate-pulse rounded bg-muted"></div>
					</div>
					<div class="h-6 w-16 animate-pulse rounded bg-muted"></div>
				</div>
			{/each}
		</div>
	{:else if historyState.batches.length === 0}
		<div class="py-16 text-center">
			<p class="text-sm text-muted-foreground">
				No rename history yet. Scan and rename some files to see history here.
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
