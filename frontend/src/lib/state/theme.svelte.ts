class ThemeState {
	mode = $state<'dark' | 'light'>('dark');

	toggle() {
		this.mode = this.mode === 'dark' ? 'light' : 'dark';
		this.apply();
		this.persist();
	}

	init() {
		if (typeof window !== 'undefined') {
			const stored = localStorage.getItem('mediarr-theme');
			if (stored === 'light' || stored === 'dark') {
				this.mode = stored;
			}
		}
		this.apply();
	}

	private apply() {
		if (typeof document !== 'undefined') {
			document.documentElement.classList.toggle('dark', this.mode === 'dark');
			document.documentElement.classList.toggle('light', this.mode === 'light');
		}
	}

	private persist() {
		if (typeof window !== 'undefined') {
			localStorage.setItem('mediarr-theme', this.mode);
		}
	}
}

export const themeState = new ThemeState();
