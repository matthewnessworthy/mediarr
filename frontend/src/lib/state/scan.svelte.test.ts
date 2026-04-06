import { describe, it, expect, beforeEach } from 'vitest';
import { flushSync } from 'svelte';
import { scanState } from './scan.svelte';
import { mockScanResult, mockMediaInfo } from '../../test/fixtures';

beforeEach(() => {
	scanState.reset();
});

describe('ScanState', () => {
	it('filteredResults returns all when no filters set', () => {
		const r1 = mockScanResult({ source_path: '/a.mkv' });
		const r2 = mockScanResult({ source_path: '/b.mkv' });
		scanState.results = [r1, r2];
		flushSync();
		expect(scanState.filteredResults).toHaveLength(2);
	});

	it('filteredResults sorts alphabetically by source filename', () => {
		scanState.results = [
			mockScanResult({ source_path: '/folder/Zeta.mkv' }),
			mockScanResult({ source_path: '/folder/alpha.mkv' }),
			mockScanResult({ source_path: '/other/middle.mkv' }),
		];
		flushSync();
		const names = scanState.filteredResults.map((r) => r.source_path);
		expect(names).toEqual(['/folder/alpha.mkv', '/other/middle.mkv', '/folder/Zeta.mkv']);
	});

	it('filteredResults filters by media_type', () => {
		scanState.results = [
			mockScanResult({
				source_path: '/movie.mkv',
				media_info: mockMediaInfo({ media_type: 'Movie' }),
			}),
			mockScanResult({
				source_path: '/series.mkv',
				media_info: mockMediaInfo({ media_type: 'Series' }),
			}),
		];
		scanState.filterType = 'Movie';
		flushSync();
		expect(scanState.filteredResults).toHaveLength(1);
		expect(scanState.filteredResults[0].media_info.media_type).toBe('Movie');
	});

	it('filteredResults filters by status', () => {
		scanState.results = [
			mockScanResult({ source_path: '/ok.mkv', status: 'Ok' }),
			mockScanResult({
				source_path: '/ambig.mkv',
				status: 'Ambiguous',
				ambiguity_reason: 'Multiple matches',
			}),
		];
		scanState.filterStatus = 'Ambiguous';
		flushSync();
		expect(scanState.filteredResults).toHaveLength(1);
		expect(scanState.filteredResults[0].status).toBe('Ambiguous');
	});

	it('filteredResults filters by searchQuery (case-insensitive)', () => {
		scanState.results = [
			mockScanResult({
				source_path: '/office.mkv',
				media_info: mockMediaInfo({ title: 'The Office' }),
			}),
			mockScanResult({
				source_path: '/other.mkv',
				media_info: mockMediaInfo({ title: 'Breaking Bad' }),
			}),
		];
		scanState.searchQuery = 'office';
		flushSync();
		expect(scanState.filteredResults).toHaveLength(1);
		expect(scanState.filteredResults[0].media_info.title).toBe('The Office');
	});

	it('filteredResults applies multiple filters simultaneously', () => {
		scanState.results = [
			mockScanResult({
				source_path: '/a.mkv',
				media_info: mockMediaInfo({ title: 'The Office', media_type: 'Series' }),
				status: 'Ok',
			}),
			mockScanResult({
				source_path: '/b.mkv',
				media_info: mockMediaInfo({ title: 'The Office Movie', media_type: 'Movie' }),
				status: 'Ok',
			}),
			mockScanResult({
				source_path: '/c.mkv',
				media_info: mockMediaInfo({ title: 'Breaking Bad', media_type: 'Series' }),
				status: 'Ok',
			}),
		];
		scanState.filterType = 'Series';
		scanState.searchQuery = 'office';
		flushSync();
		expect(scanState.filteredResults).toHaveLength(1);
		expect(scanState.filteredResults[0].source_path).toBe('/a.mkv');
	});

	it('selectedCount reflects selectedPaths.size', () => {
		scanState.selectedPaths = new Set(['/a.mkv', '/b.mkv']);
		flushSync();
		expect(scanState.selectedCount).toBe(2);
	});

	it('toggleSelect adds path when not present, removes when present', () => {
		scanState.toggleSelect('/a.mkv');
		flushSync();
		expect(scanState.selectedPaths.has('/a.mkv')).toBe(true);
		expect(scanState.selectedCount).toBe(1);

		scanState.toggleSelect('/a.mkv');
		flushSync();
		expect(scanState.selectedPaths.has('/a.mkv')).toBe(false);
		expect(scanState.selectedCount).toBe(0);
	});

	it('selectAll selects all filteredResults source_paths (respects filters)', () => {
		scanState.results = [
			mockScanResult({
				source_path: '/movie.mkv',
				media_info: mockMediaInfo({ media_type: 'Movie' }),
			}),
			mockScanResult({
				source_path: '/series.mkv',
				media_info: mockMediaInfo({ media_type: 'Series' }),
			}),
		];
		scanState.filterType = 'Movie';
		flushSync();
		scanState.selectAll();
		flushSync();
		expect(scanState.selectedCount).toBe(1);
		expect(scanState.selectedPaths.has('/movie.mkv')).toBe(true);
		expect(scanState.selectedPaths.has('/series.mkv')).toBe(false);
	});

	it('deselectAll empties selectedPaths', () => {
		scanState.selectedPaths = new Set(['/a.mkv', '/b.mkv']);
		flushSync();
		scanState.deselectAll();
		flushSync();
		expect(scanState.selectedCount).toBe(0);
	});

	it('counts returns correct breakdown by media_type and ambiguous status', () => {
		scanState.results = [
			mockScanResult({
				source_path: '/m1.mkv',
				media_info: mockMediaInfo({ media_type: 'Movie' }),
			}),
			mockScanResult({
				source_path: '/m2.mkv',
				media_info: mockMediaInfo({ media_type: 'Movie' }),
			}),
			mockScanResult({
				source_path: '/s1.mkv',
				media_info: mockMediaInfo({ media_type: 'Series' }),
			}),
			mockScanResult({
				source_path: '/a1.mkv',
				media_info: mockMediaInfo({ media_type: 'Anime' }),
				status: 'Ambiguous',
				ambiguity_reason: 'Multiple matches',
			}),
		];
		flushSync();
		const c = scanState.counts;
		expect(c.all).toBe(4);
		expect(c.movies).toBe(2);
		expect(c.series).toBe(1);
		expect(c.anime).toBe(1);
		expect(c.ambiguous).toBe(1);
	});

	it('reset clears all state back to defaults', () => {
		scanState.results = [mockScanResult()];
		scanState.loading = true;
		scanState.folderPaths = ['/test'];
		scanState.filterType = 'Movie';
		scanState.filterStatus = 'Ok';
		scanState.searchQuery = 'test';
		scanState.selectedPaths = new Set(['/a.mkv']);
		scanState.scanProgress = { scanned: 5, total: 10 };
		flushSync();

		scanState.reset();
		flushSync();

		expect(scanState.results).toHaveLength(0);
		expect(scanState.loading).toBe(false);
		expect(scanState.folderPaths).toEqual([]);
		expect(scanState.filterType).toBeNull();
		expect(scanState.filterStatus).toBeNull();
		expect(scanState.searchQuery).toBe('');
		expect(scanState.selectedCount).toBe(0);
		expect(scanState.scanProgress).toEqual({ scanned: 0, total: 0 });
		expect(scanState.scanningFolderIndex).toBe(-1);
	});

	describe('conflict group selection', () => {
		it('selectAll excludes Conflict-status files', () => {
			scanState.results = [
				mockScanResult({ source_path: '/ok.mkv', status: 'Ok', proposed_path: '/out/ok.mkv' }),
				mockScanResult({
					source_path: '/dup-a.mkv',
					status: 'Conflict',
					proposed_path: '/out/same.mkv',
				}),
				mockScanResult({
					source_path: '/dup-b.mkv',
					status: 'Conflict',
					proposed_path: '/out/same.mkv',
				}),
			];
			flushSync();
			scanState.selectAll();
			flushSync();
			expect(scanState.selectedPaths.has('/ok.mkv')).toBe(true);
			expect(scanState.selectedPaths.has('/dup-a.mkv')).toBe(false);
			expect(scanState.selectedPaths.has('/dup-b.mkv')).toBe(false);
			expect(scanState.selectedCount).toBe(1);
		});

		it('toggleSelect enforces mutual exclusion within conflict group', () => {
			scanState.results = [
				mockScanResult({
					source_path: '/dup-a.mkv',
					status: 'Conflict',
					proposed_path: '/out/same.mkv',
				}),
				mockScanResult({
					source_path: '/dup-b.mkv',
					status: 'Conflict',
					proposed_path: '/out/same.mkv',
				}),
				mockScanResult({
					source_path: '/dup-c.mkv',
					status: 'Conflict',
					proposed_path: '/out/same.mkv',
				}),
			];
			flushSync();

			// Select first conflict file
			scanState.toggleSelect('/dup-a.mkv');
			flushSync();
			expect(scanState.selectedPaths.has('/dup-a.mkv')).toBe(true);

			// Select second — first should be deselected
			scanState.toggleSelect('/dup-b.mkv');
			flushSync();
			expect(scanState.selectedPaths.has('/dup-b.mkv')).toBe(true);
			expect(scanState.selectedPaths.has('/dup-a.mkv')).toBe(false);

			// Select third — second should be deselected
			scanState.toggleSelect('/dup-c.mkv');
			flushSync();
			expect(scanState.selectedPaths.has('/dup-c.mkv')).toBe(true);
			expect(scanState.selectedPaths.has('/dup-b.mkv')).toBe(false);
			expect(scanState.selectedPaths.has('/dup-a.mkv')).toBe(false);
		});

		it('toggleSelect deselects a conflict file normally', () => {
			scanState.results = [
				mockScanResult({
					source_path: '/dup-a.mkv',
					status: 'Conflict',
					proposed_path: '/out/same.mkv',
				}),
				mockScanResult({
					source_path: '/dup-b.mkv',
					status: 'Conflict',
					proposed_path: '/out/same.mkv',
				}),
			];
			flushSync();

			scanState.toggleSelect('/dup-a.mkv');
			flushSync();
			expect(scanState.selectedPaths.has('/dup-a.mkv')).toBe(true);

			// Deselect it
			scanState.toggleSelect('/dup-a.mkv');
			flushSync();
			expect(scanState.selectedPaths.has('/dup-a.mkv')).toBe(false);
			expect(scanState.selectedPaths.has('/dup-b.mkv')).toBe(false);
		});

		it('toggleSelect does not affect non-conflict files', () => {
			scanState.results = [
				mockScanResult({ source_path: '/ok-a.mkv', status: 'Ok', proposed_path: '/out/a.mkv' }),
				mockScanResult({ source_path: '/ok-b.mkv', status: 'Ok', proposed_path: '/out/b.mkv' }),
			];
			flushSync();

			scanState.toggleSelect('/ok-a.mkv');
			scanState.toggleSelect('/ok-b.mkv');
			flushSync();
			expect(scanState.selectedPaths.has('/ok-a.mkv')).toBe(true);
			expect(scanState.selectedPaths.has('/ok-b.mkv')).toBe(true);
		});

		it('independent conflict groups do not interfere with each other', () => {
			scanState.results = [
				mockScanResult({
					source_path: '/group1-a.mkv',
					status: 'Conflict',
					proposed_path: '/out/dest1.mkv',
				}),
				mockScanResult({
					source_path: '/group1-b.mkv',
					status: 'Conflict',
					proposed_path: '/out/dest1.mkv',
				}),
				mockScanResult({
					source_path: '/group2-a.mkv',
					status: 'Conflict',
					proposed_path: '/out/dest2.mkv',
				}),
				mockScanResult({
					source_path: '/group2-b.mkv',
					status: 'Conflict',
					proposed_path: '/out/dest2.mkv',
				}),
			];
			flushSync();

			// Select one from each group
			scanState.toggleSelect('/group1-a.mkv');
			scanState.toggleSelect('/group2-b.mkv');
			flushSync();
			expect(scanState.selectedPaths.has('/group1-a.mkv')).toBe(true);
			expect(scanState.selectedPaths.has('/group1-b.mkv')).toBe(false);
			expect(scanState.selectedPaths.has('/group2-a.mkv')).toBe(false);
			expect(scanState.selectedPaths.has('/group2-b.mkv')).toBe(true);
			expect(scanState.selectedCount).toBe(2);
		});

		it('conflictGroups getter builds correct map', () => {
			scanState.results = [
				mockScanResult({ source_path: '/ok.mkv', status: 'Ok', proposed_path: '/out/ok.mkv' }),
				mockScanResult({
					source_path: '/dup-a.mkv',
					status: 'Conflict',
					proposed_path: '/out/same.mkv',
				}),
				mockScanResult({
					source_path: '/dup-b.mkv',
					status: 'Conflict',
					proposed_path: '/out/same.mkv',
				}),
			];
			flushSync();

			const groups = scanState.conflictGroups;
			expect(groups.size).toBe(1);
			expect(groups.get('/out/same.mkv')).toEqual(['/dup-a.mkv', '/dup-b.mkv']);
		});

		it('filteredResults sorts conflict items before non-conflict items', () => {
			scanState.results = [
				mockScanResult({ source_path: '/ok-a.mkv', status: 'Ok', proposed_path: '/out/a.mkv' }),
				mockScanResult({ source_path: '/dup-a.mkv', status: 'Conflict', proposed_path: '/out/same.mkv' }),
				mockScanResult({ source_path: '/ok-b.mkv', status: 'Ok', proposed_path: '/out/b.mkv' }),
				mockScanResult({ source_path: '/dup-b.mkv', status: 'Conflict', proposed_path: '/out/same.mkv' }),
			];
			flushSync();

			const paths = scanState.filteredResults.map((r) => r.source_path);
			// Conflict items should come first, grouped together
			expect(paths[0]).toBe('/dup-a.mkv');
			expect(paths[1]).toBe('/dup-b.mkv');
			// Non-conflict items follow
			expect(paths[2]).toBe('/ok-a.mkv');
			expect(paths[3]).toBe('/ok-b.mkv');
		});

		it('filteredResults groups conflict items by proposed_path', () => {
			scanState.results = [
				mockScanResult({ source_path: '/g2-b.mkv', status: 'Conflict', proposed_path: '/out/dest2.mkv' }),
				mockScanResult({ source_path: '/g1-a.mkv', status: 'Conflict', proposed_path: '/out/dest1.mkv' }),
				mockScanResult({ source_path: '/g2-a.mkv', status: 'Conflict', proposed_path: '/out/dest2.mkv' }),
				mockScanResult({ source_path: '/g1-b.mkv', status: 'Conflict', proposed_path: '/out/dest1.mkv' }),
			];
			flushSync();

			const results = scanState.filteredResults;
			// Group 1 (dest1) members should be adjacent
			expect(results[0].proposed_path).toBe('/out/dest1.mkv');
			expect(results[1].proposed_path).toBe('/out/dest1.mkv');
			// Group 2 (dest2) members should be adjacent
			expect(results[2].proposed_path).toBe('/out/dest2.mkv');
			expect(results[3].proposed_path).toBe('/out/dest2.mkv');
		});

		it('counts includes conflict count', () => {
			scanState.results = [
				mockScanResult({ source_path: '/ok.mkv', status: 'Ok', proposed_path: '/out/ok.mkv' }),
				mockScanResult({ source_path: '/dup-a.mkv', status: 'Conflict', proposed_path: '/out/same.mkv' }),
				mockScanResult({ source_path: '/dup-b.mkv', status: 'Conflict', proposed_path: '/out/same.mkv' }),
			];
			flushSync();
			expect(scanState.counts.conflicts).toBe(2);
		});

		it('getConflictGroupInfo returns null for non-conflict results', () => {
			scanState.results = [
				mockScanResult({ source_path: '/ok.mkv', status: 'Ok', proposed_path: '/out/ok.mkv' }),
			];
			flushSync();
			expect(scanState.getConflictGroupInfo(scanState.results[0])).toBeNull();
		});

		it('getConflictGroupInfo returns group index and size for conflict results', () => {
			scanState.results = [
				mockScanResult({ source_path: '/g1-a.mkv', status: 'Conflict', proposed_path: '/out/dest1.mkv' }),
				mockScanResult({ source_path: '/g1-b.mkv', status: 'Conflict', proposed_path: '/out/dest1.mkv' }),
				mockScanResult({ source_path: '/g2-a.mkv', status: 'Conflict', proposed_path: '/out/dest2.mkv' }),
				mockScanResult({ source_path: '/g2-b.mkv', status: 'Conflict', proposed_path: '/out/dest2.mkv' }),
				mockScanResult({ source_path: '/g2-c.mkv', status: 'Conflict', proposed_path: '/out/dest2.mkv' }),
			];
			flushSync();

			const g1Info = scanState.getConflictGroupInfo(scanState.results[0]);
			expect(g1Info).toEqual({ groupIndex: 1, groupSize: 2 });

			const g2Info = scanState.getConflictGroupInfo(scanState.results[2]);
			expect(g2Info).toEqual({ groupIndex: 2, groupSize: 3 });
		});
	});

	describe('multi-folder management', () => {
		it('addFolder appends path when not present', () => {
			scanState.addFolder('/movies');
			flushSync();
			expect(scanState.folderPaths).toEqual(['/movies']);
			scanState.addFolder('/series');
			flushSync();
			expect(scanState.folderPaths).toEqual(['/movies', '/series']);
		});

		it('addFolder does not duplicate existing path', () => {
			scanState.addFolder('/movies');
			scanState.addFolder('/movies');
			flushSync();
			expect(scanState.folderPaths).toEqual(['/movies']);
		});

		it('removeFolder removes specific path', () => {
			scanState.folderPaths = ['/movies', '/series', '/anime'];
			flushSync();
			scanState.removeFolder('/series');
			flushSync();
			expect(scanState.folderPaths).toEqual(['/movies', '/anime']);
		});

		it('removeFolder is no-op for non-existent path', () => {
			scanState.folderPaths = ['/movies'];
			flushSync();
			scanState.removeFolder('/nonexistent');
			flushSync();
			expect(scanState.folderPaths).toEqual(['/movies']);
		});
	});
});
