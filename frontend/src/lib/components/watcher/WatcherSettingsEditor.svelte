<script lang="ts">
	import { Switch } from '$lib/components/ui/switch';
	import type { Config, WatcherSettings, RenameOperation, ConflictStrategy, NonPreferredAction } from '$lib/types';

	let {
		settings = $bindable<WatcherSettings>({}),
		globalConfig,
	}: {
		settings: WatcherSettings;
		globalConfig: Config;
	} = $props();

	function languagesDisplay(langs: string[] | undefined | null): string {
		if (!langs || langs.length === 0) return '';
		return langs.join(', ');
	}

	function parseLanguages(value: string): string[] {
		return value.split(',').map(s => s.trim()).filter(s => s.length > 0);
	}
</script>

<div class="flex flex-col gap-1">
	<!-- General -->
	<h4 class="text-xs font-medium text-foreground/70 uppercase tracking-wider mb-2">General</h4>

	<!-- Output Directory -->
	<div class="space-y-1.5">
		<div class="flex items-center justify-between">
			<span class="text-xs font-medium text-muted-foreground">Output Directory</span>
			<div class="flex items-center gap-2">
				<span class="text-[10px] text-muted-foreground/60">Override</span>
				<Switch
					checked={settings.output_dir != null}
					onCheckedChange={(checked: boolean) => {
						if (checked) {
							settings.output_dir = globalConfig.general.output_dir ?? '';
						} else {
							settings.output_dir = undefined;
						}
					}}
					size="sm"
				/>
			</div>
		</div>
		<input
			type="text"
			value={settings.output_dir != null ? settings.output_dir : (globalConfig.general.output_dir ?? 'In-place rename')}
			disabled={settings.output_dir == null}
			oninput={(e) => { settings.output_dir = (e.currentTarget as HTMLInputElement).value; }}
			class="w-full rounded-md border border-input bg-background px-3 py-1.5 text-sm {settings.output_dir == null ? 'opacity-50 cursor-not-allowed' : ''}"
		/>
		{#if settings.output_dir != null}
			<p class="text-[10px] text-muted-foreground/60">Leave empty for in-place rename</p>
		{/if}
	</div>

	<!-- Operation -->
	<div class="space-y-1.5 mt-3">
		<div class="flex items-center justify-between">
			<span class="text-xs font-medium text-muted-foreground">Operation</span>
			<div class="flex items-center gap-2">
				<span class="text-[10px] text-muted-foreground/60">Override</span>
				<Switch
					checked={settings.operation != null}
					onCheckedChange={(checked: boolean) => {
						if (checked) {
							settings.operation = globalConfig.general.operation;
						} else {
							settings.operation = undefined;
						}
					}}
					size="sm"
				/>
			</div>
		</div>
		<div class="flex items-center gap-4 {settings.operation == null ? 'opacity-50 pointer-events-none' : ''}">
			{#each ['Move', 'Copy'] as op}
				<label class="flex items-center gap-2 text-sm cursor-pointer">
					<input
						type="radio"
						name="watcher-settings-operation"
						value={op}
						checked={(settings.operation != null ? settings.operation : globalConfig.general.operation) === op}
						disabled={settings.operation == null}
						onchange={() => { settings.operation = op as RenameOperation; }}
						class="accent-primary"
					/>
					{op}
				</label>
			{/each}
		</div>
	</div>

	<!-- Conflict Strategy -->
	<div class="space-y-1.5 mt-3">
		<div class="flex items-center justify-between">
			<span class="text-xs font-medium text-muted-foreground">Conflict Strategy</span>
			<div class="flex items-center gap-2">
				<span class="text-[10px] text-muted-foreground/60">Override</span>
				<Switch
					checked={settings.conflict_strategy != null}
					onCheckedChange={(checked: boolean) => {
						if (checked) {
							settings.conflict_strategy = globalConfig.general.conflict_strategy;
						} else {
							settings.conflict_strategy = undefined;
						}
					}}
					size="sm"
				/>
			</div>
		</div>
		<div class="flex items-center gap-4 flex-wrap {settings.conflict_strategy == null ? 'opacity-50 pointer-events-none' : ''}">
			{#each [['Skip', 'Skip'], ['Overwrite', 'Overwrite'], ['NumericSuffix', 'Numeric Suffix']] as [value, label]}
				<label class="flex items-center gap-2 text-sm cursor-pointer">
					<input
						type="radio"
						name="watcher-settings-conflict"
						value={value}
						checked={(settings.conflict_strategy != null ? settings.conflict_strategy : globalConfig.general.conflict_strategy) === value}
						disabled={settings.conflict_strategy == null}
						onchange={() => { settings.conflict_strategy = value as ConflictStrategy; }}
						class="accent-primary"
					/>
					{label}
				</label>
			{/each}
		</div>
	</div>

	<!-- Create Directories -->
	<div class="space-y-1.5 mt-3">
		<div class="flex items-center justify-between">
			<span class="text-xs font-medium text-muted-foreground">Create Directories</span>
			<div class="flex items-center gap-2">
				<span class="text-[10px] text-muted-foreground/60">Override</span>
				<Switch
					checked={settings.create_directories != null}
					onCheckedChange={(checked: boolean) => {
						if (checked) {
							settings.create_directories = globalConfig.general.create_directories;
						} else {
							settings.create_directories = undefined;
						}
					}}
					size="sm"
				/>
			</div>
		</div>
		<div class="flex items-center gap-2 {settings.create_directories == null ? 'opacity-50 pointer-events-none' : ''}">
			<Switch
				checked={settings.create_directories != null ? settings.create_directories : globalConfig.general.create_directories}
				onCheckedChange={(checked: boolean) => { settings.create_directories = checked; }}
				disabled={settings.create_directories == null}
				size="sm"
			/>
			<span class="text-sm text-muted-foreground">
				{(settings.create_directories != null ? settings.create_directories : globalConfig.general.create_directories) ? 'Yes' : 'No'}
			</span>
		</div>
	</div>

	<!-- Templates -->
	<h4 class="text-xs font-medium text-foreground/70 uppercase tracking-wider mt-5 mb-2">Templates</h4>

	<!-- Movie Template -->
	<div class="space-y-1.5">
		<div class="flex items-center justify-between">
			<span class="text-xs font-medium text-muted-foreground">Movie Template</span>
			<div class="flex items-center gap-2">
				<span class="text-[10px] text-muted-foreground/60">Override</span>
				<Switch
					checked={settings.movie_template != null}
					onCheckedChange={(checked: boolean) => {
						if (checked) {
							settings.movie_template = globalConfig.templates.movie;
						} else {
							settings.movie_template = undefined;
						}
					}}
					size="sm"
				/>
			</div>
		</div>
		<input
			type="text"
			value={settings.movie_template != null ? settings.movie_template : globalConfig.templates.movie}
			disabled={settings.movie_template == null}
			oninput={(e) => { settings.movie_template = (e.currentTarget as HTMLInputElement).value; }}
			class="w-full rounded-md border border-input bg-background px-3 py-1.5 text-sm font-mono text-xs {settings.movie_template == null ? 'opacity-50 cursor-not-allowed' : ''}"
		/>
	</div>

	<!-- Series Template -->
	<div class="space-y-1.5 mt-3">
		<div class="flex items-center justify-between">
			<span class="text-xs font-medium text-muted-foreground">Series Template</span>
			<div class="flex items-center gap-2">
				<span class="text-[10px] text-muted-foreground/60">Override</span>
				<Switch
					checked={settings.series_template != null}
					onCheckedChange={(checked: boolean) => {
						if (checked) {
							settings.series_template = globalConfig.templates.series;
						} else {
							settings.series_template = undefined;
						}
					}}
					size="sm"
				/>
			</div>
		</div>
		<input
			type="text"
			value={settings.series_template != null ? settings.series_template : globalConfig.templates.series}
			disabled={settings.series_template == null}
			oninput={(e) => { settings.series_template = (e.currentTarget as HTMLInputElement).value; }}
			class="w-full rounded-md border border-input bg-background px-3 py-1.5 text-sm font-mono text-xs {settings.series_template == null ? 'opacity-50 cursor-not-allowed' : ''}"
		/>
	</div>

	<!-- Subtitles -->
	<h4 class="text-xs font-medium text-foreground/70 uppercase tracking-wider mt-5 mb-2">Subtitles</h4>

	<!-- Subtitles Enabled -->
	<div class="space-y-1.5">
		<div class="flex items-center justify-between">
			<span class="text-xs font-medium text-muted-foreground">Subtitles Enabled</span>
			<div class="flex items-center gap-2">
				<span class="text-[10px] text-muted-foreground/60">Override</span>
				<Switch
					checked={settings.subtitles_enabled != null}
					onCheckedChange={(checked: boolean) => {
						if (checked) {
							settings.subtitles_enabled = globalConfig.subtitles.enabled;
						} else {
							settings.subtitles_enabled = undefined;
						}
					}}
					size="sm"
				/>
			</div>
		</div>
		<div class="flex items-center gap-2 {settings.subtitles_enabled == null ? 'opacity-50 pointer-events-none' : ''}">
			<Switch
				checked={settings.subtitles_enabled != null ? settings.subtitles_enabled : globalConfig.subtitles.enabled}
				onCheckedChange={(checked: boolean) => { settings.subtitles_enabled = checked; }}
				disabled={settings.subtitles_enabled == null}
				size="sm"
			/>
			<span class="text-sm text-muted-foreground">
				{(settings.subtitles_enabled != null ? settings.subtitles_enabled : globalConfig.subtitles.enabled) ? 'Enabled' : 'Disabled'}
			</span>
		</div>
	</div>

	<!-- Preferred Languages -->
	<div class="space-y-1.5 mt-3">
		<div class="flex items-center justify-between">
			<span class="text-xs font-medium text-muted-foreground">Preferred Languages</span>
			<div class="flex items-center gap-2">
				<span class="text-[10px] text-muted-foreground/60">Override</span>
				<Switch
					checked={settings.preferred_languages != null}
					onCheckedChange={(checked: boolean) => {
						if (checked) {
							settings.preferred_languages = [...globalConfig.subtitles.preferred_languages];
						} else {
							settings.preferred_languages = undefined;
						}
					}}
					size="sm"
				/>
			</div>
		</div>
		<input
			type="text"
			value={settings.preferred_languages != null ? languagesDisplay(settings.preferred_languages) : languagesDisplay(globalConfig.subtitles.preferred_languages)}
			disabled={settings.preferred_languages == null}
			oninput={(e) => { settings.preferred_languages = parseLanguages((e.currentTarget as HTMLInputElement).value); }}
			placeholder="en, ja, de"
			class="w-full rounded-md border border-input bg-background px-3 py-1.5 text-sm {settings.preferred_languages == null ? 'opacity-50 cursor-not-allowed' : ''}"
		/>
		{#if settings.preferred_languages != null}
			<p class="text-[10px] text-muted-foreground/60">Comma-separated ISO 639 codes (e.g. en, ja, de)</p>
		{/if}
	</div>

	<!-- Non-Preferred Action -->
	<div class="space-y-1.5 mt-3">
		<div class="flex items-center justify-between">
			<span class="text-xs font-medium text-muted-foreground">Non-Preferred Action</span>
			<div class="flex items-center gap-2">
				<span class="text-[10px] text-muted-foreground/60">Override</span>
				<Switch
					checked={settings.non_preferred_action != null}
					onCheckedChange={(checked: boolean) => {
						if (checked) {
							settings.non_preferred_action = globalConfig.subtitles.non_preferred_action;
						} else {
							settings.non_preferred_action = undefined;
						}
					}}
					size="sm"
				/>
			</div>
		</div>
		<div class="flex items-center gap-4 flex-wrap {settings.non_preferred_action == null ? 'opacity-50 pointer-events-none' : ''}">
			{#each [['Ignore', 'Ignore'], ['Backup', 'Backup'], ['KeepAll', 'Keep All'], ['Review', 'Review']] as [value, label]}
				<label class="flex items-center gap-2 text-sm cursor-pointer">
					<input
						type="radio"
						name="watcher-settings-non-preferred"
						value={value}
						checked={(settings.non_preferred_action != null ? settings.non_preferred_action : globalConfig.subtitles.non_preferred_action) === value}
						disabled={settings.non_preferred_action == null}
						onchange={() => { settings.non_preferred_action = value as NonPreferredAction; }}
						class="accent-primary"
					/>
					{label}
				</label>
			{/each}
		</div>
	</div>
</div>
