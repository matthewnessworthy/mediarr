<script lang="ts">
	import type { MediaType, ScanStatus } from '$lib/types';
	import { scanState } from '$lib/state/scan.svelte.js';
	import { cn } from '$lib/utils.js';

	type FilterItem = {
		label: string;
		count: number;
		filterType: MediaType | null;
		filterStatus: ScanStatus | null;
	};

	const tabs = $derived<FilterItem[]>([
		{ label: 'All', count: scanState.counts.all, filterType: null, filterStatus: null },
		{ label: 'Series', count: scanState.counts.series, filterType: 'Series', filterStatus: null },
		{ label: 'Movies', count: scanState.counts.movies, filterType: 'Movie', filterStatus: null },
		{ label: 'Anime', count: scanState.counts.anime, filterType: 'Anime', filterStatus: null },
		{ label: 'Ambiguous', count: scanState.counts.ambiguous, filterType: null, filterStatus: 'Ambiguous' },
		{ label: 'Conflicts', count: scanState.counts.conflicts, filterType: null, filterStatus: 'Conflict' },
	]);

	function isActive(tab: FilterItem): boolean {
		if (tab.filterStatus) {
			return scanState.filterStatus === tab.filterStatus;
		}
		if (tab.filterType) {
			return scanState.filterType === tab.filterType && scanState.filterStatus === null;
		}
		return scanState.filterType === null && scanState.filterStatus === null;
	}

	function selectTab(tab: FilterItem) {
		scanState.filterType = tab.filterType;
		scanState.filterStatus = tab.filterStatus;
	}
</script>

<div class="flex items-center gap-1 border-b border-border px-4" role="tablist">
	{#each tabs as tab}
		<button
			type="button"
			role="tab"
			aria-selected={isActive(tab)}
			class={cn(
				'relative px-3 py-2 text-xs font-medium transition-colors duration-100',
				isActive(tab)
					? 'text-foreground'
					: 'text-muted-foreground hover:text-foreground/80'
			)}
			onclick={() => selectTab(tab)}
		>
			{tab.label}
			{#if tab.count > 0}
				<span class="ml-1 text-[10px] text-muted-foreground/60">{tab.count}</span>
			{/if}
			{#if isActive(tab)}
				<span class="absolute bottom-0 left-3 right-3 h-px bg-foreground"></span>
			{/if}
		</button>
	{/each}
</div>
