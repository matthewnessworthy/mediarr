<script lang="ts">
	import type { MediaInfo, MediaType } from '$lib/types';
	import { cn } from '$lib/utils.js';
	import MediaBadge from './MediaBadge.svelte';
	import { Loader2 } from '@lucide/svelte';

	const ALL_MEDIA_TYPES: MediaType[] = ['Movie', 'Series'];

	const {
		currentInfo,
		ambiguityReason,
		alternatives,
		groupId,
		onResolve,
	}: {
		currentInfo: MediaInfo;
		ambiguityReason: string | null;
		alternatives: MediaInfo[];
		groupId: string;
		onResolve: (chosen: MediaInfo) => void;
	} = $props();

	let selectedIndex = $state<number>(0); // 0 = current, 1+ = alternatives
	let resolving = $state(false);

	/**
	 * Build the full options list: current info first, then backend-provided
	 * alternatives, then synthetic alternatives for any missing media types.
	 */
	const allOptions = $derived.by(() => {
		const options: MediaInfo[] = [currentInfo, ...alternatives];
		const presentTypes = new Set(options.map((o) => o.media_type));

		for (const mediaType of ALL_MEDIA_TYPES) {
			if (!presentTypes.has(mediaType)) {
				options.push({
					...currentInfo,
					media_type: mediaType,
					confidence: 'Low',
				});
			}
		}

		return options;
	});

	function formatOption(info: MediaInfo): string {
		if (info.media_type === 'Movie') {
			return info.year ? `${info.title} (${info.year})` : info.title;
		}
		const season = info.season != null ? `S${String(info.season).padStart(2, '0')}` : '';
		const episode =
			info.episodes.length > 0
				? `E${info.episodes.map((e: number) => String(e).padStart(2, '0')).join('E')}`
				: '';
		const se = [season, episode].filter(Boolean).join('');
		return se ? `${info.title} ${se}` : info.title;
	}

	async function handleApply() {
		if (selectedIndex === 0) return; // No change
		resolving = true;
		try {
			onResolve(allOptions[selectedIndex]);
		} finally {
			resolving = false;
		}
	}
</script>

<div class="bg-muted/30 border-t border-border/30 px-4 py-3 ml-8 pl-4">
	<!-- Ambiguity reason -->
	{#if ambiguityReason}
		<p class="text-xs text-amber-400/80 mb-2">{ambiguityReason}</p>
	{/if}

	<!-- Radio options -->
	<div class="flex flex-col gap-1.5">
		{#each allOptions as option, i}
			<label
				class={cn(
					'flex items-center gap-2.5 px-2.5 py-1.5 rounded-md cursor-pointer transition-colors duration-100 hover:bg-accent/20',
					selectedIndex === i && 'bg-accent/30'
				)}
			>
				<input
					type="radio"
					name="ambiguity-{groupId}"
					value={i}
					checked={selectedIndex === i}
					onchange={() => (selectedIndex = i)}
					class="accent-primary size-3.5"
				/>
				<MediaBadge mediaType={option.media_type} />
				<span class="text-sm text-foreground">{formatOption(option)}</span>
				{#if option.confidence}
					<span class="text-[10px] text-muted-foreground/60 ml-auto">
						{option.confidence}
					</span>
				{/if}
				{#if i === 0}
					<span class="text-[10px] text-muted-foreground/40 italic">(current)</span>
				{/if}
			</label>
		{/each}
	</div>

	<!-- Apply button -->
	<div class="mt-2.5 flex items-center gap-2">
		<button
			type="button"
			class="inline-flex items-center gap-1.5 rounded-md bg-primary px-3 py-1 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-40 disabled:cursor-not-allowed"
			disabled={selectedIndex === 0 || resolving}
			onclick={handleApply}
		>
			{#if resolving}
				<Loader2 class="size-3 animate-spin" />
			{/if}
			Apply
		</button>
		{#if selectedIndex === 0}
			<span class="text-[10px] text-muted-foreground/40">Select an alternative to apply</span>
		{/if}
	</div>
</div>
