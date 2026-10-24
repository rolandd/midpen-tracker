// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

/**
 * Shared fast-check arbitraries for property-based unit tests.
 *
 * Named so it is obvious that this module is test-only: it is never imported
 * by application code, so it does not end up in the bundle.
 */

import * as fc from 'fast-check';
import type { ActivitySummary, PreserveSummary } from '../generated';

/** A date string that `Date.parse` accepts (arbitrary but valid ISO instants). */
export const isoTimestampArb = fc
	.date({ min: new Date('2000-01-01T00:00:00Z'), max: new Date('2030-12-31T23:59:59Z') })
	.map((d) => d.toISOString());

export const activityArb: fc.Arbitrary<ActivitySummary> = fc.record({
	id: fc.integer({ min: 1, max: 2 ** 31 - 1 }),
	name: fc.string(),
	sport_type: fc.constantFrom('Run', 'Ride', 'Hike', 'Walk', 'TrailRun', 'Swim', ''),
	start_date: isoTimestampArb,
	preserves: fc.array(fc.string(), { maxLength: 3 })
});

/**
 * Preserves with unique names. Real responses never repeat a preserve, and the
 * dashboard relies on that (list keys, year unions).
 */
export const uniquePreservesArb: fc.Arbitrary<PreserveSummary[]> = fc.uniqueArray(
	fc.record({
		name: fc.string({ minLength: 1, maxLength: 24 }),
		count: fc.integer({ min: 0, max: 500 }),
		activities: fc.constant([])
	}),
	{ selector: (preserve) => preserve.name, maxLength: 8 }
);
