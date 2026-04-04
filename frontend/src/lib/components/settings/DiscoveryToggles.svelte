<script lang="ts">
	import type { DiscoveryToggles } from '$lib/types';
	import { Switch } from '$lib/components/ui/switch';
	import { Label } from '$lib/components/ui/label';

	interface Props {
		toggles: DiscoveryToggles;
		onUpdate: (toggles: DiscoveryToggles) => void;
	}

	let { toggles, onUpdate }: Props = $props();

	const items = [
		{
			key: 'sidecar' as const,
			label: 'Sidecar files',
			desc: 'Same directory, matching filename',
		},
		{
			key: 'subs_subfolder' as const,
			label: 'Subs subfolder',
			desc: 'Look in Subs/ and Subtitles/ folders',
		},
		{
			key: 'nested_language_folders' as const,
			label: 'Language folders',
			desc: 'Scan English/, Japanese/ subfolders',
		},
		{
			key: 'vobsub_pairs' as const,
			label: 'VobSub pairs',
			desc: 'Paired .idx + .sub files',
		},
	];

	function handleToggle(key: keyof DiscoveryToggles, checked: boolean) {
		onUpdate({ ...toggles, [key]: checked });
	}
</script>

<div class="space-y-3">
	{#each items as item}
		<div class="flex items-start justify-between gap-4">
			<div class="space-y-0.5">
				<Label class="text-sm">{item.label}</Label>
				<p class="text-xs text-muted-foreground">{item.desc}</p>
			</div>
			<Switch
				checked={toggles[item.key]}
				onCheckedChange={(checked: boolean) => handleToggle(item.key, checked)}
				size="sm"
			/>
		</div>
	{/each}
</div>
