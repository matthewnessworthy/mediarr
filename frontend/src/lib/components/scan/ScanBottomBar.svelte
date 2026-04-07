<script lang="ts">
	import { scanState } from '$lib/state/scan.svelte.js';
	import { Button } from '$lib/components/ui/button';
	import { Loader2, X } from '@lucide/svelte';

	const {
		onApplyRenames,
		onClearAll,
		executing = false,
		hasResults = false,
	}: {
		onApplyRenames: () => Promise<void>;
		onClearAll: () => void;
		executing?: boolean;
		hasResults?: boolean;
	} = $props();

	const hasSelection = $derived(scanState.filteredSelectedCount > 0);
</script>

<div class="flex items-center gap-4 px-4 py-3 border-t border-border bg-background/80 backdrop-blur-sm">
	<!-- Clear All -->
	{#if hasResults}
		<Button variant="ghost" size="sm" disabled={executing} onclick={onClearAll} class="text-muted-foreground">
			<X class="size-3.5 mr-1.5" />
			Clear All
		</Button>
	{/if}

	<!-- Spacer -->
	<div class="flex-1"></div>

	<!-- Actions -->
	<div class="flex items-center gap-2">
		<Button size="sm" disabled={!hasSelection || executing} onclick={onApplyRenames}>
			{#if executing}
				<Loader2 class="size-3.5 animate-spin mr-1.5" />
			{/if}
			Apply Renames
		</Button>
	</div>
</div>
