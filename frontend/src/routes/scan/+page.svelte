<script lang="ts">
	import { invoke, Channel } from '@tauri-apps/api/core';
	import type { ScanEvent } from '$lib/types';
	import { scanState } from '$lib/state/scan.svelte.js';
	import ScanTopBar from '$lib/components/scan/ScanTopBar.svelte';
	import ScanBottomBar from '$lib/components/scan/ScanBottomBar.svelte';
	import ScanRow from '$lib/components/scan/ScanRow.svelte';
	import FilterTabs from '$lib/components/scan/FilterTabs.svelte';
	import EmptyState from '$lib/components/scan/EmptyState.svelte';

	let expandedPaths = $state<Set<string>>(new Set());
	let scanError = $state<string | null>(null);

	const hasResults = $derived(scanState.results.length > 0);
	const showMain = $derived(hasResults || scanState.loading);

	async function startScan(path: string) {
		scanState.reset();
		expandedPaths = new Set();
		scanError = null;
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

	async function handleDryRun() {
		const entries = scanState.filteredResults
			.filter((r) => scanState.selectedPaths.has(r.source_path))
			.map((r) => ({
				source: r.source_path,
				destination: r.proposed_path,
			}));

		try {
			const results = await invoke<Array<{ source_path: string; dest_path: string; success: boolean; error: string | null }>>('dry_run_renames', { entries });
			// TODO: Display dry run results in a modal or panel
			console.log('Dry run results:', results);
		} catch (e) {
			console.error('Dry run failed:', e);
		}
	}

	async function handleApplyRenames() {
		const entries = scanState.filteredResults
			.filter((r) => scanState.selectedPaths.has(r.source_path))
			.map((r) => ({
				source: r.source_path,
				destination: r.proposed_path,
			}));

		try {
			await invoke('execute_renames', { entries });
			// After successful rename, re-scan or clear results
		} catch (e) {
			console.error('Rename failed:', e);
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
		<ScanBottomBar onDryRun={handleDryRun} onApplyRenames={handleApplyRenames} />
	{:else}
		<!-- Empty state -->
		<EmptyState onSelect={startScan} />
	{/if}
</div>
