import { test, expect } from '@playwright/test';
import { gotoWithMocks } from './helpers/mock-setup';

test.describe('Navigation', () => {
	test('sidebar shows all four navigation items', async ({ page }) => {
		await gotoWithMocks(page, '/scan');

		await expect(page.getByRole('link', { name: 'Scan' })).toBeVisible();
		await expect(page.getByRole('link', { name: 'Watcher' })).toBeVisible();
		await expect(page.getByRole('link', { name: 'History' })).toBeVisible();
		await expect(page.getByRole('link', { name: 'Settings' })).toBeVisible();
	});

	test('clicking nav items navigates between views', async ({ page }) => {
		await gotoWithMocks(page, '/scan');

		await page.getByRole('link', { name: 'History' }).click();
		await expect(page).toHaveURL(/\/history/);

		await page.getByRole('link', { name: 'Settings' }).click();
		await expect(page).toHaveURL(/\/settings/);

		await page.getByRole('link', { name: 'Watcher' }).click();
		await expect(page).toHaveURL(/\/watcher/);

		await page.getByRole('link', { name: 'Scan' }).click();
		await expect(page).toHaveURL(/\/scan/);
	});

	test('active nav item is highlighted', async ({ page }) => {
		await gotoWithMocks(page, '/scan');

		const scanLink = page.getByRole('link', { name: 'Scan' });
		const historyLink = page.getByRole('link', { name: 'History' });

		// Scan should be active (has font-medium class)
		await expect(scanLink).toHaveClass(/font-medium/);
		// Also has bg-accent as a standalone class (not hover:bg-accent)
		await expect(scanLink).toHaveClass(/\bbg-accent\b/);

		// History should NOT have font-medium (inactive)
		await expect(historyLink).not.toHaveClass(/font-medium/);
	});

	test('theme toggle switches between dark and light', async ({ page }) => {
		// Emulate dark color scheme so theme.init() picks dark mode
		await page.emulateMedia({ colorScheme: 'dark' });
		await gotoWithMocks(page, '/scan');

		const html = page.locator('html');
		const themeButton = page.getByRole('button', { name: 'Toggle theme' });

		// Default is dark mode (system preference is dark)
		await expect(html).toHaveClass(/dark/);

		// Click to switch to light mode
		await themeButton.click();
		await expect(html).not.toHaveClass(/dark/);

		// Click again to switch back to dark
		await themeButton.click();
		await expect(html).toHaveClass(/dark/);
	});

	test('app title "Mediarr" is visible in sidebar', async ({ page }) => {
		await gotoWithMocks(page, '/scan');

		await expect(page.getByRole('heading', { name: 'Mediarr' })).toBeVisible();
	});
});
