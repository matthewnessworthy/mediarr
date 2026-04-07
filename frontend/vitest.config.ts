import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
	plugins: [svelte()],
	resolve: {
		conditions: ['browser'],
		alias: {
			'$lib': '/src/lib',
			'$app': '/src/app',
		},
	},
	test: {
		globals: true,
		environment: 'happy-dom',
		include: ['src/**/*.test.ts', 'src/**/*.svelte.test.ts'],
		setupFiles: ['src/test/setup.ts'],
	},
});
