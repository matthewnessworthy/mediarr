class ThemeState {
	mode = $state<'dark' | 'light'>('dark');

	toggle() {
		this.mode = this.mode === 'dark' ? 'light' : 'dark';
		if (typeof document !== 'undefined') {
			document.documentElement.classList.toggle('dark', this.mode === 'dark');
			document.documentElement.classList.toggle('light', this.mode === 'light');
		}
	}

	init() {
		if (typeof document !== 'undefined') {
			document.documentElement.classList.add(this.mode);
		}
	}
}

export const themeState = new ThemeState();
