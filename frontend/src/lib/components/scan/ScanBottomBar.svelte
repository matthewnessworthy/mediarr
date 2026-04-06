<script lang="ts">
	import type { RenameResult } from '$lib/types';
	import { scanState } from '$lib/state/scan.svelte.js';
	import { Button } from '$lib/components/ui/button';
	import { Loader2 } from '@lucide/svelte';

	const {
		onDryRun,
		onApplyRenames,
		dryRunResults = null,
		renameResults = null,
		executing = false,
	}: {
		onDryRun: () => Promise<void>;
		onApplyRenames: () => Promise<void>;
		dryRunResults?: RenameResult[] | null;
		renameResults?: RenameResult[] | null;
		executing?: boolean;
	} = $props();

	const hasSelection = $derived(scanState.selectedCount > 0);
	const totalFiltered = $derived(scanState.filteredResults.length);

	const dryRunSummary = $derived(() => {
		if (!dryRunResults) return null;
		const valid = dryRunResults.filter((r) => r.success).length;
		const conflicts = dryRunResults.filter((r) => !r.success).length;
		return { valid, conflicts };
	});

	const renameSummary = $derived(() => {
		if (!renameResults) return null;
		const succeeded = renameResults.filter((r) => r.success).length;
		const failed = renameResults.filter((r) => !r.success).length;
		return { succeeded, failed };
	});
</script>

<div class="flex items-center gap-4 px-4 py-2.5 border-t border-border bg-background/80 backdrop-blur-sm">
	<!-- Selection count -->
	<span class="text-xs text-muted-foreground">
		{scanState.selectedCount} selected of {totalFiltered} file{totalFiltered !== 1 ? 's' : ''}
	</span>

	<!-- Result summaries -->
	{#if dryRunSummary()}
		{@const summary = dryRunSummary()}
		<span class="text-xs">
			<span class="text-emerald-400">{summary?.valid} valid</span>
			{#if summary?.conflicts}
				<span class="text-muted-foreground">, </span>
				<span class="text-amber-400">{summary?.conflicts} collision{summary?.conflicts !== 1 ? 's' : ''}</span>
			{/if}
		</span>
	{/if}

	{#if renameSummary()}
		{@const summary = renameSummary()}
		<span class="text-xs">
			<span class="text-emerald-400">{summary?.succeeded} succeeded</span>
			{#if summary?.failed}
				<span class="text-muted-foreground">, </span>
				<span class="text-destructive">{summary?.failed} failed</span>
			{/if}
		</span>
	{/if}

	<!-- Spacer -->
	<div class="flex-1"></div>

	<!-- Actions -->
	<div class="flex items-center gap-2">
		<Button variant="outline" size="sm" disabled={!hasSelection || executing} onclick={onDryRun}>
			{#if executing}
				<Loader2 class="size-3.5 animate-spin mr-1.5" />
			{/if}
			Dry Run
		</Button>
		<Button size="sm" disabled={!hasSelection || executing} onclick={onApplyRenames}>
			{#if executing}
				<Loader2 class="size-3.5 animate-spin mr-1.5" />
			{/if}
			Apply Renames
		</Button>
	</div>
</div>
