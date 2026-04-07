<script lang="ts">
	import { onMount } from 'svelte';
	import { getVersion } from '@tauri-apps/api/app';
	import { updaterState } from '$lib/state/updater.svelte';
	import { Button } from '$lib/components/ui/button';
	import { Label } from '$lib/components/ui/label';
	import { Loader2, Download, CheckCircle, AlertCircle } from '@lucide/svelte';

	let currentVersion = $state<string | null>(null);
	let hasChecked = $state(false);

	onMount(async () => {
		try {
			currentVersion = await getVersion();
		} catch {
			currentVersion = null;
		}
	});

	async function handleCheck() {
		await updaterState.checkForUpdates();
		hasChecked = true;
	}
</script>

<div class="space-y-6">
	<!-- Current version -->
	<div class="space-y-2">
		<Label class="text-sm font-medium">Version</Label>
		<p class="text-sm text-muted-foreground">
			{#if currentVersion}
				Mediarr v{currentVersion}
			{:else}
				Loading version...
			{/if}
		</p>
	</div>

	<!-- Update check -->
	<div class="space-y-3">
		<Label class="text-sm font-medium">Updates</Label>

		{#if updaterState.downloading}
			<!-- Downloading state -->
			<div class="space-y-2">
				<p class="text-sm text-muted-foreground">
					Downloading update{updaterState.totalSize > 0 ? `... ${updaterState.progress}%` : '...'}
				</p>
				<div class="h-2 rounded-full bg-muted">
					<div
						class="h-full rounded-full bg-primary transition-all"
						style="width: {updaterState.progress}%"
					></div>
				</div>
			</div>
		{:else if updaterState.available && updaterState.version}
			<!-- Update available -->
			<div class="space-y-3">
				<div class="flex items-start gap-2">
					<Download class="size-4 mt-0.5 text-primary" />
					<div class="space-y-1">
						<p class="text-sm text-foreground">
							Version {updaterState.version} is available
						</p>
						{#if updaterState.body}
							<p class="text-xs text-muted-foreground whitespace-pre-line">{updaterState.body}</p>
						{/if}
					</div>
				</div>
				<Button onclick={() => updaterState.downloadAndInstall()} class="focus-ring">
					<Download class="size-4" />
					Download & Install
				</Button>
			</div>
		{:else if updaterState.error}
			<!-- Error state -->
			<div class="flex items-start gap-2">
				<AlertCircle class="size-4 mt-0.5 text-destructive" />
				<p class="text-sm text-destructive">{updaterState.error}</p>
			</div>
		{:else if hasChecked && !updaterState.available && !updaterState.checking}
			<!-- Up to date -->
			<div class="flex items-center gap-2">
				<CheckCircle class="size-4 text-muted-foreground" />
				<p class="text-sm text-muted-foreground">You're up to date</p>
			</div>
		{/if}

		{#if !updaterState.downloading}
			<Button
				variant="outline"
				onclick={handleCheck}
				disabled={updaterState.checking}
				class="focus-ring"
			>
				{#if updaterState.checking}
					<Loader2 class="size-4 animate-spin" />
					Checking...
				{:else}
					Check for Updates
				{/if}
			</Button>
		{/if}
	</div>
</div>
