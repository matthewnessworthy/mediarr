<script lang="ts">
	import type { SubtitleConfig } from '$lib/types';
	import { Switch } from '$lib/components/ui/switch';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Badge } from '$lib/components/ui/badge';
	import { X } from '@lucide/svelte';
	import DiscoveryToggles from './DiscoveryToggles.svelte';

	interface Props {
		config: SubtitleConfig;
		onUpdate: (config: SubtitleConfig) => void;
	}

	let { config, onUpdate }: Props = $props();

	let languageInput = $state('');

	function update(partial: Partial<SubtitleConfig>) {
		onUpdate({ ...config, ...partial });
	}

	function addLanguage() {
		const code = languageInput.trim().toLowerCase();
		if (code.length >= 2 && code.length <= 3 && !config.preferred_languages.includes(code)) {
			update({ preferred_languages: [...config.preferred_languages, code] });
		}
		languageInput = '';
	}

	function removeLanguage(code: string) {
		update({
			preferred_languages: config.preferred_languages.filter((l) => l !== code),
		});
	}

	function handleLanguageKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter' || event.key === 'Tab') {
			event.preventDefault();
			addLanguage();
		}
	}
</script>

<div class="space-y-6">
	<!-- Enable/disable -->
	<div class="flex items-center justify-between gap-4">
		<div class="space-y-0.5">
			<Label class="text-sm font-medium">Subtitle handling</Label>
			<p class="text-xs text-muted-foreground">Discover and rename subtitle files alongside video</p>
		</div>
		<Switch
			checked={config.enabled}
			onCheckedChange={(checked: boolean) => update({ enabled: checked })}
		/>
	</div>

	{#if config.enabled}
		<!-- Preferred languages -->
		<div class="space-y-2">
			<Label class="text-sm font-medium">Preferred languages</Label>
			<p class="text-xs text-muted-foreground">ISO 639-1 codes (e.g. en, ja, fr). Press Enter to add.</p>
			<div class="flex flex-wrap items-center gap-1.5">
				{#each config.preferred_languages as lang}
					<Badge variant="secondary" class="gap-1 pl-2 pr-1">
						{lang}
						<button
							type="button"
							onclick={() => removeLanguage(lang)}
							class="rounded-full p-0.5 hover:bg-muted-foreground/20 transition-colors"
						>
							<X class="size-3" />
						</button>
					</Badge>
				{/each}
				<Input
					bind:value={languageInput}
					onkeydown={handleLanguageKeydown}
					placeholder="Add..."
					class="h-6 w-16 min-w-0 border-none bg-transparent px-1 text-sm shadow-none focus-visible:ring-0"
				/>
			</div>
		</div>

		<!-- Discovery toggles -->
		<div class="space-y-2">
			<Label class="text-sm font-medium">Discovery methods</Label>
			<DiscoveryToggles
				toggles={config.discovery}
				onUpdate={(discovery) => update({ discovery })}
			/>
		</div>
	{/if}
</div>
