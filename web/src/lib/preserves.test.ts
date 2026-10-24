// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

import * as fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import type { PreserveStatsResponse, PreserveSummary } from './generated';
import { comparePreserves, countVisited, selectPreserves } from './preserves';
import { uniquePreservesArb } from './testing/arbitraries';

type ByYear = PreserveStatsResponse['preserves_by_year'];

function preserve(name: string, count: number): PreserveSummary {
	return { name, count, activities: [] };
}

function sumCounts(preserves: readonly PreserveSummary[]): number {
	return preserves.reduce((total, p) => total + p.count, 0);
}

function names(preserves: readonly PreserveSummary[]): Set<string> {
	return new Set(preserves.map((p) => p.name));
}

/** Visit counts for one year, restricted to names that exist in `allTime`. */
function yearCountsArb(allTime: readonly PreserveSummary[]): fc.Arbitrary<Record<string, number>> {
	const known = allTime.map((p) => p.name);
	return fc.dictionary(
		fc.constantFrom(...(known.length > 0 ? known : [''])),
		fc.integer({ min: 1, max: 200 })
	);
}

const ALL_TIME_CASE = fc
	.record({
		allTime: uniquePreservesArb,
		showUnvisited: fc.boolean()
	})
	.map(({ allTime, showUnvisited }) => ({ allTime, showUnvisited }));

type YearCase = {
	allTime: PreserveSummary[];
	showUnvisited: boolean;
	byYear: ByYear;
	year: string;
};

const YEAR_CASE: fc.Arbitrary<YearCase> = fc
	.record({ allTime: uniquePreservesArb, showUnvisited: fc.boolean() })
	.chain((base) =>
		yearCountsArb(base.allTime).map((yearCounts): YearCase => ({
			...base,
			byYear: { '2025': yearCounts },
			year: '2025'
		}))
	);

describe('comparePreserves', () => {
	it('orders by visits descending, then by name', () => {
		const preserves = [preserve('b', 1), preserve('a', 5), preserve('c', 1)];

		expect([...preserves].sort(comparePreserves).map((p) => p.name)).toEqual(['a', 'b', 'c']);
	});

	it('is a total order (antisymmetric for distinct preserves)', () => {
		fc.assert(
			fc.property(uniquePreservesArb, (preserves) => {
				for (const a of preserves) {
					for (const b of preserves) {
						if (a.name === b.name) continue;
						const forward = comparePreserves(a, b);
						const backward = comparePreserves(b, a);
						if (forward === 0 || backward === 0) return false;
						if (Math.sign(forward) !== -Math.sign(backward)) return false;
					}
				}
				return true;
			})
		);
	});
});

