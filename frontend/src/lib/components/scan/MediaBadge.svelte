<script lang="ts">
	import type { MediaType } from '$lib/types';
	import { cn } from '$lib/utils.js';

	const { mediaType }: { mediaType: MediaType } = $props();

	const variants: Record<MediaType, string> = {
		Series: 'bg-blue-500/15 text-blue-400',
		Movie: 'bg-emerald-500/15 text-emerald-400',
		Anime: 'bg-purple-500/15 text-purple-400',
	};
</script>

<span
	class={cn(
		'inline-flex items-center rounded-md px-1.5 py-0.5 text-[11px] font-medium uppercase tracking-wide',
		variants[mediaType]
	)}
>
	{mediaType}
</span>
