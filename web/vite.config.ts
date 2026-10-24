// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import { mdsvex } from 'mdsvex';
import { defineConfig, loadEnv } from 'vite';

// SvelteKit 3 configures itself through this file; `svelte.config.js` is no
// longer supported. Prerendering is handled by the adapter/`prerender` options
// below — see docs/specs/web-major-upgrades.md.
export default defineConfig(({ mode }) => {
	const env = loadEnv(mode, process.cwd(), '');
	return {
		plugins: [
			sveltekit({
				extensions: ['.svelte', '.md'],
				preprocess: [vitePreprocess(), mdsvex({ extension: '.md' })],

				// Cloudflare Pages expects static files in the build directory
				adapter: adapter({
					pages: 'build',
					assets: 'build',
					fallback: 'index.html', // SPA fallback
					precompress: true
				})
				// This is a client-rendered SPA: pages do not export
				// `prerender = true`, so the build emits only the `index.html`
				// fallback (Cloudflare Pages serves it for deep links). A previous
				// `prerender.entries` list here was dead config. Enabling real
				// prerendering changes the deploy output and needs a Pages/CSP
				// check — see docs/specs/web-major-upgrades.md.
				//
				// CSP is handled by the Cloudflare Pages middleware
				// (functions/_middleware.ts), which injects nonces per-request to
				// support Cloudflare's injected scripts.
			})
		],
		// Vite 8 externalizes node_modules imports during the SSR build, which
		// made its built-in postcss-import try to read `tailwindcss` as a file
		// and fail with ENOENT. CSS packages must be resolved, not externalized.
		ssr: {
			noExternal: ['tailwindcss']
		},
		define: {
			'import.meta.env.PUBLIC_API_URL': JSON.stringify(env.PUBLIC_API_URL),
			'import.meta.env.PUBLIC_BASE_URL': JSON.stringify(env.PUBLIC_BASE_URL),
			'import.meta.env.PUBLIC_DEMO_MODE': JSON.stringify(env.PUBLIC_DEMO_MODE),
			'import.meta.env.PUBLIC_BUILD_ID': JSON.stringify(
				process.env.CF_PAGES_COMMIT_SHA || env.PUBLIC_BUILD_ID || 'dev'
			)
		}
	};
});
