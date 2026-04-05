<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { Button } from '$lib/components/ui/button';
	import { Check, X } from '@lucide/svelte';
	import type { ReviewQueueEntry } from '$lib/types';

	const { entries, onChanged }: { entries: ReviewQueueEntry[]; onChanged: () => void } = $props();

	let processing = $state<Record<number, boolean>>({});

	function filename(path: string): string {
		const parts = path.split('/');
		return parts[parts.length - 1] || path;
	}

	function truncatePath(path: string, maxLen = 60): string {
		if (path.length <= maxLen) return path;
		const parts = path.split('/');
		if (parts.length <= 3) return '...' + path.slice(-(maxLen - 3));
		return parts[0] + '/.../' + parts.slice(-2).join('/');
	}

	function relativeTime(iso: string): string {
		const diff = Date.now() - new Date(iso).getTime();
		const seconds = Math.floor(diff / 1000);
		if (seconds < 60) return 'just now';
		const minutes = Math.floor(seconds / 60);
		if (minutes < 60) return `${minutes} min ago`;
		const hours = Math.floor(minutes / 60);
		if (hours < 24) return `${hours} hour${hours === 1 ? '' : 's'} ago`;
		const days = Math.floor(hours / 24);
		return `${days} day${days === 1 ? '' : 's'} ago`;
	}

	function isProcessing(id: number | null): boolean {
		return id !== null && !!processing[id];
	}

	async function approve(entry: ReviewQueueEntry) {
		if (entry.id === null) return;
		processing[entry.id] = true;
		try {
			await invoke('approve_review_entry', { id: entry.id });
			onChanged();
		} catch (e) {
			console.error('Failed to approve review entry:', e);
		} finally {
			if (entry.id !== null) processing[entry.id] = false;
		}
	}

	async function reject(entry: ReviewQueueEntry) {
		if (entry.id === null) return;
		processing[entry.id] = true;
		try {
			await invoke('update_review_status', { id: entry.id, status: 'rejected' });
			onChanged();
		} catch (e) {
			console.error('Failed to reject review entry:', e);
		} finally {
			if (entry.id !== null) processing[entry.id] = false;
		}
	}
</script>

{#if entries.length > 0}
	<div class="rounded-md border border-border/60">
		{#each entries as entry (entry.id)}
			<div class="border-b border-border/50 last:border-b-0 py-3 px-3 transition-colors hover:bg-accent/10" style="transition-duration: var(--duration-fast);">
				<div class="flex items-start gap-3">
					<div class="flex-1 min-w-0">
						<span class="block truncate text-sm font-medium text-foreground" title={entry.source_path}>
							{filename(entry.source_path)}
						</span>
						<span class="block truncate text-xs text-muted-foreground mt-0.5" title={entry.proposed_path}>
							{truncatePath(entry.proposed_path)}
						</span>
					</div>

					<span class="shrink-0 text-xs text-muted-foreground/60 mt-0.5">
						{relativeTime(entry.timestamp)}
					</span>

					<div class="flex items-center gap-1.5 shrink-0">
						<Button
							variant="outline"
							size="sm"
							class="h-7 px-2 text-xs text-green-600 hover:bg-green-500/10 hover:text-green-600 focus-ring"
							disabled={isProcessing(entry.id)}
							onclick={() => approve(entry)}
						>
							<Check class="size-3.5" />
							Approve
						</Button>
						<Button
							variant="outline"
							size="sm"
							class="h-7 px-2 text-xs text-destructive hover:bg-destructive/10 hover:text-destructive focus-ring"
							disabled={isProcessing(entry.id)}
							onclick={() => reject(entry)}
						>
							<X class="size-3.5" />
							Reject
						</Button>
					</div>
				</div>
			</div>
		{/each}
	</div>
{/if}
