<script lang="ts">
	import { page } from '$app/stores';
	import { ScanSearch, Eye, Clock, Settings } from '@lucide/svelte';
	import '../app.css';

	const { children } = $props();

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
					class="flex items-center gap-3 px-3 py-2 rounded-md text-sm transition-colors duration-150
								 {$page.url.pathname.startsWith(item.href)
						? 'bg-accent text-accent-foreground font-medium'
						: 'text-muted-foreground hover:bg-accent/50 hover:text-foreground'}"
				>
					<item.icon class="size-4 shrink-0" />
					{item.label}
				</a>
			{/each}
		</div>
	</nav>
	<main class="flex-1 overflow-auto">
		{@render children()}
	</main>
</div>
