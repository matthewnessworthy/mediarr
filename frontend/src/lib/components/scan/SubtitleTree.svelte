<script lang="ts">
	import type { SubtitleMatch } from '$lib/types';

	const { subtitles }: { subtitles: SubtitleMatch[] } = $props();

	function basename(path: string): string {
		return path.split(/[\\/]/).pop() ?? path;
	}

	function typeBadgeClass(type: string): string {
		switch (type.toLowerCase()) {
			case 'forced':
				return 'bg-amber-500/15 text-amber-400';
			case 'sdh':
			case 'hi':
				return 'bg-cyan-500/15 text-cyan-400';
			case 'commentary':
				return 'bg-rose-500/15 text-rose-400';
			default:
				return 'bg-muted text-muted-foreground';
		}
	}
</script>

{#if subtitles.length > 0}
	<div class="ml-8 border-l border-border pl-4 py-2 space-y-1.5">
		{#each subtitles as sub}
			<div class="flex items-center gap-2 text-xs text-muted-foreground">
				<span class="font-medium text-foreground/80 w-6 text-center shrink-0">
					{sub.language || '??'}
				</span>

				{#if sub.subtitle_type}
					<span
						class="inline-flex items-center rounded px-1 py-0.5 text-[10px] font-medium uppercase {typeBadgeClass(sub.subtitle_type)}"
					>
						{sub.subtitle_type}
					</span>
				{/if}

				<span class="text-[10px] text-muted-foreground shrink-0">
					{sub.discovery_method}
				</span>

				<span class="break-all font-mono text-[11px] text-muted-foreground" title={sub.source_path}>
					{basename(sub.source_path)}
				</span>

				<span class="text-muted-foreground/50 shrink-0">&rarr;</span>

				<span class="break-all font-mono text-[11px] text-foreground/80" title={sub.proposed_path}>
					{basename(sub.proposed_path)}
				</span>
			</div>
		{/each}
	</div>
{/if}
