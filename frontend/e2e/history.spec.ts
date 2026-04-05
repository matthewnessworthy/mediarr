import { test, expect } from '@playwright/test';
import { gotoWithMocks, setupMocks, defaultMockData } from './helpers/mock-setup';

test.describe('History View', () => {
	test('shows empty state when no batches exist', async ({ page }) => {
		await gotoWithMocks(page, '/history', {
			list_batches: [],
		});

		await expect(page.getByText('No rename history yet')).toBeVisible();
		await expect(page.getByText('Your renamed files will appear here')).toBeVisible();
	});

	test('displays heading with batch count', async ({ page }) => {
		await gotoWithMocks(page, '/history', {
			list_batches: [
				{
					batch_id: 'batch-001',
					timestamp: '2024-01-15T10:30:00Z',
					file_count: 3,
					entries: [],
				},
				{
					batch_id: 'batch-002',
					timestamp: '2024-01-14T08:00:00Z',
					file_count: 5,
					entries: [],
				},
			],
			check_undo: { eligible: true, batch_id: 'batch-001', ineligible_reasons: [] },
		});

		await expect(page.getByRole('heading', { name: 'History' })).toBeVisible();
		await expect(page.getByText('2 batches')).toBeVisible();
	});

	test('displays batch cards with file counts', async ({ page }) => {
		await gotoWithMocks(page, '/history', {
			list_batches: [
				{
					batch_id: 'batch-001',
					timestamp: new Date().toISOString(),
					file_count: 7,
					entries: [],
				},
			],
			check_undo: { eligible: true, batch_id: 'batch-001', ineligible_reasons: [] },
		});

		// Batch card should show file count
		await expect(page.getByText('7 files renamed')).toBeVisible();
	});

	test('undo button is visible for eligible batches', async ({ page }) => {
		await gotoWithMocks(page, '/history', {
			list_batches: [
				{
					batch_id: 'batch-001',
					timestamp: new Date().toISOString(),
					file_count: 2,
					entries: [],
				},
			],
			check_undo: { eligible: true, batch_id: 'batch-001', ineligible_reasons: [] },
		});

		// Undo button text should be visible (use getByText to avoid strict mode with tooltip wrappers)
		await expect(page.getByText('Undo', { exact: true })).toBeVisible();
	});

	test('batch card expands to show rename details', async ({ page }) => {
		await gotoWithMocks(page, '/history', {
			list_batches: [
				{
					batch_id: 'batch-001',
					timestamp: new Date().toISOString(),
					file_count: 1,
					entries: [
						{
							batch_id: 'batch-001',
							timestamp: new Date().toISOString(),
							source_path: '/media/Old.Name.mkv',
							dest_path: '/output/New Name/New Name.mkv',
							media_info: {
								title: 'New Name',
								media_type: 'Movie',
								year: 2024,
								season: null,
								episodes: [],
								resolution: '1080p',
								video_codec: 'x264',
								audio_codec: null,
								source: 'BluRay',
								release_group: 'GROUP',
								container: 'mkv',
								language: null,
								confidence: 'High',
							},
							file_size: 1500000000,
							file_mtime: new Date().toISOString(),
						},
					],
				},
			],
			check_undo: { eligible: true, batch_id: 'batch-001', ineligible_reasons: [] },
		});

		// Click the batch card button to expand
		await page.getByText('1 file renamed').click();

		// After expanding, should show filenames from RenameDetail component
		// RenameDetail uses filename() which extracts just the filename from paths
		await expect(page.getByText('Old.Name.mkv')).toBeVisible();
		await expect(page.getByText('New Name.mkv')).toBeVisible();
	});
});
