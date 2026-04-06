<script lang="ts">
	import { scanState } from '$lib/state/scan.svelte.js';
	import { Search, X } from '@lucide/svelte';

	const progressPercent = $derived(
		scanState.scanProgress.total > 0
			? Math.round((scanState.scanProgress.scanned / scanState.scanProgress.total) * 100)
			: 0
	);
</script>

<div class="relative">
	<!-- Progress bar (thin accent line at very top when scanning) -->
	{#if scanState.loading && scanState.scanProgress.total > 0}
		<div class="absolute top-0 left-0 right-0 h-0.5 bg-border/30">
			<div
				class="h-full bg-foreground/60 transition-[width] duration-300 ease-out"
				style="width: {progressPercent}%;"
			></div>
		</div>
	{:else if scanState.loading}
		<div class="absolute top-0 left-0 right-0 h-0.5 bg-border/30 overflow-hidden">
			<div class="h-full w-1/3 bg-foreground/60 animate-pulse"></div>
		</div>
	{/if}

	<div class="flex items-center gap-4 px-4 py-3 border-b border-border">
		<div class="flex items-center gap-1.5 min-w-0 flex-wrap">
			<span class="text-xs text-muted-foreground/60 shrink-0">
				{scanState.results.length} file{scanState.results.length !== 1 ? 's' : ''}
			</span>
			{#if scanState.loading && scanState.folderPaths.length > 1 && scanState.scanningFolderIndex >= 0}
				<span class="text-xs text-muted-foreground/60 shrink-0">
					Scanning folder {scanState.scanningFolderIndex + 1} of {scanState.folderPaths.length}...
				</span>
			{:else if scanState.loading}
				<span class="text-xs text-muted-foreground/60 shrink-0 animate-pulse">Scanning...</span>
			{/if}

			<!-- Clear button — before spacer so it stays visible near the file count -->
			{#if scanState.folderPaths.length > 0 || scanState.filePaths.length > 0 || scanState.results.length > 0}
				<button
					type="button"
					class="text-xs text-muted-foreground/60 hover:text-foreground transition-colors shrink-0 ml-1"
					style="transition-duration: var(--duration-fast);"
					onclick={() => { scanState.reset(); }}
					aria-label="Clear scan results"
				>
					<X class="size-3.5" />
				</button>
			{/if}
		</div>

		<!-- Spacer -->
		<div class="flex-1"></div>

		<!-- Search -->
		<div class="relative">
			<Search class="absolute left-2 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground/50" />
			<input
				type="text"
				placeholder="Filter by title..."
				class="h-7 w-48 rounded-md border border-border/60 bg-background pl-7 pr-2 text-xs text-foreground placeholder:text-muted-foreground/40 focus:outline-none focus:border-ring transition-colors"
				bind:value={scanState.searchQuery}
			/>
		</div>

	</div>
</div>
