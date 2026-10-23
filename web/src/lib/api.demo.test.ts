// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

import { afterAll, describe, expect, it, vi } from 'vitest';

/**
 * Demo-mode behaviour of the API layer.
 *
 * `DEMO_MODE` is evaluated once, at module load time, so the env var has to be
 * stubbed before `./api` is imported — hence the dynamic import below. This
 * file intentionally has no static import of `./api`.
 */
vi.stubEnv('PUBLIC_DEMO_MODE', 'true');

const api = await import('./api');
const mockData = await import('./mockData');

afterAll(() => {
	vi.unstubAllEnvs();
});

describe('demo mode', () => {
	it('is enabled for this module instance', async () => {
		expect(mockData.DEMO_MODE).toBe(true);
	});

	it('serves the mock user without a network call', async () => {
		const fetchSpy = vi.fn();
		vi.stubGlobal('fetch', fetchSpy);

		expect(await api.fetchMe()).toEqual(mockData.mockUser);
		expect(fetchSpy).not.toHaveBeenCalled();

		vi.unstubAllGlobals();
	});

	it('reports a synthetic health payload', async () => {
		expect(await api.fetchHealth()).toEqual({ status: 'ok', build_id: 'demo' });
	});

	it('filters unvisited preserves when showUnvisited=false', async () => {
		const withUnvisited = await api.fetchPreserveStats(true);
		const visitedOnly = await api.fetchPreserveStats(false);

		expect(withUnvisited.preserves.some((p) => p.count === 0)).toBe(true);
		expect(visitedOnly.preserves.every((p) => p.count > 0)).toBe(true);
		expect(visitedOnly.preserves.length).toBeLessThanOrEqual(withUnvisited.preserves.length);
		// The shared mock object must not be mutated by the filter.
		expect(withUnvisited.preserves.length).toBe(mockData.mockPreserveStats.preserves.length);
	});

	it('serves paginated mock activities', async () => {
		const preserve = mockData.mockPreserveStats.preserves.find((p) => p.count > 1);
		expect(preserve).toBeDefined();

		const response = await api.fetchActivities(preserve!.name, 1);

		expect(response.page).toBe(1);
		expect(response.per_page).toBe(50);
		expect(response.total).toBe(preserve!.activities.length);
		expect(response.activities.length).toBeLessThanOrEqual(50);
		expect(response.activities.length).toBeGreaterThan(0);
		expect(response.activities[0].preserves).toEqual([preserve!.name]);
		expect(response.activities.every((a) => !Number.isNaN(Date.parse(a.start_date)))).toBe(true);
	});
});
