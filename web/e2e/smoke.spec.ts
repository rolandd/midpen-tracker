// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

import { expect, test, type Page } from '@playwright/test';

const currentYear = new Date().getFullYear();

/**
 * Collects errors that must never reach the browser: uncaught exceptions,
 * failed hydration, or Svelte runtime warnings. Network noise (the demo avatar
 * is loaded from ui-avatars.com) is filtered out so the suite works offline.
 */
function collectRuntimeErrors(page: Page): string[] {
	const errors: string[] = [];

	page.on('pageerror', (error) => {
		errors.push(`uncaught: ${error.message}`);
	});

	page.on('console', (message) => {
		if (message.type() !== 'error' && message.type() !== 'warning') return;
		const text = message.text();
		if (/Failed to load resource|ui-avatars\.com|net::ERR_/.test(text)) return;
		if (/hydration|hydrating/i.test(text)) errors.push(`hydration: ${text}`);
	});

	return errors;
}

test.describe('smoke', () => {
	test('logged-in demo users are redirected from the landing page', async ({ page }) => {
		const errors = collectRuntimeErrors(page);

		await page.goto('/');

		await expect(page).toHaveURL(/\/dashboard$/);
		await expect(page.getByRole('heading', { name: /Midpen Tracker/ })).toBeVisible();
		expect(errors).toEqual([]);
	});

	test('dashboard renders demo statistics and preserve list', async ({ page }) => {
		const errors = collectRuntimeErrors(page);

		await page.goto('/dashboard');

		const progress = page.getByRole('progressbar');
		await expect(progress).toBeVisible();
		await expect(page.getByText('Preserves Visited')).toBeVisible();
		await expect(page.getByText(`/ ${24}`)).toBeVisible();

		// Visited preserves are shown by default.
		await expect(page.getByRole('button', { name: /Rancho San Antonio/ })).toBeVisible();

		// Year filter renders "All Time" plus the mock years.
		await expect(page.getByRole('button', { name: 'All Time' })).toHaveAttribute(
			'aria-pressed',
			'true'
		);
		const yearPill = page.getByRole('button', { name: String(currentYear), exact: true });
		await yearPill.click();
		await expect(yearPill).toHaveAttribute('aria-pressed', 'true');

		// Unvisited preserves appear once toggled on.
		await page.getByRole('button', { name: /Show unvisited preserves/ }).click();
		await expect(page.getByText("Devil's Slide")).toBeVisible();

		expect(errors).toEqual([]);
	});

	test('expanding a preserve loads its activities', async ({ page }) => {
		const errors = collectRuntimeErrors(page);

		await page.goto('/dashboard');
		const preserve = page.getByRole('button', { name: /Rancho San Antonio/ });
		await preserve.click();

		await expect(preserve).toHaveAttribute('aria-expanded', 'true');
		await expect(page.locator('a.activity').first()).toBeVisible();
		await expect(page.locator('a.activity').first()).toHaveAttribute(
			'href',
			/^https:\/\/www\.strava\.com\/activities\/\d+$/
		);

		expect(errors).toEqual([]);
	});

	test('legal page renders the mdsvex document', async ({ page }) => {
		const errors = collectRuntimeErrors(page);

		await page.goto('/legal');

		await expect(
			page.getByRole('heading', { name: 'Privacy Policy & Terms of Service', level: 1 })
		).toBeVisible();
		await expect(page.getByText('We take your privacy seriously')).toBeVisible();
		// Markdown lists must survive the mdsvex pipeline.
		await expect(page.getByRole('heading', { name: 'Privacy Policy', level: 2 })).toBeVisible();

		expect(errors).toEqual([]);
	});

	test('account deletion page redirects when no deletion is pending', async ({ page }) => {
		await page.goto('/account-deletion-in-progress');

		// Demo user has deletion_requested_at = null, so the polling page bounces
		// back to the dashboard (exercises the $app/navigation + $app/state glue).
		await expect(page).toHaveURL(/\/dashboard$/);
	});
});

test.describe('static build contract', () => {
	test('deep links are served the SPA shell', async ({ request }) => {
		for (const pathname of ['/dashboard', '/legal', '/account-deletion-in-progress']) {
			const response = await request.get(pathname);
			expect(response.status(), pathname).toBe(200);
			expect(await response.text(), pathname).toContain('__sveltekit_');
		}
	});

	test('missing hashed assets 404 instead of returning HTML', async ({ request }) => {
		const response = await request.get('/_app/immutable/entry/does-not-exist.js');
		expect(response.status()).toBe(404);
	});
});
