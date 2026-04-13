<script lang="ts">
	import { scanState } from '$lib/state/scan.svelte.js';
	import { Button } from '$lib/components/ui/button';
	import * as AlertDialog from '$lib/components/ui/alert-dialog';
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
		<AlertDialog.Root>
			<AlertDialog.Trigger>
				{#snippet child({ props })}
					<Button variant="ghost" size="sm" disabled={executing} class="text-muted-foreground" {...props}>
						<X class="size-3.5 mr-1.5" />
						Clear All
					</Button>
				{/snippet}
			</AlertDialog.Trigger>
			<AlertDialog.Content>
				<AlertDialog.Header>
					<AlertDialog.Title>Clear all scan results?</AlertDialog.Title>
					<AlertDialog.Description>
						This will remove all files from the current scan. No files on disk are affected.
					</AlertDialog.Description>
				</AlertDialog.Header>
				<AlertDialog.Footer>
					<AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
					<AlertDialog.Action variant="destructive" onclick={onClearAll}>Clear All</AlertDialog.Action>
				</AlertDialog.Footer>
			</AlertDialog.Content>
		</AlertDialog.Root>
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
