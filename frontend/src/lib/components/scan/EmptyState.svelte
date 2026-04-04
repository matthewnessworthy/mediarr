<script lang="ts">
	import FolderSelector from './FolderSelector.svelte';

	const { onSelect }: { onSelect: (path: string) => void } = $props();
</script>

<div class="flex flex-1 items-center justify-center p-8">
	<div class="text-center">
		<h2 class="text-lg font-medium text-foreground mb-2">Scan Media Files</h2>
		<p class="text-sm text-muted-foreground mb-8">
			Select a folder to scan your media files and preview renames
		</p>
		<FolderSelector {onSelect} />
	</div>
</div>
