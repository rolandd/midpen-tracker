// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

import * as fc from 'fast-check';
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

/**
 * Model-based test: `activityCache` is a small keyed store, so the strongest
 * check is that it agrees with a plain `Map` model for arbitrary operation
 * sequences — including reads, re-sets, appends and clears.
 */
type Op =
	| { kind: 'read'; preserve: string }
	| { kind: 'clear' }
	| {
			kind: 'write';
			preserve: string;
			ids: number[];
			total: number;
			page: number;
			append: boolean;
	  };

type Entry = { ids: number[]; total: number; page: number };

const preserveArb = fc.constantFrom('Rancho San Antonio', 'Windy Hill', 'Teague Hill');

const writeArb: fc.Arbitrary<Op> = fc.record({
	kind: fc.constant('write' as const),
	preserve: preserveArb,
	ids: fc.array(fc.integer({ min: 1, max: 40 }), { maxLength: 5 }),
	total: fc.integer({ min: 0, max: 100 }),
	page: fc.integer({ min: 1, max: 10 }),
	append: fc.boolean()
});

const opArb: fc.Arbitrary<Op> = fc.oneof(
	writeArb,
	fc.record({ kind: fc.constant('read' as const), preserve: preserveArb }),
	fc.record({ kind: fc.constant('clear' as const) })
);

function expectMatchesModel(model: Map<string, Entry>) {
	for (const [preserve, entry] of model) {
		const cached = activityCache.get(preserve);
		expect(cached, preserve).toBeDefined();
		expect(cached!.activities.map((a) => a.id)).toEqual(entry.ids);
		expect(cached!.total).toBe(entry.total);
		expect(cached!.page).toBe(entry.page);
		expect(cached!.timestamp).toBeGreaterThan(0);
	}
}

describe('activityCache (model-based)', () => {
	it('behaves like a Map of activity pages for arbitrary operations', () => {
		fc.assert(
			fc.property(fc.array(opArb, { maxLength: 20 }), (ops) => {
				activityCache.clear();
				const model = new Map<string, Entry>();

				for (const op of ops) {
					if (op.kind === 'clear') {
						activityCache.clear();
						model.clear();
						continue;
					}

					if (op.kind === 'read') {
						const cached = activityCache.get(op.preserve);
						const expected = model.get(op.preserve);
						if (expected === undefined) {
							expect(cached).toBeUndefined();
						} else {
							expect(cached?.activities.map((a) => a.id)).toEqual(expected.ids);
						}
						continue;
					}

					const previous = model.get(op.preserve);
					const entry: Entry = {
						ids: [...(op.append && previous ? previous.ids : []), ...op.ids],
						total: op.total,
						page: op.page
					};

					// Appending must not rewrite the array handed out earlier.
					const before = previous ? [...previous.ids] : undefined;

					if (op.append) {
						activityCache.append(op.preserve, op.ids.map(activity), op.total, op.page);
					} else {
						activityCache.set(op.preserve, op.ids.map(activity), op.total, op.page);
					}

					model.set(op.preserve, entry);
					if (before) expect(previous!.ids).toEqual(before);

					expectMatchesModel(model);
				}

				// Clearing really empties the store.
				activityCache.clear();
				expect(activityCache.get('Rancho San Antonio')).toBeUndefined();
			})
		);
	});
});
