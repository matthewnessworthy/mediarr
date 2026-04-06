<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { ChevronDown } from '@lucide/svelte';
	import type { MediaType, MediaInfo, TemplateWarning } from '$lib/types';
	import { Label } from '$lib/components/ui/label';
	import { Input } from '$lib/components/ui/input';
	import * as Collapsible from '$lib/components/ui/collapsible';

	interface Props {
		label: string;
		mediaType: MediaType;
		template: string;
		onUpdate: (value: string) => void;
	}

	let { label, mediaType, template, onUpdate }: Props = $props();

	let preview = $state('');
	let warnings = $state<TemplateWarning[]>([]);
	let debounceTimer = $state<ReturnType<typeof setTimeout> | null>(null);
	let variablesOpen = $state(false);

	const SAMPLE_MEDIA: Record<MediaType, MediaInfo> = {
		Movie: {
			title: 'Inception',
			media_type: 'Movie',
			year: 2010,
			season: null,
			episodes: [],
			resolution: '1080p',
			video_codec: 'x264',
			audio_codec: 'DTS',
			source: 'BluRay',
			release_group: 'SPARKS',
			container: 'mkv',
			language: 'en',
			confidence: 'High',
		},
		Series: {
			title: 'The Office',
			media_type: 'Series',
			year: 2005,
			season: 2,
			episodes: [3],
			resolution: '720p',
			video_codec: 'x264',
			audio_codec: 'AAC',
			source: 'BluRay',
			release_group: 'DEMAND',
			container: 'mkv',
			language: 'en',
			confidence: 'High',
		},
		Anime: {
			title: 'Steins Gate',
			media_type: 'Anime',
			year: 2011,
			season: 1,
			episodes: [12],
			resolution: '1080p',
			video_codec: 'x265',
			audio_codec: 'FLAC',
			source: 'BluRay',
			release_group: 'SubsPlease',
			container: 'mkv',
			language: 'ja',
			confidence: 'High',
		},
	};

	const AVAILABLE_VARIABLES = [
		{ name: '{title}', desc: 'Media title' },
		{ name: '{year}', desc: 'Release year' },
		{ name: '{season}', desc: 'Season number' },
		{ name: '{season:02}', desc: 'Season, zero-padded' },
		{ name: '{episode}', desc: 'Episode number' },
		{ name: '{episode:02}', desc: 'Episode, zero-padded' },
		{ name: '{resolution}', desc: 'e.g. 1080p, 720p' },
		{ name: '{video_codec}', desc: 'Video codec' },
		{ name: '{audio_codec}', desc: 'Audio codec' },
		{ name: '{source}', desc: 'e.g. BluRay, WEB-DL' },
		{ name: '{release_group}', desc: 'Release group' },
		{ name: '{language}', desc: 'Content language' },
		{ name: '{ext}', desc: 'File extension' },
	];

	async function updatePreview() {
		if (!template) {
			preview = '';
			warnings = [];
			return;
		}
		try {
			const [previewResult, warningResult] = await Promise.all([
				invoke<string>('preview_template', {
					template,
					mediaInfo: SAMPLE_MEDIA[mediaType],
				}),
				invoke<TemplateWarning[]>('validate_template', {
					template,
					mediaType,
				}),
			]);
			preview = previewResult;
			warnings = warningResult;
		} catch (e) {
			preview = `Error: ${e}`;
			warnings = [];
		}
	}

	function handleInput(event: Event) {
		const target = event.target as HTMLInputElement;
		onUpdate(target.value);
		if (debounceTimer) clearTimeout(debounceTimer);
		debounceTimer = setTimeout(updatePreview, 300);
	}

	// Trigger initial preview on mount
	$effect(() => {
		if (template) {
			updatePreview();
		}
	});
</script>

<div class="space-y-2">
	<Label class="text-sm font-medium">{label} Template</Label>
	<Input
		value={template}
		oninput={handleInput}
		placeholder="{'{'}title{'}'} ({'{'}year{'}'})/...{'{'}ext{'}'}"
		class="font-mono text-sm"
	/>

	{#if preview}
		<div class="rounded-md border border-border/50 bg-muted/30 px-3 py-2">
			<p class="text-[11px] uppercase tracking-wide text-muted-foreground/60 mb-1">Preview</p>
			<p class="font-mono text-sm text-foreground break-all">{preview}</p>
		</div>
	{/if}

	{#if warnings.length > 0}
		<div class="space-y-1">
			{#each warnings as warning}
				<p class="text-xs text-amber-500">
					<span class="font-mono">{warning.variable}</span>: {warning.message}
				</p>
			{/each}
		</div>
	{/if}

	<Collapsible.Root bind:open={variablesOpen}>
		<Collapsible.Trigger
			class="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
			style="transition-duration: var(--duration-fast);"
		>
			<ChevronDown
				class="size-3 transition-transform {variablesOpen ? 'rotate-0' : '-rotate-90'}"
				style="transition-duration: var(--duration-fast);"
			/>
			Available variables
		</Collapsible.Trigger>
		<Collapsible.Content>
			<div class="mt-2 grid grid-cols-2 gap-x-4 gap-y-1">
				{#each AVAILABLE_VARIABLES as v}
					<div class="flex items-baseline gap-2 text-xs">
						<code class="font-mono text-foreground/80">{v.name}</code>
						<span class="text-muted-foreground">{v.desc}</span>
					</div>
				{/each}
			</div>
		</Collapsible.Content>
	</Collapsible.Root>
</div>
