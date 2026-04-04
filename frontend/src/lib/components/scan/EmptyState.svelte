<script lang="ts">
	import { FolderSearch } from '@lucide/svelte';
	import FolderSelector from './FolderSelector.svelte';

	const { onSelect }: { onSelect: (path: string) => void } = $props();
</script>

<div class="flex flex-1 items-center justify-center p-8">
	<div class="text-center max-w-md">
		<FolderSearch class="size-10 mx-auto mb-5 text-muted-foreground/30" />
		<h2 class="text-lg font-medium text-foreground mb-2">Scan Media Files</h2>
		<p class="text-sm text-muted-foreground mb-8 leading-relaxed">
			Drag a folder here, browse for one, or pick from recent scans.
			Files are parsed and previewed before any renames happen.
		</p>
		<FolderSelector {onSelect} />
	</div>
</div>
