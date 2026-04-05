import type { ScanResult, MediaType, ScanStatus } from '$lib/types';

class ScanState {
	results = $state<ScanResult[]>([]);
	loading = $state(false);
	folderPaths = $state<string[]>([]);
	scanningFolderIndex = $state(-1);
	filterType = $state<MediaType | null>(null);
	filterStatus = $state<ScanStatus | null>(null);
	searchQuery = $state('');
	selectedPaths = $state<Set<string>>(new Set());
	scanProgress = $state({ scanned: 0, total: 0 });
	recentPaths = $state<string[]>([]);

	get filteredResults(): ScanResult[] {
		return this.results.filter((r) => {
			if (this.filterType && r.media_info.media_type !== this.filterType) return false;
			if (this.filterStatus && r.status !== this.filterStatus) return false;
			if (this.searchQuery) {
				const q = this.searchQuery.toLowerCase();
				if (!r.media_info.title.toLowerCase().includes(q)) return false;
			}
			return true;
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
		return { all, series, movies, anime, ambiguous };
	}

	toggleSelect(path: string) {
		const next = new Set(this.selectedPaths);
		if (next.has(path)) {
			next.delete(path);
		} else {
			next.add(path);
		}
		this.selectedPaths = next;
	}

	selectAll() {
		this.selectedPaths = new Set(this.filteredResults.map((r) => r.source_path));
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

	clearAll() {
		this.reset();
	}

	reset() {
		this.results = [];
		this.loading = false;
		this.folderPaths = [];
		this.scanningFolderIndex = -1;
		this.filterType = null;
		this.filterStatus = null;
		this.searchQuery = '';
		this.selectedPaths = new Set();
		this.scanProgress = { scanned: 0, total: 0 };
	}
}

export const scanState = new ScanState();
