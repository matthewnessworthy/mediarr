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
		scanState.folderPath = '/test';
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
		expect(scanState.folderPath).toBe('');
		expect(scanState.filterType).toBeNull();
		expect(scanState.filterStatus).toBeNull();
		expect(scanState.searchQuery).toBe('');
		expect(scanState.selectedCount).toBe(0);
		expect(scanState.scanProgress).toEqual({ scanned: 0, total: 0 });
	});
});
