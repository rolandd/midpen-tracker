import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess(),

	kit: {
		adapter: adapter({
			// Cloudflare Pages expects static files in build output
			pages: 'build',
			assets: 'build',
			fallback: 'index.html', // SPA fallback
			precompress: true
		}),
		// Prerender all pages as static HTML
		prerender: {
			entries: ['*']
		}
	}
};

export default config;
