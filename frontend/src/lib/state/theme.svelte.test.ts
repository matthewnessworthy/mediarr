import { describe, it, expect, beforeEach, vi } from 'vitest';
import { flushSync } from 'svelte';
import { themeState } from './theme.svelte';

// Mock matchMedia for jsdom (not natively available)
function mockMatchMedia(prefersDark: boolean) {
	Object.defineProperty(window, 'matchMedia', {
		writable: true,
		value: vi.fn().mockImplementation((query: string) => ({
			matches: query === '(prefers-color-scheme: dark)' ? prefersDark : false,
			media: query,
			onchange: null,
			addListener: vi.fn(),
			removeListener: vi.fn(),
			addEventListener: vi.fn(),
			removeEventListener: vi.fn(),
			dispatchEvent: vi.fn(),
		})),
	});
}

beforeEach(() => {
	// Reset theme to default
	themeState.mode = 'dark';
	// Clear localStorage
	localStorage.clear();
	// Reset document classes
	document.documentElement.classList.remove('dark', 'light');
	// Default: system prefers dark
	mockMatchMedia(true);
});

describe('ThemeState', () => {
	it('default mode is dark', () => {
		expect(themeState.mode).toBe('dark');
	});

	it('toggle switches from dark to light', () => {
		themeState.mode = 'dark';
		flushSync();
		themeState.toggle();
		flushSync();
		expect(themeState.mode).toBe('light');
	});

	it('toggle switches from light to dark', () => {
		themeState.mode = 'light';
		flushSync();
		themeState.toggle();
		flushSync();
		expect(themeState.mode).toBe('dark');
	});

	it('init reads from localStorage if value present', () => {
		localStorage.setItem('mediarr-theme', 'light');
		themeState.init();
		flushSync();
		expect(themeState.mode).toBe('light');
	});

	it('init defaults to dark when localStorage is empty', () => {
		themeState.mode = 'dark';
		themeState.init();
		flushSync();
		expect(themeState.mode).toBe('dark');
	});

	it('toggle persists to localStorage', () => {
		themeState.mode = 'dark';
		flushSync();
		themeState.toggle();
		flushSync();
		expect(localStorage.getItem('mediarr-theme')).toBe('light');
	});

	it('init respects system light preference when no localStorage', () => {
		mockMatchMedia(false); // system prefers light
		themeState.init();
		flushSync();
		expect(themeState.mode).toBe('light');
	});

	it('init respects system dark preference when no localStorage', () => {
		mockMatchMedia(true); // system prefers dark
		themeState.mode = 'light'; // start from light to prove init changes it
		flushSync();
		themeState.init();
		flushSync();
		expect(themeState.mode).toBe('dark');
	});

	it('toggle applies correct CSS classes to document', () => {
		themeState.mode = 'dark';
		flushSync();
		themeState.toggle();
		flushSync();
		expect(document.documentElement.classList.contains('light')).toBe(true);
		expect(document.documentElement.classList.contains('dark')).toBe(false);
	});
});
