<script lang="ts">
	import type { MediaInfo } from '$lib/types';

	const { mediaInfo }: { mediaInfo: MediaInfo } = $props();

	const pills = $derived(
		[
			mediaInfo.resolution,
			mediaInfo.source,
			mediaInfo.video_codec,
		].filter((v): v is string => v != null && v.length > 0)
	);
</script>

{#if pills.length > 0}
	<div class="flex items-center gap-1">
		{#each pills as pill}
			<span
				class="inline-flex items-center rounded px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground/60 bg-muted/30"
			>
				{pill}
			</span>
		{/each}
	</div>
{/if}
