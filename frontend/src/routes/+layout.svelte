<script lang="ts">
	import { page } from '$app/stores';
	import { onMount } from 'svelte';
	import { ScanSearch, Eye, Clock, Settings, Sun, Moon } from '@lucide/svelte';
	import { themeState } from '$lib/state/theme.svelte';
	import '../app.css';

	const { children } = $props();

	onMount(() => {
		themeState.init();
	});

	const navItems = [
		{ href: '/scan', label: 'Scan', icon: ScanSearch },
		{ href: '/watcher', label: 'Watcher', icon: Eye },
		{ href: '/history', label: 'History', icon: Clock },
		{ href: '/settings', label: 'Settings', icon: Settings },
	];
</script>

<div class="flex h-screen bg-background text-foreground">
	<nav class="w-52 border-r border-border flex flex-col pt-6 pb-4 shrink-0">
		<div class="px-5 mb-8">
			<h1 class="text-sm font-semibold tracking-wide text-foreground/70 uppercase">Mediarr</h1>
		</div>
		<div class="flex flex-col gap-0.5 px-3">
			{#each navItems as item}
				<a
					href={item.href}
					class="flex items-center gap-3 px-3 py-2 rounded-md text-sm transition-colors focus-ring
								 {$page.url.pathname.startsWith(item.href)
						? 'bg-accent text-accent-foreground font-medium'
						: 'text-muted-foreground hover:bg-accent/50 hover:text-foreground'}"
					style="transition-duration: var(--duration-fast);"
				>
					<item.icon class="size-4 shrink-0" />
					{item.label}
				</a>
			{/each}
		</div>

		<!-- Spacer -->
		<div class="flex-1"></div>

		<!-- Theme toggle -->
		<div class="px-3 mt-4">
			<button
				onclick={() => themeState.toggle()}
				class="flex items-center gap-3 px-3 py-2 rounded-md text-sm text-muted-foreground hover:bg-accent/50 hover:text-foreground transition-colors w-full focus-ring"
				style="transition-duration: var(--duration-fast);"
				aria-label="Toggle theme"
			>
				{#if themeState.mode === 'dark'}
					<Sun class="size-4 shrink-0" />
					<span>Light mode</span>
				{:else}
					<Moon class="size-4 shrink-0" />
					<span>Dark mode</span>
				{/if}
			</button>
		</div>
	</nav>
	<main class="flex-1 overflow-auto">
		{#key $page.url.pathname}
			<div class="page-enter h-full">
				{@render children()}
			</div>
		{/key}
	</main>
</div>
