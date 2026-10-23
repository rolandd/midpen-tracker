// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

import { describe, expect, it } from 'vitest';
import { DEMO_MODE, getMockActivitiesForPreserve, mockPreserveStats, mockUser } from './mockData';

const currentYear = new Date().getFullYear();

describe('mockData (demo fixtures)', () => {
	it('is disabled outside demo builds', () => {
		expect(DEMO_MODE).toBe(false);
	});

	it('provides a complete mock user', () => {
		expect(mockUser).toMatchObject({
			athlete_id: expect.any(Number),
			firstname: expect.any(String),
			lastname: expect.any(String),
			deletion_requested_at: null
		});
	});

	it('keeps preserve counts internally consistent', () => {
		const stats = mockPreserveStats;

		expect(stats.preserves).toHaveLength(stats.total_preserves);
		expect(stats.total_preserves_visited).toBe(stats.preserves.filter((p) => p.count > 0).length);
		expect(stats.pending_activities).toBe(0);
		expect(stats.preserves.every((p) => Number.isInteger(p.count) && p.count >= 0)).toBe(true);

		// Names must be unique, otherwise `{#each ... (preserve.name)}` breaks.
		const names = stats.preserves.map((p) => p.name);
		expect(new Set(names).size).toBe(names.length);
	});

	it('exposes the three most recent years, descending', () => {
		expect(mockPreserveStats.available_years).toEqual([
			String(currentYear),
			String(currentYear - 1),
			String(currentYear - 2)
		]);
	});

	it('only reports year buckets that exist and are known preserves', () => {
		const knownNames = new Set(mockPreserveStats.preserves.map((p) => p.name));

		for (const [year, byName] of Object.entries(mockPreserveStats.preserves_by_year)) {
			expect(mockPreserveStats.available_years).toContain(year);
			for (const [name, count] of Object.entries(byName)) {
				expect(knownNames.has(name)).toBe(true);
				expect(Number.isInteger(count) && count > 0).toBe(true);
			}
		}
	});
});

describe('getMockActivitiesForPreserve', () => {
	it('returns an empty page for an unknown preserve', () => {
		const response = getMockActivitiesForPreserve('Not A Preserve');

		expect(response).toMatchObject({ activities: [], total: 0, page: 1, per_page: 50 });
		expect(response.next_cursor).toBeNull();
	});

	it('maps mock activities to the API shape, newest first', () => {
		const preserve = mockPreserveStats.preserves.find((p) => p.activities.length > 1)!;
		const response = getMockActivitiesForPreserve(preserve.name);

		expect(response.activities.length).toBeGreaterThan(1);
		expect(response.total).toBe(preserve.activities.length);

		for (const activity of response.activities) {
			expect(activity.preserves).toEqual([preserve.name]);
			expect(Number.isNaN(Date.parse(activity.start_date))).toBe(false);
		}

		const times = response.activities.map((a) => Date.parse(a.start_date));
		expect([...times].sort((a, b) => b - a)).toEqual(times);
	});

	it('paginates with a fixed page size', () => {
		const preserve = mockPreserveStats.preserves.find((p) => p.activities.length > 50);
		expect(preserve).toBeDefined();

		const first = getMockActivitiesForPreserve(preserve!.name, 1);
		const second = getMockActivitiesForPreserve(preserve!.name, 2);

		expect(first.activities).toHaveLength(50);
		expect(second.activities.length).toBeGreaterThan(0);
		expect(second.page).toBe(2);
		expect(second.activities[0].id).not.toBe(first.activities[0].id);
		// Pages must not overlap.
		const firstIds = new Set(first.activities.map((a) => a.id));
		expect(second.activities.every((a) => !firstIds.has(a.id))).toBe(true);
	});
});
