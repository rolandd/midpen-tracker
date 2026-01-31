// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import { mdsvex } from 'mdsvex';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	extensions: ['.svelte', '.md'],
	preprocess: [vitePreprocess(), mdsvex({ extension: '.md' })],

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
			entries: ['/', '/legal', '/dashboard', '/callback', '/account-deletion-in-progress']
		},
		// Content Security Policy
		// SvelteKit automatically adds hashes for inline scripts
		csp: {
			mode: 'hash',
			directives: {
				'default-src': ['self'],
				'script-src': ['self', 'sha256-fjILgSuxVwjCY4WpZhCz6v0RcpWZpe/xfr8tVgR+EDo='],
				'style-src': ['self', 'unsafe-inline', 'https://fonts.googleapis.com'],
				'font-src': ['self', 'https://fonts.gstatic.com'],
				'img-src': ['self', 'data:', 'https:'],
				'connect-src': [
					'self',
					'https://*.run.app',
					'http://localhost:8080',
					...(process.env.PUBLIC_API_URL ? [process.env.PUBLIC_API_URL] : [])
				],
				'frame-ancestors': ['none']
			}
		}
	}
};

export default config;
