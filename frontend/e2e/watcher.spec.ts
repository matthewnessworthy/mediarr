import { test, expect } from '@playwright/test';
import { gotoWithMocks } from './helpers/mock-setup';

test.describe('Watcher View', () => {
	test('shows empty state when no watchers configured', async ({ page }) => {
		await gotoWithMocks(page, '/watcher', {
			list_watchers: [],
			list_watcher_events: [],
		});

		await expect(page.getByText('No folders being watched')).toBeVisible();
		await expect(page.getByText('click Add Folder to start monitoring')).toBeVisible();
	});

	test('displays watcher cards with path and mode', async ({ page }) => {
		await gotoWithMocks(page, '/watcher', {
			list_watchers: [
				{
					path: '/watch/movies',
					mode: 'auto',
					active: true,
					debounce_seconds: 5,
				},
				{
					path: '/watch/series',
					mode: 'review',
					active: false,
					debounce_seconds: 10,
				},
			],
			list_watcher_events: [],
		});

		// Should show watcher paths
		await expect(page.getByText('/watch/movies')).toBeVisible();
		await expect(page.getByText('/watch/series')).toBeVisible();

		// Should show mode badges
		await expect(page.getByText('Auto-rename')).toBeVisible();
		await expect(page.getByText('Queue for review')).toBeVisible();
	});

	test('heading shows monitored folder count', async ({ page }) => {
		await gotoWithMocks(page, '/watcher', {
			list_watchers: [
				{ path: '/watch/movies', mode: 'auto', active: true, debounce_seconds: 5 },
				{ path: '/watch/tv', mode: 'auto', active: true, debounce_seconds: 5 },
				{ path: '/watch/anime', mode: 'review', active: false, debounce_seconds: 5 },
			],
			list_watcher_events: [],
		});

		await expect(page.getByRole('heading', { name: 'Watcher' })).toBeVisible();
		await expect(page.getByText('3 folders monitored')).toBeVisible();
	});

	test('Add Folder button is visible', async ({ page }) => {
		await gotoWithMocks(page, '/watcher', {
			list_watchers: [],
			list_watcher_events: [],
		});

		await expect(page.getByRole('button', { name: 'Add Folder' })).toBeVisible();
	});

	test('Add Folder button opens sheet dialog', async ({ page }) => {
		await gotoWithMocks(page, '/watcher', {
			list_watchers: [],
			list_watcher_events: [],
		});

		await page.getByRole('button', { name: 'Add Folder' }).click();

		// Sheet dialog should appear with the title
		await expect(page.getByText('Add Watch Folder')).toBeVisible();
		await expect(page.getByText('Configure a new folder to monitor')).toBeVisible();
	});
});