describe('selectPreserves (property-based)', () => {
	it('all-time: unique names, sorted, and preserves the visit counts', () => {
		fc.assert(
			fc.property(ALL_TIME_CASE, ({ allTime, showUnvisited }) => {
				const result = selectPreserves({ allTime, byYear: {}, year: null, showUnvisited });

				// No duplicate keys for the `{#each}` block.
				expect(new Set(result.map((p) => p.name)).size).toBe(result.length);
				// Sorted with the dashboard comparator.
				expect([...result].sort(comparePreserves)).toEqual(result);

				// Unvisited preserves are only rendered when asked for.
				for (const p of result) {
					if (!showUnvisited && p.count <= 0) throw new Error(`hidden ${p.count}`);
					expect(names(allTime).has(p.name)).toBe(true);
				}
				if (showUnvisited) {
					expect(names(result)).toEqual(names(allTime));
				}

				// Counts are never invented or dropped.
				expect(sumCounts(result)).toBe(sumCounts(allTime));
				expect(countVisited(result)).toBe(allTime.filter((p) => p.count > 0).length);
			})
		);
	});

	it('year: reports the union with all-time names, zeroing unvisited ones', () => {
		fc.assert(
			fc.property(YEAR_CASE, ({ allTime, byYear, year, showUnvisited }) => {
				const yearCounts = byYear[year];

				const result = selectPreserves({ allTime, byYear, year, showUnvisited });
				const allTimeNames = names(allTime);

				expect(new Set(result.map((p) => p.name)).size).toBe(result.length);
				expect([...result].sort(comparePreserves)).toEqual(result);

				// Never shows a preserve that exists nowhere.
				for (const name of names(result)) {
					if (!allTimeNames.has(name) && !(name in yearCounts)) {
						throw new Error(`unknown preserve ${name}`);
					}
				}

				// Counts come from the year record, never from all-time.
				for (const p of result) {
					expect(p.count).toBe(Object.hasOwn(yearCounts, p.name) ? yearCounts[p.name] : 0);
				}

				if (showUnvisited) {
					// Union of the year's visits and every known preserve.
					expect(names(result)).toEqual(new Set([...Object.keys(yearCounts), ...allTimeNames]));
				} else {
					// Only preserves visited that year, i.e. no zero rows slipped through.
					for (const p of result) expect(p.count).toBeGreaterThan(0);
					expect(names(result)).toEqual(
						new Set(Object.keys(yearCounts).filter((name) => yearCounts[name] > 0))
					);
				}

				// Visit counts are conserved for the selected year.
				expect(sumCounts(result)).toBe(
					sumCounts(Object.entries(yearCounts).map(([n, c]) => preserve(n, c)))
				);
			})
		);
	});

	it('an unknown year shows nothing, or only unvisited preserves', () => {
		fc.assert(
			fc.property(uniquePreservesArb, fc.boolean(), (allTime, showUnvisited) => {
				const result = selectPreserves({
					allTime,
					byYear: { '1900': { Nowhere: 3 } },
					year: '2025',
					showUnvisited
				});

				if (!showUnvisited) expect(result).toEqual([]);
				else expect(result.every((p) => p.count === 0)).toBe(true);
			})
		);
	});

	it('does not mutate its inputs', () => {
		fc.assert(
			fc.property(ALL_TIME_CASE, ({ allTime, showUnvisited }) => {
				const snapshot = JSON.stringify({ allTime, showUnvisited });

				selectPreserves({ allTime, byYear: {}, year: null, showUnvisited });

				expect(JSON.stringify({ allTime, showUnvisited })).toBe(snapshot);
			})
		);
	});
});

describe('selectPreserves (examples)', () => {
	const allTime = [
		preserve('Rancho San Antonio', 12),
		preserve('Windy Hill', 0),
		preserve('Teague Hill', 4)
	];

	it('all time hides unvisited preserves by default', () => {
		expect(selectPreserves({ allTime, byYear: {}, year: null, showUnvisited: false })).toEqual([
			preserve('Rancho San Antonio', 12),
			preserve('Teague Hill', 4)
		]);
	});

	it('all time includes unvisited preserves when toggled', () => {
		expect(
			selectPreserves({ allTime, byYear: {}, year: null, showUnvisited: true }).map((p) => p.name)
		).toEqual(['Rancho San Antonio', 'Teague Hill', 'Windy Hill']);
	});

	it('year mode unions the year record with known preserves', () => {
		const byYear = { '2025': { 'Teague Hill': 4 } };

		expect(
			selectPreserves({ allTime, byYear, year: '2025', showUnvisited: true }).map((p) => [
				p.name,
				p.count
			])
		).toEqual([
			['Teague Hill', 4],
			['Rancho San Antonio', 0],
			['Windy Hill', 0]
		]);
	});

	it('year mode hides zero rows without the toggle', () => {
		// A backend that emits explicit zeroes must not leak empty rows either.
		const byYear = { '2025': { 'Teague Hill': 4, 'Windy Hill': 0 } };

		expect(
			selectPreserves({ allTime, byYear, year: '2025', showUnvisited: false }).map((p) => p.name)
		).toEqual(['Teague Hill']);
	});
});
