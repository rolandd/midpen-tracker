// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

import { defineConfig, devices } from '@playwright/test';

const PORT = 4173;

/**
 * Browser smoke tests against the production build.
 *
 * `pnpm test:e2e` builds with PUBLIC_DEMO_MODE=true (so the dashboard renders
 * fixture data without a backend) and serves `build/` with a Pages-like
 * fallback. See docs/specs/web-major-upgrades.md.
 */
export default defineConfig({
	testDir: './e2e',
	outputDir: './test-results',
	fullyParallel: true,
	forbidOnly: Boolean(process.env.CI),
	retries: process.env.CI ? 2 : 0,
	workers: process.env.CI ? 2 : undefined,
	// The HTML report is what CI uploads on failure; `open: 'never'` keeps it
	// from launching a browser in CI.
	reporter: process.env.CI ? [['list'], ['github'], ['html', { open: 'never' }]] : [['list']],
	use: {
		baseURL: `http://127.0.0.1:${PORT}`,
		trace: 'on-first-retry'
	},
	projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
	webServer: {
		command: 'node scripts/serve-build.mjs',
		url: `http://127.0.0.1:${PORT}`,
		reuseExistingServer: !process.env.CI,
		timeout: 30_000
	}
});
