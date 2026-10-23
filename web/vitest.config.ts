// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

import path from 'node:path';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vitest/config';

/**
 * Unit-test configuration.
 *
 * Deliberately independent of `vite.config.ts`: the SvelteKit plugin pulls in
 * the whole app/router graph, which unit tests do not need. The `svelte`
 * plugin is still required so that `.svelte.ts` modules (runes) compile.
 */
export default defineConfig({
	plugins: [svelte()],
	// SvelteKit uses PUBLIC_ for env vars; mirror that so `import.meta.env.PUBLIC_*`
	// is populated from `.env.test` and can be overridden with `vi.stubEnv`.
	envPrefix: 'PUBLIC_',
	resolve: {
		alias: {
			$lib: path.resolve(import.meta.dirname, 'src/lib')
		}
	},
	test: {
		include: ['src/**/*.test.ts'],
		environment: 'node',
		restoreMocks: true
	}
});
