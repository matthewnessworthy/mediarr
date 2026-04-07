<script lang="ts">
	import { Check, Clock, X } from '@lucide/svelte';
	import type { WatcherEvent } from '$lib/types';
	import { relativeTime } from '$lib/utils.js';

	const { events }: { events: WatcherEvent[] } = $props();

	function actionText(event: WatcherEvent): string {
		switch (event.action) {
			case 'renamed':
				return event.detail ? `renamed to ${event.detail}` : 'renamed';
			case 'queued':
				return 'queued for review';
			case 'error':
				return event.detail ? `error: ${event.detail}` : 'error';
			default:
				return String(event.action);
		}
	}

	function borderColor(action: string): string {
		switch (action) {
			case 'renamed':
				return 'border-l-green-500/60';
			case 'queued':
				return 'border-l-amber-500/60';
			case 'error':
				return 'border-l-destructive/60';
			default:
				return 'border-l-border';
		}
	}
</script>

{#if events.length === 0}
	<div class="py-8 text-center">
		<p class="text-sm text-muted-foreground">No watcher activity yet</p>
	</div>
{:else}
	<div class="max-h-80 overflow-y-auto">
		{#each events as event}
			<div class="flex items-start gap-3 py-2 px-3 text-xs border-l-2 {borderColor(event.action)}">
				{#if event.action === 'renamed'}
					<Check class="mt-0.5 size-3.5 shrink-0 text-green-500" />
				{:else if event.action === 'queued'}
					<Clock class="mt-0.5 size-3.5 shrink-0 text-amber-500" />
				{:else}
					<X class="mt-0.5 size-3.5 shrink-0 text-destructive" />
				{/if}

				<div class="flex-1 min-w-0">
					<span class="font-mono text-foreground/80">{event.filename}</span>
					<span class="text-muted-foreground"> {actionText(event)}</span>
				</div>

				<span class="shrink-0 text-muted-foreground/60">{relativeTime(event.timestamp)}</span>
			</div>
		{/each}
	</div>
{/if}
