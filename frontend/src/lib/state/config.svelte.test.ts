import { describe, it, expect, beforeEach } from 'vitest';
import { flushSync } from 'svelte';
import { configState } from './config.svelte';
import { mockConfig } from '../../test/fixtures';

beforeEach(() => {
	configState.config = null;
	configState.loading = false;
	configState.saving = false;
});

describe('ConfigState', () => {
	it('initial state has config null, loading false, saving false', () => {
		expect(configState.config).toBeNull();
		expect(configState.loading).toBe(false);
		expect(configState.saving).toBe(false);
	});

	it('config can be set to a valid Config object and read back', () => {
		const cfg = mockConfig({ general: { output_dir: '/custom', operation: 'Copy', conflict_strategy: 'Overwrite', create_directories: false } });
		configState.config = cfg;
		flushSync();
		expect(configState.config).not.toBeNull();
		expect(configState.config!.general.output_dir).toBe('/custom');
		expect(configState.config!.general.operation).toBe('Copy');
	});

	it('loading and saving flags can be toggled', () => {
		configState.loading = true;
		flushSync();
		expect(configState.loading).toBe(true);

		configState.saving = true;
		flushSync();
		expect(configState.saving).toBe(true);
	});
});
