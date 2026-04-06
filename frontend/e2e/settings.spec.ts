import { test, expect } from '@playwright/test';
import { gotoWithMocks } from './helpers/mock-setup';

test.describe('Settings View', () => {
	test('loads and displays settings heading', async ({ page }) => {
		await gotoWithMocks(page, '/settings');

		await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible();
		await expect(page.getByText('Configure templates, subtitles, and preferences')).toBeVisible();
	});

	test('shows template editors for movie and series', async ({ page }) => {
		await gotoWithMocks(page, '/settings');

		// Two template sections should be present
		await expect(page.getByText('Movie Template')).toBeVisible();
		await expect(page.getByText('Series Template')).toBeVisible();
	});

	test('template inputs contain default template values', async ({ page }) => {
		await gotoWithMocks(page, '/settings');

		// The template input fields should contain the default config values
		const movieInput = page.locator('input.font-mono').first();
		await expect(movieInput).toHaveValue('{title} ({year})/{title} ({year}).{ext}');
	});

	test('has subtitle discovery section with toggle', async ({ page }) => {
		await gotoWithMocks(page, '/settings');

		// Subtitle handling section
		await expect(page.getByText('Subtitle handling')).toBeVisible();
		await expect(page.getByText('Discover and rename subtitle files alongside video')).toBeVisible();
	});

	test('has Naming Templates and General section headings', async ({ page }) => {
		await gotoWithMocks(page, '/settings');

		await expect(page.getByText('Naming Templates')).toBeVisible();
		await expect(page.getByRole('heading', { name: 'Subtitles' })).toBeVisible();
		await expect(page.getByRole('heading', { name: 'General' })).toBeVisible();
	});

	test('save button shows Saved when no changes made', async ({ page }) => {
		await gotoWithMocks(page, '/settings');

		// The save button should show "Saved" when no changes have been made
		await expect(page.getByRole('button', { name: 'Saved' })).toBeVisible();
	});
});
