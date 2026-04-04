<script lang="ts">
	import { scanState } from '$lib/state/scan.svelte.js';
	import { Button } from '$lib/components/ui/button';

	const {
		onDryRun,
		onApplyRenames,
	}: {
		onDryRun: () => void;
		onApplyRenames: () => void;
	} = $props();

	const hasSelection = $derived(scanState.selectedCount > 0);
	const totalFiltered = $derived(scanState.filteredResults.length);
</script>

<div class="flex items-center gap-4 px-4 py-2.5 border-t border-border bg-background/80 backdrop-blur-sm">
	<!-- Selection count -->
	<span class="text-xs text-muted-foreground">
		{scanState.selectedCount} selected of {totalFiltered} file{totalFiltered !== 1 ? 's' : ''}
	</span>

	<!-- Spacer -->
	<div class="flex-1"></div>

	<!-- Actions -->
	<div class="flex items-center gap-2">
		<button
			type="button"
			class="text-xs text-muted-foreground hover:text-foreground transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
			onclick={() => scanState.selectAll()}
			disabled={scanState.selectedCount === totalFiltered}
		>
			Select All
		</button>
		<button
			type="button"
			class="text-xs text-muted-foreground hover:text-foreground transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
			onclick={() => scanState.deselectAll()}
			disabled={!hasSelection}
		>
			Deselect All
		</button>

		<div class="w-px h-4 bg-border mx-1"></div>

		<Button variant="outline" size="sm" disabled={!hasSelection} onclick={onDryRun}>
			Dry Run
		</Button>
		<Button size="sm" disabled={!hasSelection} onclick={onApplyRenames}>
			Apply Renames
		</Button>
	</div>
</div>
