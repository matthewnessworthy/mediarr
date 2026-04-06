<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { RotateCcw, ChevronDown } from '@lucide/svelte';
	import { cn } from '$lib/utils.js';
	import { Button } from '$lib/components/ui/button';
	import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '$lib/components/ui/tooltip';
	import RenameDetail from './RenameDetail.svelte';
	import type { BatchSummary, UndoEligibility, RenameResult, RenameRecord } from '$lib/types';

	const {
		batch,
		expanded = false,
		onToggle,
		undoEligibility = null,
		onUndoComplete,
	}: {
		batch: BatchSummary;
		expanded: boolean;
		onToggle: () => void;
		undoEligibility: UndoEligibility | null;
		onUndoComplete?: () => void;
	} = $props();

	let undoing = $state(false);
	let entries = $state<RenameRecord[]>([]);
	let loadingEntries = $state(false);
	let entriesLoaded = $state(false);

	$effect(() => {
		if (expanded && !entriesLoaded && !loadingEntries) {
			fetchEntries();
		}
	});

	async function fetchEntries() {
		loadingEntries = true;
		try {
			entries = await invoke<RenameRecord[]>('get_batch', { batchId: batch.batch_id });
			entriesLoaded = true;
		} catch (e) {
			console.error(`Failed to load batch ${batch.batch_id}:`, e);
		} finally {
			loadingEntries = false;
		}
	}

	function formatTimestamp(iso: string): string {
		const date = new Date(iso);
		const now = new Date();
		const diff = now.getTime() - date.getTime();
		const oneDay = 86_400_000;

		if (diff < oneDay && date.getDate() === now.getDate()) {
			return `Today at ${date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}`;
		}
		const yesterday = new Date(now);
		yesterday.setDate(yesterday.getDate() - 1);
		if (date.getDate() === yesterday.getDate() && date.getMonth() === yesterday.getMonth()) {
			return `Yesterday at ${date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}`;
		}
		return date.toLocaleDateString([], { month: 'short', day: 'numeric', year: 'numeric' });
	}

	async function handleUndo() {
		undoing = true;
		try {
			await invoke<RenameResult[]>('execute_undo', { batchId: batch.batch_id });
			onUndoComplete?.();
		} catch (e) {
			console.error('Undo failed:', e);
		} finally {
			undoing = false;
		}
	}
</script>

<div class="border-b border-border/50 last:border-b-0">
	<button
		type="button"
		onclick={onToggle}
		aria-expanded={expanded}
		class="flex w-full items-center gap-4 px-4 py-3 text-left transition-colors hover:bg-accent/20 focus-ring"
		style="transition-duration: var(--duration-fast);"
	>
		<div class="flex-1 min-w-0">
			<div class="flex items-baseline gap-2">
				<span class="text-sm font-medium text-foreground">
					{formatTimestamp(batch.timestamp)}
				</span>
				<span class="text-xs text-muted-foreground">
					{batch.file_count} file{batch.file_count === 1 ? '' : 's'} renamed
				</span>
			</div>
		</div>

		{#if undoEligibility?.eligible}
			<TooltipProvider>
				<Tooltip>
					<TooltipTrigger>
						<Button
							variant="outline"
							size="xs"
							disabled={undoing}
							onclick={(e: MouseEvent) => {
								e.stopPropagation();
								handleUndo();
							}}
						>
							<RotateCcw class="size-3" />
							{undoing ? 'Undoing...' : 'Undo'}
						</Button>
					</TooltipTrigger>
					<TooltipContent>
						<p>Reverse this rename batch</p>
					</TooltipContent>
				</Tooltip>
			</TooltipProvider>
		{:else if undoEligibility && !undoEligibility.eligible}
			<TooltipProvider>
				<Tooltip>
					<TooltipTrigger>
						<Button variant="outline" size="xs" disabled>
							<RotateCcw class="size-3" />
							Undo
						</Button>
					</TooltipTrigger>
					<TooltipContent>
						<p>{undoEligibility.ineligible_reasons[0]?.reason ?? 'Cannot undo'}</p>
					</TooltipContent>
				</Tooltip>
			</TooltipProvider>
		{/if}

		<ChevronDown
			class={cn(
				'size-4 shrink-0 text-muted-foreground/50 transition-transform',
				expanded && 'rotate-180'
			)}
			style="transition-duration: var(--duration-normal);"
		/>
	</button>

	<div class={cn('expandable', expanded && 'expanded')}>
		<div>
			{#if loadingEntries}
				<div class="px-4 pb-3 space-y-2">
					{#each { length: Math.min(batch.file_count, 3) } as _}
						<div class="flex items-center gap-3 py-1.5">
							<div class="skeleton h-3 w-12"></div>
							<div class="skeleton h-3 w-40"></div>
							<div class="skeleton h-3 w-4"></div>
							<div class="skeleton h-3 w-40"></div>
						</div>
					{/each}
				</div>
			{:else if entries.length > 0}
				<div class="px-4 pb-3">
					{#each entries as record}
						<RenameDetail {record} />
					{/each}
				</div>
			{/if}
		</div>
	</div>
</div>
