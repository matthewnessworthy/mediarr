<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import type { Config, MediaType } from '$lib/types';
	import { configState } from '$lib/state/config.svelte';
	import { Button } from '$lib/components/ui/button';
	import { Separator } from '$lib/components/ui/separator';
	import { Save, Loader2 } from '@lucide/svelte';
	import TemplateEditor from '$lib/components/settings/TemplateEditor.svelte';
	import SubtitlePrefs from '$lib/components/settings/SubtitlePrefs.svelte';
	import GeneralSettings from '$lib/components/settings/GeneralSettings.svelte';

	let hasUnsavedChanges = $state(false);
	let savedSnapshot = $state<string>('');

	const templateTypes: { label: string; mediaType: MediaType; key: 'movie' | 'series' | 'anime' }[] = [
		{ label: 'Movie', mediaType: 'Movie', key: 'movie' },
		{ label: 'Series', mediaType: 'Series', key: 'series' },
		{ label: 'Anime', mediaType: 'Anime', key: 'anime' },
	];

	onMount(async () => {
		configState.loading = true;
		try {
			configState.config = await invoke<Config>('get_config');
			savedSnapshot = JSON.stringify(configState.config);
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
		try {
			await invoke('update_config', { config: configState.config });
			savedSnapshot = JSON.stringify(configState.config);
			hasUnsavedChanges = false;
		} finally {
			configState.saving = false;
		}
	}
</script>

<div class="p-8 max-w-2xl">
	<div class="flex items-center justify-between mb-8">
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

	{#if configState.loading}
		<div class="space-y-10">
			<!-- Templates skeleton -->
			<section class="space-y-4">
				<div class="skeleton h-4 w-32"></div>
				{#each [1, 2, 3] as _, i}
					<div class="space-y-2">
						<div class="skeleton h-3.5" style="width: {80 + i * 15}px;"></div>
						<div class="skeleton h-9 w-full"></div>
						<div class="skeleton h-14 w-full rounded-md"></div>
					</div>
				{/each}
			</section>
			<div class="h-px bg-border"></div>
			<!-- Subtitles skeleton -->
			<section class="space-y-3">
				<div class="skeleton h-4 w-20"></div>
				<div class="skeleton h-9 w-full"></div>
				<div class="skeleton h-9 w-3/4"></div>
			</section>
			<div class="h-px bg-border"></div>
			<!-- General skeleton -->
			<section class="space-y-3">
				<div class="skeleton h-4 w-16"></div>
				<div class="skeleton h-9 w-full"></div>
				<div class="skeleton h-9 w-2/3"></div>
			</section>
		</div>
	{:else if configState.config}
		<div class="space-y-10">
			<!-- Naming Templates -->
			<section>
				<h3 class="text-sm font-medium text-foreground mb-4">Naming Templates</h3>
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
			</section>

			<Separator />

			<!-- Subtitles -->
			<section>
				<h3 class="text-sm font-medium text-foreground mb-4">Subtitles</h3>
				<SubtitlePrefs
					config={configState.config.subtitles}
					onUpdate={(subtitles) => {
						if (configState.config) {
							configState.config.subtitles = subtitles;
						}
					}}
				/>
			</section>

			<Separator />

			<!-- General -->
			<section>
				<h3 class="text-sm font-medium text-foreground mb-4">General</h3>
				<GeneralSettings
					config={configState.config.general}
					onUpdate={(general) => {
						if (configState.config) {
							configState.config.general = general;
						}
					}}
				/>
			</section>
		</div>

		<!-- Bottom save button -->
		{#if hasUnsavedChanges}
			<div class="mt-8 pt-6 border-t border-border">
				<Button onclick={saveConfig} disabled={configState.saving} class="focus-ring">
					{#if configState.saving}
						<Loader2 class="size-4 animate-spin" />
						Saving...
					{:else}
						<Save class="size-4" />
						Save Settings
					{/if}
				</Button>
			</div>
		{/if}
	{/if}
</div>
