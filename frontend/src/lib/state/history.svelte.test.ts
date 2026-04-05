import { describe, it, expect, beforeEach } from 'vitest';
import { flushSync } from 'svelte';
import { historyState } from './history.svelte';

beforeEach(() => {
	historyState.batches = [];
	historyState.loading = false;
	historyState.expandedBatchIds = new Set();
	historyState.undoEligibility = new Map();
});

describe('HistoryState', () => {
	it('toggleExpanded adds batchId when not expanded', () => {
		historyState.toggleExpanded('batch-001');
		flushSync();
		expect(historyState.expandedBatchIds.has('batch-001')).toBe(true);
	});

	it('toggleExpanded removes batchId when already expanded', () => {
		historyState.expandedBatchIds = new Set(['batch-001']);
		flushSync();
		historyState.toggleExpanded('batch-001');
		flushSync();
		expect(historyState.expandedBatchIds.has('batch-001')).toBe(false);
	});

	it('isExpanded returns true for expanded, false for collapsed', () => {
		historyState.expandedBatchIds = new Set(['batch-001']);
		flushSync();
		expect(historyState.isExpanded('batch-001')).toBe(true);
		expect(historyState.isExpanded('batch-002')).toBe(false);
	});
});
