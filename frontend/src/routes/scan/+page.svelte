<script lang="ts">
	import { invoke, Channel } from '@tauri-apps/api/core';
	import type { ScanEvent, RenameResult, MediaInfo } from '$lib/types';
	import { scanState } from '$lib/state/scan.svelte.js';
	import ScanTopBar from '$lib/components/scan/ScanTopBar.svelte';
	import ScanBottomBar from '$lib/components/scan/ScanBottomBar.svelte';
	import ScanRow from '$lib/components/scan/ScanRow.svelte';
	import FilterTabs from '$lib/components/scan/FilterTabs.svelte';
	import EmptyState from '$lib/components/scan/EmptyState.svelte';

	let expandedPaths = $state<Set<string>>(new Set());
	let scanError = $state<string | null>(null);
	let dryRunResults = $state<RenameResult[] | null>(null);
	let renameResults = $state<RenameResult[] | null>(null);
	let executing = $state(false);

	const hasResults = $derived(scanState.results.length > 0);
	const showMain = $derived(hasResults || scanState.loading);

	async function startScan(path: string) {
		scanState.reset();
		expandedPaths = new Set();
		scanError = null;
		dryRunResults = null;
		renameResults = null;
		scanState.folderPath = path;
		scanState.loading = true;

		// Add to recent paths
		if (!scanState.recentPaths.includes(path)) {
			scanState.recentPaths = [path, ...scanState.recentPaths.slice(0, 4)];
		}

		const onEvent = new Channel<ScanEvent>();
		onEvent.onmessage = (message: ScanEvent) => {
			if (message.event === 'result') {
				scanState.results = [...scanState.results, message.data.scan_result];
			} else if (message.event === 'progress') {
				scanState.scanProgress = {
					scanned: message.data.scanned,
					total: message.data.total_estimate,
				};
			} else if (message.event === 'complete') {
				scanState.loading = false;
				scanState.selectAll();
			} else if (message.event === 'error') {
				scanState.loading = false;
				scanError = message.data.message;
			}
		};

		try {
			await invoke('scan_folder_streaming', { path, onEvent });
		} catch (e) {
			scanState.loading = false;
			scanError = e instanceof Error ? e.message : String(e);
		}
	}

	function toggleExpand(path: string) {
		const next = new Set(expandedPaths);
		if (next.has(path)) {
			next.delete(path);
		} else {
			next.add(path);
		}
		expandedPaths = next;
	}

	/**
	 * Build rename entries from selected scan results, including subtitle entries.
	 * Video entries include media_info for accurate history recording.
	 */
	function getSelectedEntries(): { source_path: string; dest_path: string; media_info?: MediaInfo }[] {
		const entries: { source_path: string; dest_path: string; media_info?: MediaInfo }[] = [];
		for (const result of scanState.results) {
			if (!scanState.selectedPaths.has(result.source_path)) continue;
			// Video file entry with full media info
			entries.push({
				source_path: result.source_path,
				dest_path: result.proposed_path,
				media_info: result.media_info,
			});
			// Subtitle entries (no media_info needed)
			for (const sub of result.subtitles) {
				entries.push({
					source_path: sub.source_path,
					dest_path: sub.proposed_path,
				});
			}
		}
		return entries;
	}

	async function handleDryRun() {
		executing = true;
		dryRunResults = null;
		renameResults = null;
		try {
			const entries = getSelectedEntries();
			dryRunResults = await invoke<RenameResult[]>('dry_run_renames', { entries });
		} catch (e) {
			scanError = e instanceof Error ? e.message : String(e);
		} finally {
			executing = false;
		}
	}

	async function handleApplyRenames() {
		executing = true;
		renameResults = null;
		try {
			const entries = getSelectedEntries();
			renameResults = await invoke<RenameResult[]>('execute_renames', { entries });
			// Remove successfully renamed files from the results list
			const succeeded = new Set(
				renameResults.filter((r) => r.success).map((r) => r.source_path)
			);
			scanState.results = scanState.results.filter((r) => !succeeded.has(r.source_path));
			scanState.deselectAll();
		} catch (e) {
			scanError = e instanceof Error ? e.message : String(e);
		} finally {
			executing = false;
		}
	}
</script>

<div class="flex h-full flex-col">
	{#if showMain}
		<!-- Top bar with path, count, search, folder selector -->
		<ScanTopBar onSelect={startScan} />

		<!-- Filter tabs -->
		<FilterTabs />

		<!-- Results list -->
		<div class="flex-1 overflow-y-auto">
			{#if scanError}
				<div class="flex items-center justify-center p-8">
					<div class="text-center">
						<p class="text-sm text-destructive mb-2">Scan failed</p>
						<p class="text-xs text-muted-foreground">{scanError}</p>
					</div>
				</div>
			{:else if scanState.loading && scanState.results.length === 0}
				<div class="flex items-center justify-center p-8">
					<p class="text-sm text-muted-foreground animate-pulse">Scanning...</p>
				</div>
			{:else if scanState.filteredResults.length === 0 && hasResults}
				<div class="flex items-center justify-center p-8">
					<p class="text-sm text-muted-foreground">No files match the current filter</p>
				</div>
			{:else}
				{#each scanState.filteredResults as result (result.source_path)}
					<ScanRow
						{result}
						selected={scanState.selectedPaths.has(result.source_path)}
						onToggleSelect={() => scanState.toggleSelect(result.source_path)}
						expanded={expandedPaths.has(result.source_path)}
						onToggleExpand={() => toggleExpand(result.source_path)}
					/>
				{/each}
			{/if}
		</div>

		<!-- Bottom bar with selection and actions -->
		<ScanBottomBar
			onDryRun={handleDryRun}
			onApplyRenames={handleApplyRenames}
			{dryRunResults}
			{renameResults}
			{executing}
		/>
	{:else}
		<!-- Empty state -->
		<EmptyState onSelect={startScan} />
	{/if}
</div>
