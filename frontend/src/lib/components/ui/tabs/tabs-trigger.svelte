<script lang="ts">
	import { Tabs as TabsPrimitive } from "bits-ui";
	import { cn } from "$lib/utils.js";

	let {
		ref = $bindable(null),
		class: className,
		...restProps
	}: TabsPrimitive.TriggerProps = $props();
</script>

<TabsPrimitive.Trigger
	bind:ref
	data-slot="tabs-trigger"
	class={cn(
		"relative inline-flex items-center justify-center whitespace-nowrap text-sm font-medium transition-colors duration-100",
		"gap-1.5 [&_svg:not([class*='size-'])]:size-4 [&_svg]:pointer-events-none [&_svg]:shrink-0",
		"text-muted-foreground hover:text-foreground/80 data-[state=active]:text-foreground",
		"focus-visible:outline-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] focus-visible:outline-1",
		"disabled:pointer-events-none disabled:opacity-50",
		"group-data-[orientation=vertical]/tabs:w-full group-data-[orientation=vertical]/tabs:justify-start",
		// Default variant: pill-style with background
		"group-data-[variant=default]/tabs-list:rounded-md group-data-[variant=default]/tabs-list:border group-data-[variant=default]/tabs-list:border-transparent group-data-[variant=default]/tabs-list:px-3 group-data-[variant=default]/tabs-list:py-1 group-data-[variant=default]/tabs-list:h-[calc(100%-1px)] group-data-[variant=default]/tabs-list:flex-1",
		"group-data-[variant=default]/tabs-list:data-[state=active]:bg-background group-data-[variant=default]/tabs-list:data-[state=active]:shadow-sm",
		// Line variant: underline indicator
		"group-data-[variant=line]/tabs-list:px-3 group-data-[variant=line]/tabs-list:py-2 group-data-[variant=line]/tabs-list:bg-transparent group-data-[variant=line]/tabs-list:rounded-none",
		"after:absolute after:opacity-0 after:transition-opacity",
		"group-data-[orientation=horizontal]/tabs:after:inset-x-0 group-data-[orientation=horizontal]/tabs:after:bottom-[-1px] group-data-[orientation=horizontal]/tabs:after:h-px",
		"group-data-[orientation=vertical]/tabs:after:inset-y-0 group-data-[orientation=vertical]/tabs:after:-right-1 group-data-[orientation=vertical]/tabs:after:w-0.5",
		"after:bg-foreground group-data-[variant=line]/tabs-list:data-[state=active]:after:opacity-100",
		className
	)}
	{...restProps}
/>
