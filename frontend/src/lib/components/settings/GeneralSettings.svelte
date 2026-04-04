<script lang="ts">
	import type { GeneralConfig, RenameOperation, ConflictStrategy } from '$lib/types';
	import { Switch } from '$lib/components/ui/switch';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Button } from '$lib/components/ui/button';
	import { FolderOpen, X } from '@lucide/svelte';

	interface Props {
		config: GeneralConfig;
		onUpdate: (config: GeneralConfig) => void;
	}

	let { config, onUpdate }: Props = $props();

	function update(partial: Partial<GeneralConfig>) {
		onUpdate({ ...config, ...partial });
	}

	async function browseOutputDir() {
		try {
			const { open } = await import('@tauri-apps/plugin-dialog');
			const selected = await open({ directory: true, title: 'Select output directory' });
			if (selected) {
				update({ output_dir: selected as string });
			}
		} catch {
			// Dialog cancelled or not available
		}
	}

	const operationOptions: { value: RenameOperation; label: string }[] = [
		{ value: 'Move', label: 'Move files' },
		{ value: 'Copy', label: 'Copy files' },
	];

	const conflictOptions: { value: ConflictStrategy; label: string; desc: string }[] = [
		{ value: 'Skip', label: 'Skip', desc: 'Leave existing file, skip this rename' },
		{ value: 'Overwrite', label: 'Overwrite', desc: 'Replace existing file' },
		{ value: 'NumericSuffix', label: 'Numeric suffix', desc: 'Append (1), (2), etc.' },
	];
</script>

<div class="space-y-6">
	<!-- Output directory -->
	<div class="space-y-2">
		<Label class="text-sm font-medium">Output directory</Label>
		<p class="text-xs text-muted-foreground">
			{#if config.output_dir}
				Files will be renamed into this directory.
			{:else}
				In-place rename (files stay in their current location).
			{/if}
		</p>
		<div class="flex items-center gap-2">
			<Input
				value={config.output_dir ?? ''}
				oninput={(e: Event) => update({ output_dir: (e.target as HTMLInputElement).value || null })}
				placeholder="In-place rename"
				class="text-sm"
			/>
			<Button variant="outline" size="icon" onclick={browseOutputDir}>
				<FolderOpen class="size-4" />
			</Button>
			{#if config.output_dir}
				<Button variant="ghost" size="icon" onclick={() => update({ output_dir: null })}>
					<X class="size-4" />
				</Button>
			{/if}
		</div>
	</div>

	<!-- Operation mode -->
	<div class="space-y-2">
		<Label class="text-sm font-medium">Operation mode</Label>
		<div class="flex gap-4">
			{#each operationOptions as option}
				<label class="flex items-center gap-2 cursor-pointer">
					<input
						type="radio"
						name="operation"
						value={option.value}
						checked={config.operation === option.value}
						onchange={() => update({ operation: option.value })}
						class="accent-primary"
					/>
					<span class="text-sm text-foreground">{option.label}</span>
				</label>
			{/each}
		</div>
	</div>

	<!-- Conflict strategy -->
	<div class="space-y-2">
		<Label class="text-sm font-medium">Conflict strategy</Label>
		<div class="space-y-2">
			{#each conflictOptions as option}
				<label class="flex items-start gap-3 cursor-pointer group">
					<input
						type="radio"
						name="conflict_strategy"
						value={option.value}
						checked={config.conflict_strategy === option.value}
						onchange={() => update({ conflict_strategy: option.value })}
						class="mt-0.5 accent-primary"
					/>
					<div class="space-y-0.5">
						<span class="text-sm text-foreground group-hover:text-foreground/90">{option.label}</span>
						<p class="text-xs text-muted-foreground">{option.desc}</p>
					</div>
				</label>
			{/each}
		</div>
	</div>

	<!-- Create directories -->
	<div class="flex items-center justify-between gap-4">
		<div class="space-y-0.5">
			<Label class="text-sm font-medium">Create directories</Label>
			<p class="text-xs text-muted-foreground">Automatically create folders in naming template paths</p>
		</div>
		<Switch
			checked={config.create_directories}
			onCheckedChange={(checked: boolean) => update({ create_directories: checked })}
		/>
	</div>
</div>
