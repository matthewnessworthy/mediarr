<script lang="ts">
	import type { RenameRecord } from '$lib/types';
	import { Badge } from '$lib/components/ui/badge';

	const { record }: { record: RenameRecord } = $props();

	function formatFileSize(bytes: number): string {
		if (bytes >= 1_073_741_824) {
			return `${(bytes / 1_073_741_824).toFixed(1)} GB`;
		}
		if (bytes >= 1_048_576) {
			return `${(bytes / 1_048_576).toFixed(0)} MB`;
		}
		if (bytes >= 1024) {
			return `${(bytes / 1024).toFixed(0)} KB`;
		}
		return `${bytes} B`;
	}

	import { basename } from '$lib/utils.js';
	const filename = basename;
</script>

<div class="flex items-center gap-3 py-1.5 text-xs">
	<Badge variant="outline" class="shrink-0 text-[10px] font-normal">
		{record.media_info.media_type}
	</Badge>
	<span class="truncate font-mono text-muted-foreground" title={record.source_path}>
		{filename(record.source_path)}
	</span>
	<span class="shrink-0 text-muted-foreground/50">&rarr;</span>
	<span class="truncate font-mono text-foreground/80" title={record.dest_path}>
		{filename(record.dest_path)}
	</span>
	<span class="ml-auto shrink-0 tabular-nums text-muted-foreground/60">
		{formatFileSize(record.file_size)}
	</span>
</div>
