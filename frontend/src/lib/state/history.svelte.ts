import type { BatchSummary, UndoEligibility } from '$lib/types';

class HistoryState {
	batches = $state<BatchSummary[]>([]);
	loading = $state(false);
	expandedBatchIds = $state<Set<string>>(new Set());
	undoEligibility = $state<Map<string, UndoEligibility>>(new Map());

	toggleExpanded(batchId: string) {
		const next = new Set(this.expandedBatchIds);
		if (next.has(batchId)) {
			next.delete(batchId);
		} else {
			next.add(batchId);
		}
		this.expandedBatchIds = next;
	}

	isExpanded(batchId: string): boolean {
		return this.expandedBatchIds.has(batchId);
	}
}

export const historyState = new HistoryState();
