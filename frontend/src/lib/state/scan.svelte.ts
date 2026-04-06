import type { ScanResult, MediaType, ScanStatus } from '$lib/types';

class ScanState {
	results = $state<ScanResult[]>([]);
	loading = $state(false);
	folderPaths = $state<string[]>([]);
	filePaths = $state<string[]>([]);
	scanningFolderIndex = $state(-1);
	filterType = $state<MediaType | null>(null);
	filterStatus = $state<ScanStatus | null>(null);
	searchQuery = $state('');
	selectedPaths = $state<Set<string>>(new Set());
	scanProgress = $state({ scanned: 0, total: 0 });
	recentPaths = $state<string[]>([]);

	get filteredResults(): ScanResult[] {
		const filtered = this.results
			.filter((r) => {
				if (this.filterType && r.media_info.media_type !== this.filterType) return false;
				if (this.filterStatus && r.status !== this.filterStatus) return false;
				if (this.searchQuery) {
					const q = this.searchQuery.toLowerCase();
					if (!r.media_info.title.toLowerCase().includes(q)) return false;
				}
				return true;
			});

		// Build a conflict group key for each result so members sort adjacent.
		// Non-conflict items get a high sort key to stay after conflict groups.
		const conflictGroupKey = new Map<string, string>();
		for (const r of filtered) {
			if (r.status === 'Conflict') {
				conflictGroupKey.set(r.source_path, r.proposed_path);
			}
		}

		return filtered.sort((a, b) => {
			const aConflict = a.status === 'Conflict';
			const bConflict = b.status === 'Conflict';

			// Conflict items sort before non-conflict items
			if (aConflict && !bConflict) return -1;
			if (!aConflict && bConflict) return 1;

			// Within conflict items, group by proposed_path
			if (aConflict && bConflict) {
				const groupCmp = a.proposed_path.localeCompare(b.proposed_path);
				if (groupCmp !== 0) return groupCmp;
			}

			// Within same group (or both non-conflict), sort by filename
			const nameA = a.source_path.split(/[\\/]/).pop()?.toLowerCase() ?? a.source_path;
			const nameB = b.source_path.split(/[\\/]/).pop()?.toLowerCase() ?? b.source_path;
			return nameA.localeCompare(nameB);
		});
	}

	get selectedCount(): number {
		return this.selectedPaths.size;
	}

	get counts() {
		const all = this.results.length;
		const series = this.results.filter((r) => r.media_info.media_type === 'Series').length;
		const movies = this.results.filter((r) => r.media_info.media_type === 'Movie').length;
		const anime = this.results.filter((r) => r.media_info.media_type === 'Anime').length;
		const ambiguous = this.results.filter((r) => r.status === 'Ambiguous').length;
		const collisions = this.results.filter((r) => r.status === 'Conflict').length;
		return { all, series, movies, anime, ambiguous, collisions };
	}

	/**
	 * Build a map of proposed_path -> source_paths[] for all Conflict-status results.
	 * Used to enforce mutual exclusion: only one file per conflict group can be selected.
	 */
	get conflictGroups(): Map<string, string[]> {
		const groups = new Map<string, string[]>();
		for (const r of this.results) {
			if (r.status !== 'Conflict') continue;
			const key = r.proposed_path;
			const existing = groups.get(key);
			if (existing) {
				existing.push(r.source_path);
			} else {
				groups.set(key, [r.source_path]);
			}
		}
		return groups;
	}

	/**
	 * Get conflict group info for a result: which group it belongs to (1-based index)
	 * and how many members are in the group. Returns null if not a conflict.
	 */
	getConflictGroupInfo(result: ScanResult): { groupIndex: number; groupSize: number } | null {
		if (result.status !== 'Conflict') return null;
		const groups = this.conflictGroups;
		const siblings = groups.get(result.proposed_path);
		if (!siblings || siblings.length < 2) return null;

		// Assign a stable 1-based group index from the conflict groups map
		let groupIndex = 0;
		for (const [key, members] of groups) {
			if (members.length >= 2) {
				groupIndex++;
				if (key === result.proposed_path) break;
			}
		}
		return { groupIndex, groupSize: siblings.length };
	}

	/**
	 * Find the conflict group (sibling source_paths sharing the same proposed_path)
	 * for a given source_path, or null if not in a conflict group.
	 */
	private getConflictSiblings(path: string): string[] | null {
		for (const siblings of this.conflictGroups.values()) {
			if (siblings.includes(path)) {
				return siblings;
			}
		}
		return null;
	}

	toggleSelect(path: string) {
		const next = new Set(this.selectedPaths);
		if (next.has(path)) {
			next.delete(path);
		} else {
			// Enforce mutual exclusion: deselect siblings in the same conflict group
			const siblings = this.getConflictSiblings(path);
			if (siblings) {
				for (const sibling of siblings) {
					if (sibling !== path) {
						next.delete(sibling);
					}
				}
			}
			next.add(path);
		}
		this.selectedPaths = next;
	}

	selectAll() {
		// Collect all source_paths that are in any conflict group
		const conflictPaths = new Set<string>();
		for (const siblings of this.conflictGroups.values()) {
			for (const p of siblings) {
				conflictPaths.add(p);
			}
		}
		// Select all filtered results EXCEPT those in conflict groups
		this.selectedPaths = new Set(
			this.filteredResults
				.filter((r) => !conflictPaths.has(r.source_path))
				.map((r) => r.source_path)
		);
	}

	deselectAll() {
		this.selectedPaths = new Set();
	}

	addFolder(path: string) {
		if (!this.folderPaths.includes(path)) {
			this.folderPaths = [...this.folderPaths, path];
		}
	}

	removeFolder(path: string) {
		this.folderPaths = this.folderPaths.filter(p => p !== path);
	}

	addFile(path: string) {
		if (!this.filePaths.includes(path)) {
			this.filePaths = [...this.filePaths, path];
		}
	}

	removeFile(path: string) {
		this.filePaths = this.filePaths.filter(p => p !== path);
	}

	removeResult(sourcePath: string) {
		this.results = this.results.filter(r => r.source_path !== sourcePath);
		const next = new Set(this.selectedPaths);
		next.delete(sourcePath);
		this.selectedPaths = next;
	}

	clearAll() {
		this.reset();
	}

	reset() {
		this.results = [];
		this.loading = false;
		this.folderPaths = [];
		this.filePaths = [];
		this.scanningFolderIndex = -1;
		this.filterType = null;
		this.filterStatus = null;
		this.searchQuery = '';
		this.selectedPaths = new Set();
		this.scanProgress = { scanned: 0, total: 0 };
	}
}

export const scanState = new ScanState();
