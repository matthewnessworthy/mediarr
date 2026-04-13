import type { Config } from '$lib/types';

class ConfigState {
	config = $state<Config | null>(null);
	loading = $state(false);
	saving = $state(false);
	error = $state<string | null>(null);
}

export const configState = new ConfigState();
