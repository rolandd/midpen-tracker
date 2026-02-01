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
		}
		// Note: CSP is handled by the Cloudflare Pages middleware (functions/_middleware.ts)
		// which injects nonces per-request to support Cloudflare's injected scripts
	}
};

export default config;
