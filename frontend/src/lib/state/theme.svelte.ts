class ThemeState {
	mode = $state<'dark' | 'light'>('dark');
	/** Whether the user has manually set a preference (overriding system). */
	private userOverride = false;

	toggle() {
		this.mode = this.mode === 'dark' ? 'light' : 'dark';
		this.userOverride = true;
		this.apply();
		this.persist();
	}

	init() {
		if (typeof window === 'undefined') return;

		const stored = localStorage.getItem('mediarr-theme');
		if (stored === 'light' || stored === 'dark') {
			this.mode = stored;
			this.userOverride = true;
		} else {
			// Respect system preference when no user override exists
			const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
			this.mode = prefersDark ? 'dark' : 'light';
		}

		// Listen for system theme changes (only applies when no user override)
		window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', (e) => {
			if (!this.userOverride) {
				this.mode = e.matches ? 'dark' : 'light';
				this.apply();
			}
		});

		this.apply();
	}

	private apply() {
		if (typeof document === 'undefined') return;
		document.documentElement.classList.toggle('dark', this.mode === 'dark');
		document.documentElement.classList.toggle('light', this.mode === 'light');
	}

	private persist() {
		if (typeof window === 'undefined') return;
		localStorage.setItem('mediarr-theme', this.mode);
	}
}

export const themeState = new ThemeState();
