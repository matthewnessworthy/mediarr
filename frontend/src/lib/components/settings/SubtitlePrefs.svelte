<script lang="ts">
	import type { SubtitleConfig, NonPreferredAction } from '$lib/types';
	import { Switch } from '$lib/components/ui/switch';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { X } from '@lucide/svelte';
	import DiscoveryToggles from './DiscoveryToggles.svelte';

	interface Props {
		config: SubtitleConfig;
		onUpdate: (config: SubtitleConfig) => void;
	}

	let { config, onUpdate }: Props = $props();

	let languageInput = $state('');

	const nonPreferredOptions: { value: NonPreferredAction; label: string; desc: string }[] = [
		{ value: 'Ignore', label: 'Ignore', desc: 'Leave non-preferred subtitles in place' },
		{ value: 'Backup', label: 'Backup', desc: 'Move to a backup directory' },
		{ value: 'KeepAll', label: 'Keep All', desc: 'Rename all subtitles regardless' },
		{ value: 'Review', label: 'Review', desc: 'Flag for manual review' },
	];

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
		<!-- Naming pattern -->
		<div class="space-y-2">
			<Label class="text-sm font-medium">Subtitle naming pattern</Label>
			<Input
				value={config.naming_pattern}
				oninput={(e: Event) => update({ naming_pattern: (e.target as HTMLInputElement).value })}
				placeholder="{'{'}title{'}'}.{'{'}language{'}'}.{'{'}type{'}'}.{'{'}ext{'}'}"
				class="font-mono text-sm"
			/>
		</div>

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

		<!-- Non-preferred action -->
		<div class="space-y-2">
			<Label class="text-sm font-medium">Non-preferred subtitle action</Label>
			<div class="space-y-2">
				{#each nonPreferredOptions as option}
					<label class="flex items-start gap-3 cursor-pointer group">
						<input
							type="radio"
							name="non_preferred_action"
							value={option.value}
							checked={config.non_preferred_action === option.value}
							onchange={() => update({ non_preferred_action: option.value })}
							class="mt-0.5 accent-primary"
						/>
						<div class="space-y-0.5">
							<span class="text-sm text-foreground group-hover:text-foreground/90">{option.label}</span>
							<p class="text-xs text-muted-foreground">{option.desc}</p>
						</div>
					</label>
				{/each}
			</div>

			{#if config.non_preferred_action === 'Backup'}
				<div class="ml-6 space-y-1">
					<Label class="text-xs text-muted-foreground">Backup path</Label>
					<Input
						value={config.backup_path ?? ''}
						oninput={(e: Event) => update({ backup_path: (e.target as HTMLInputElement).value || null })}
						placeholder="/path/to/subtitle/backups"
						class="text-sm"
					/>
				</div>
			{/if}
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
