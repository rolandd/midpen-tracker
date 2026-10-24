// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

/**
 * Pure helpers for turning preserve statistics into the list the dashboard
 * renders. Extracted from `routes/dashboard/+page.svelte` so the filtering and
 * ordering rules can be tested directly (see `preserves.test.ts`).
 */

import type { PreserveStatsResponse, PreserveSummary } from './generated';

export type PreservesByYear = PreserveStatsResponse['preserves_by_year'];

export type PreserveSelection = {
	/** All-time summaries, as returned by `GET /api/stats/preserves`. */
	allTime: readonly PreserveSummary[];
	/** Per-year visit counts, keyed by year then preserve name. */
	byYear: PreservesByYear;
	/** Year to show, or `null` for "All Time". */
	year: string | null;
	/** Whether preserves with zero visits are included. */
	showUnvisited: boolean;
};

/**
 * Dashboard ordering: most visits first, then by name so that ties have a
 * deterministic order independent of the API's ordering.
 */
export function comparePreserves(a: PreserveSummary, b: PreserveSummary): number {
	return b.count - a.count || a.name.localeCompare(b.name);
}

/**
 * Select the preserves to display for the current year filter.
 *
 * Guarantees:
 * - names are unique, so `{#each ... (name)}` is safe;
 * - the result is sorted with {@link comparePreserves};
 * - with `showUnvisited: false` every entry has `count > 0`;
 * - with `showUnvisited: true` the result is exactly the union of the
 *   preserves visited in the selected year and the known preserve names
 *   (unvisited ones carry `count: 0`);
 * - visit counts are conserved: the sum over the result equals the sum over
 *   the selected year's record (or the all-time counts).
 */
export function selectPreserves({
	allTime,
	byYear,
	year,
	showUnvisited
}: PreserveSelection): PreserveSummary[] {
	// Keyed by name so duplicates in the inputs can never reach the UI.
	const byName = new Map<string, PreserveSummary>();

	if (year === null) {
		for (const preserve of allTime) {
			byName.set(preserve.name, preserve);
		}
	} else {
		for (const [name, count] of Object.entries(byYear[year] ?? {})) {
			byName.set(name, { name, count, activities: [] });
		}

		if (showUnvisited) {
			// Known preserves with no visits in this year are rendered as zeroes.
			for (const { name } of allTime) {
				if (!byName.has(name)) {
					byName.set(name, { name, count: 0, activities: [] });
				}
			}
		}
	}

	const preserves = [...byName.values()];
	return (showUnvisited ? preserves : preserves.filter((p) => p.count > 0)).sort(comparePreserves);
}

/** Number of preserves with at least one visit in `preserves`. */
export function countVisited(preserves: readonly PreserveSummary[]): number {
	return preserves.filter((p) => p.count > 0).length;
}
