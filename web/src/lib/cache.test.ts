// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

import { beforeEach, describe, expect, it } from 'vitest';
import { activityCache } from './cache.svelte';
import type { ActivitySummary } from './generated';

function activity(id: number): ActivitySummary {
	return {
		id,
		name: `Activity ${id}`,
		sport_type: 'Run',
		start_date: '2026-01-01T00:00:00Z',
		preserves: ['Rancho San Antonio']
	};
}

beforeEach(() => {
	activityCache.clear();
});

describe('activityCache', () => {
	it('returns undefined for unknown preserves', () => {
		expect(activityCache.get('Nowhere')).toBeUndefined();
	});

	it('stores activities with pagination metadata and a timestamp', () => {
		activityCache.set('Rancho San Antonio', [activity(1), activity(2)], 40, 1);

		const cached = activityCache.get('Rancho San Antonio');
		expect(cached).toBeDefined();
		expect(cached!.activities.map((a) => a.id)).toEqual([1, 2]);
		expect(cached!.total).toBe(40);
		expect(cached!.page).toBe(1);
		expect(cached!.timestamp).toBeGreaterThan(0);
	});

	it('append() on a missing preserve behaves like set()', () => {
		activityCache.append('Windy Hill', [activity(3)], 1, 1);

		expect(activityCache.get('Windy Hill')).toMatchObject({ total: 1, page: 1 });
	});

	it('append() concatenates activities and updates page/total', () => {
		activityCache.set('Windy Hill', [activity(1)], 3, 1);
		activityCache.append('Windy Hill', [activity(2), activity(3)], 3, 2);

		const cached = activityCache.get('Windy Hill')!;
		expect(cached.activities.map((a) => a.id)).toEqual([1, 2, 3]);
		expect(cached.page).toBe(2);
		expect(cached.total).toBe(3);
	});

	it('keeps preserves independent', () => {
		activityCache.set('A', [activity(1)], 1, 1);
		activityCache.set('B', [activity(2)], 1, 1);

		activityCache.append('A', [activity(3)], 2, 2);

		expect(activityCache.get('A')!.activities).toHaveLength(2);
		expect(activityCache.get('B')!.activities).toHaveLength(1);
	});

	it('clear() drops everything', () => {
		activityCache.set('A', [activity(1)], 1, 1);
		activityCache.clear();

		expect(activityCache.get('A')).toBeUndefined();
	});
});
