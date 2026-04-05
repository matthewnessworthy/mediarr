<script lang="ts">
	import type { ScanResult, MediaInfo, Config } from '$lib/types';
	import { cn } from '$lib/utils.js';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { invoke } from '@tauri-apps/api/core';
	import { scanState } from '$lib/state/scan.svelte.js';
	import MediaBadge from './MediaBadge.svelte';
	import MetadataPills from './MetadataPills.svelte';
	import SubtitleTree from './SubtitleTree.svelte';
	import AmbiguityPanel from './AmbiguityPanel.svelte';
	import { ChevronRight } from '@lucide/svelte';

	const {
		result,
		selected,
		onToggleSelect,
		expanded,
		onToggleExpand,
	}: {
		result: ScanResult;
		selected: boolean;
		onToggleSelect: () => void;
		expanded: boolean;
		onToggleExpand: () => void;
	} = $props();

	const isAmbiguous = $derived(result.status === 'Ambiguous');

	const formattedTitle = $derived(() => {
		const info = result.media_info;
		if (info.media_type === 'Movie') {
			return info.year ? `${info.title} (${info.year})` : info.title;
		}
		// Series or Anime
		const season = info.season != null ? `S${String(info.season).padStart(2, '0')}` : '';
		const episode =
			info.episodes.length > 0
				? `E${info.episodes.map((e) => String(e).padStart(2, '0')).join('E')}`
				: '';
		const se = [season, episode].filter(Boolean).join('');
		return se ? `${info.title} ${se}` : info.title;
	});

	function basename(path: string): string {
		return path.split(/[\\/]/).pop() ?? path;
	}

	function handleRowClick() {
		onToggleExpand();
	}

	function handleCheckboxClick(e: MouseEvent) {
		e.stopPropagation();
	}

	async function handleResolve(chosen: MediaInfo) {
		// Get the template for this media type from config
		const config = await invoke<Config>('get_config');
		const template =
			chosen.media_type === 'Movie'
				? config.templates.movie
				: chosen.media_type === 'Anime'
					? config.templates.anime
					: config.templates.series;
		const newPath = await invoke<string>('preview_template', {
			template,
			mediaInfo: chosen,
		});

		// Update the result in scanState
		const idx = scanState.results.findIndex((r) => r.source_path === result.source_path);
		if (idx !== -1) {
			scanState.results[idx] = {
				...scanState.results[idx],
				media_info: chosen,
				proposed_path: newPath,
				status: 'Ok',
				ambiguity_reason: null,
				alternatives: [],
			};
			scanState.results = [...scanState.results]; // trigger reactivity
		}
	}
</script>

<div
	class={cn(
		'border-b border-border/50',
		isAmbiguous && 'bg-amber-500/[0.03]',
		selected && 'bg-accent/30'
	)}
>
	<!-- Collapsed row -->
	<button
		type="button"
		aria-expanded={expanded}
		class="flex w-full flex-col gap-0.5 px-4 py-1.5 text-left hover:bg-accent/20 transition-colors focus-ring"
		style="transition-duration: var(--duration-fast);"
		onclick={handleRowClick}
	>
		<!-- Line 1: title row -->
		<div class="flex items-center gap-2 min-w-0">
			<!-- Checkbox -->
			<!-- svelte-ignore a11y_click_events_have_key_events -->
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div onclick={handleCheckboxClick}>
				<Checkbox checked={selected} onCheckedChange={() => onToggleSelect()} />
			</div>

			<!-- Expand indicator -->
			<ChevronRight
				class={cn(
					'size-3.5 shrink-0 text-muted-foreground/50 transition-transform',
					expanded && 'rotate-90'
				)}
				style="transition-duration: var(--duration-fast);"
			/>

			<!-- Media type badge -->
			{#if isAmbiguous}
				<span class="inline-flex items-center rounded-md px-1.5 py-0.5 text-[11px] font-medium uppercase tracking-wide bg-amber-500/15 text-amber-400">
					Ambiguous
				</span>
			{:else}
				<MediaBadge mediaType={result.media_info.media_type} />
			{/if}

			<!-- Title + episode info -->
			<span class="flex-1 min-w-0 truncate font-medium text-sm text-foreground">
				{formattedTitle()}
			</span>

			<!-- Metadata pills -->
			<div class="shrink-0">
				<MetadataPills mediaInfo={result.media_info} />
			</div>

			<!-- Subtitle count -->
			{#if result.subtitles.length > 0}
				<span class="shrink-0 inline-flex items-center rounded px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground/70 bg-muted/40">
					{result.subtitles.length} sub{result.subtitles.length !== 1 ? 's' : ''}
				</span>
			{/if}
		</div>

		<!-- Line 2: source path -> proposed path -->
		<div class="flex items-center gap-2 pl-[4.25rem] min-w-0">
			<span class="truncate-start font-mono text-[11px] text-muted-foreground/60 min-w-0 flex-1" title={result.source_path}>
				{basename(result.source_path)}
			</span>
			<span class="text-muted-foreground/30 shrink-0">&rarr;</span>
			<span class="truncate font-mono text-[11px] text-foreground/60 min-w-0 flex-1" title={result.proposed_path}>
				{result.proposed_path}
			</span>
		</div>
	</button>

	<!-- Expanded content with smooth grid-template-rows transition -->
	<div class={cn('expandable', expanded && 'expanded')}>
		<div>
			{#if result.subtitles.length > 0}
				<div class="px-4 pb-3">
					<SubtitleTree subtitles={result.subtitles} />
				</div>
			{:else if expanded}
				<div class="px-4 pb-3 ml-8 pl-4">
					<span class="text-xs text-muted-foreground/50">No subtitles found</span>
				</div>
			{/if}

			<!-- Ambiguity resolution panel -->
			{#if isAmbiguous && expanded}
				<AmbiguityPanel
					currentInfo={result.media_info}
					ambiguityReason={result.ambiguity_reason}
					alternatives={result.alternatives}
					onResolve={handleResolve}
				/>
			{/if}

			<!-- Template type indicator -->
			{#if expanded}
				<div class="px-4 pb-2 pl-[4.25rem]">
					<span class="text-[11px] text-muted-foreground/40">
						Using: {result.media_info.media_type} template
					</span>
				</div>
			{/if}
		</div>
	</div>
</div>
