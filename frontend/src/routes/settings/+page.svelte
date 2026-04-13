<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import type { Config, MediaType } from '$lib/types';
	import { configState } from '$lib/state/config.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Tabs from '$lib/components/ui/tabs/index.js';
	import { Save, Loader2 } from '@lucide/svelte';
	import TemplateEditor from '$lib/components/settings/TemplateEditor.svelte';
	import SubtitlePrefs from '$lib/components/settings/SubtitlePrefs.svelte';
	import GeneralSettings from '$lib/components/settings/GeneralSettings.svelte';
	import UpdateChecker from '$lib/components/settings/UpdateChecker.svelte';

	let hasUnsavedChanges = $state(false);
	let savedSnapshot = $state<string>('');

	const templateTypes: { label: string; mediaType: MediaType; key: 'movie' | 'series' }[] = [
		{ label: 'Movie', mediaType: 'Movie', key: 'movie' },
		{ label: 'Series', mediaType: 'Series', key: 'series' },
	];

	onMount(async () => {
		configState.loading = true;
		configState.error = null;
		try {
			configState.config = await invoke<Config>('get_config');
			savedSnapshot = JSON.stringify(configState.config);
		} catch (e) {
			configState.error = (e as Error).message ?? 'Failed to load config';
		} finally {
			configState.loading = false;
		}
	});

	// Track unsaved changes
	$effect(() => {
		if (configState.config && savedSnapshot) {
			hasUnsavedChanges = JSON.stringify(configState.config) !== savedSnapshot;
		}
	});

	async function saveConfig() {
		if (!configState.config) return;
		configState.saving = true;
		configState.error = null;
		try {
			await invoke('update_config', { config: configState.config });
			savedSnapshot = JSON.stringify(configState.config);
			hasUnsavedChanges = false;
		} catch (e) {
			configState.error = (e as Error).message ?? 'Failed to save config';
		} finally {
			configState.saving = false;
		}
	}
</script>

<div class="p-8 max-w-2xl">
	<div class="flex items-center justify-between mb-6">
		<div>
			<h2 class="text-lg font-medium text-foreground">Settings</h2>
			<p class="mt-1 text-sm text-muted-foreground">Configure templates, subtitles, and preferences.</p>
		</div>
		{#if configState.config}
			<Button onclick={saveConfig} disabled={configState.saving || !hasUnsavedChanges} class="focus-ring">
				{#if configState.saving}
					<Loader2 class="size-4 animate-spin" />
					Saving...
				{:else}
					<Save class="size-4" />
					{hasUnsavedChanges ? 'Save Settings' : 'Saved'}
				{/if}
			</Button>
		{/if}
	</div>

	{#if configState.error}
		<div class="mb-4 rounded-md bg-destructive/15 p-3 text-sm text-destructive">{configState.error}</div>
	{/if}

	{#if configState.loading}
		<div class="space-y-4 pt-6">
			<div class="skeleton h-4 w-32"></div>
			{#each [1, 2, 3] as _, i}
				<div class="space-y-2">
					<div class="skeleton h-3.5" style="width: {80 + i * 15}px;"></div>
					<div class="skeleton h-9 w-full"></div>
					<div class="skeleton h-14 w-full rounded-md"></div>
				</div>
			{/each}
		</div>
	{:else if configState.config}
		<Tabs.Root value="templates">
			<Tabs.List variant="line">
				<Tabs.Trigger value="templates">Templates</Tabs.Trigger>
				<Tabs.Trigger value="subtitles">Subtitles</Tabs.Trigger>
				<Tabs.Trigger value="general">General</Tabs.Trigger>
				<Tabs.Trigger value="about">About</Tabs.Trigger>
			</Tabs.List>
			<Tabs.Content value="templates" class="pt-6">
				<div class="space-y-6">
					{#each templateTypes as t}
						<TemplateEditor
							label={t.label}
							mediaType={t.mediaType}
							template={configState.config.templates[t.key]}
							onUpdate={(value) => {
								if (configState.config) {
									configState.config.templates[t.key] = value;
								}
							}}
						/>
					{/each}
				</div>
			</Tabs.Content>
			<Tabs.Content value="subtitles" class="pt-6">
				<SubtitlePrefs
					config={configState.config.subtitles}
					onUpdate={(subtitles) => {
						if (configState.config) {
							configState.config.subtitles = subtitles;
						}
					}}
				/>
			</Tabs.Content>
			<Tabs.Content value="general" class="pt-6">
				<GeneralSettings
					config={configState.config.general}
					onUpdate={(general) => {
						if (configState.config) {
							configState.config.general = general;
						}
					}}
				/>
			</Tabs.Content>
			<Tabs.Content value="about" class="pt-6">
				<UpdateChecker />
			</Tabs.Content>
		</Tabs.Root>
	{/if}
</div>
