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
	import { ChevronRight, TriangleAlert, Link, X } from '@lucide/svelte';

	const {
		result,
		selected,
		onToggleSelect,
		expanded,
		onToggleExpand,
		conflictGroup = null,
		isFirstInGroup = false,
		isLastInGroup = false,
	}: {
		result: ScanResult;
		selected: boolean;
		onToggleSelect: () => void;
		expanded: boolean;
		onToggleExpand: () => void;
		conflictGroup?: { groupIndex: number; groupSize: number } | null;
		isFirstInGroup?: boolean;
		isLastInGroup?: boolean;
	} = $props();

	const isAmbiguous = $derived(result.status === 'Ambiguous');
	const isCollision = $derived(result.status === 'Conflict');

	const outputDisplay = $derived(displayPath(result.proposed_path, result.source_path));

	function basename(path: string): string {
		return path.split(/[\\/]/).pop() ?? path;
	}

	function dirname(path: string): string {
		const parts = path.split(/[\\/]/);
		return parts.slice(0, -1).join('/');
	}

	function displayPath(proposed: string, source: string): string {
		const proposedDir = dirname(proposed);
		const sourceDir = dirname(source);

		if (proposedDir === sourceDir) {
			return basename(proposed);
		}

		const pParts = proposed.split(/[\\/]/);
		const sParts = sourceDir.split(/[\\/]/);

		let commonLen = 0;
		for (let i = 0; i < Math.min(pParts.length, sParts.length); i++) {
			if (pParts[i] === sParts[i]) commonLen = i + 1;
			else break;
		}

		const relative = pParts.slice(commonLen).join('/');
		if (relative.length > 80) {
			return pParts.slice(-3).join('/');
		}
		return relative;
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
				: config.templates.series;
		const newPath = await invoke<string>('preview_proposed_path', {
			template,
			mediaInfo: chosen,
			sourcePath: result.source_path,
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
		isCollision && 'bg-rose-500/[0.04] border-l-2 border-l-rose-500/40',
		isCollision && !isLastInGroup && 'border-b-rose-500/20',
		selected && 'bg-accent/30'
	)}
>
	<!-- Collapsed row -->
	<div class="flex items-start">
		<button
			type="button"
			aria-expanded={expanded}
			class="flex flex-1 min-w-0 items-start gap-2 px-4 py-1.5 text-left hover:bg-accent/20 transition-colors focus-ring"
			style="transition-duration: var(--duration-fast);"
			onclick={handleRowClick}
		>
			<!-- Checkbox -->
			<!-- svelte-ignore a11y_click_events_have_key_events -->
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div class="mt-0.5" onclick={handleCheckboxClick}>
				<Checkbox checked={selected} onCheckedChange={() => onToggleSelect()} />
			</div>

			<!-- Expand indicator -->
			<ChevronRight
				class={cn(
					'size-3.5 shrink-0 mt-0.5 text-muted-foreground/60 transition-transform',
					expanded && 'rotate-90'
				)}
				style="transition-duration: var(--duration-fast);"
			/>

			<!-- Content column: title + source filename aligned together -->
			<div class="flex-1 min-w-0 flex flex-col gap-0.5">
				<!-- Line 1: title row -->
				<div class="flex items-center gap-2 min-w-0">
					<!-- Media type badge -->
					{#if isCollision}
						<span class="inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[11px] font-medium uppercase tracking-wide bg-rose-500/15 text-rose-400">
							<TriangleAlert class="size-3" />
							Collision
						</span>
						{#if conflictGroup}
							<span class="inline-flex items-center gap-0.5 rounded px-1 py-0.5 text-[10px] font-medium text-rose-400/70 bg-rose-500/8">
								<Link class="size-2.5" />
								{conflictGroup.groupSize}
							</span>
						{/if}
						<MediaBadge mediaType={result.media_info.media_type} />
					{:else if isAmbiguous}
						<span class="inline-flex items-center rounded-md px-1.5 py-0.5 text-[11px] font-medium uppercase tracking-wide bg-amber-500/15 text-amber-400">
							Ambiguous
						</span>
					{:else}
						<MediaBadge mediaType={result.media_info.media_type} />
					{/if}

					<!-- Output filename -->
					<span class="flex-1 min-w-0 break-all font-medium text-sm text-foreground">
						{outputDisplay}
					</span>

					<!-- Metadata pills (hidden at narrow widths via overflow) -->
					<div class="hidden sm:block min-w-0">
						<MetadataPills mediaInfo={result.media_info} />
					</div>

					<!-- Subtitle count (hidden at narrow widths) -->
					{#if result.subtitles.length > 0}
						<span class="hidden sm:inline-flex shrink-0 items-center rounded px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground bg-muted/40">
							{result.subtitles.length} sub{result.subtitles.length !== 1 ? 's' : ''}
						</span>
					{/if}
				</div>

				<!-- Line 2: source filename -->
				<span class="break-all font-mono text-[11px] text-muted-foreground min-w-0" title={result.source_path}>
					{expanded ? result.source_path : basename(result.source_path)}
				</span>
			</div>
		</button>

		<button
			type="button"
			class="shrink-0 p-2 mr-1 self-center text-muted-foreground/30 hover:text-foreground transition-colors"
			style="transition-duration: var(--duration-fast);"
			aria-label="Remove from results"
			onclick={() => scanState.removeResult(result.source_path)}
		>
			<X class="size-3.5" />
		</button>
	</div>

	<!-- Expanded content with smooth grid-template-rows transition -->
	<div class={cn('expandable', expanded && 'expanded')}>
		<div>
			{#if result.subtitles.length > 0}
				<div class="px-4 pb-3">
					<SubtitleTree subtitles={result.subtitles} />
				</div>
			{/if}

			<!-- Ambiguity resolution panel -->
			{#if isAmbiguous && expanded}
				<AmbiguityPanel
					currentInfo={result.media_info}
					ambiguityReason={result.ambiguity_reason}
					alternatives={result.alternatives}
					groupId={result.source_path}
					onResolve={handleResolve}
				/>
			{/if}

			<!-- Collision reason -->
			{#if isCollision && expanded && result.ambiguity_reason}
				<div class="px-4 pb-2 pl-7 sm:pl-[4.25rem]">
					<span class="text-[11px] text-rose-400/80">
						{result.ambiguity_reason}
					</span>
				</div>
			{/if}

		</div>
	</div>
</div>
