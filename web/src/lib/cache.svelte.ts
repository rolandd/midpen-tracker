// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@kernel.org>

import type { ActivitySummary } from './generated';

type PreserveCache = {
	activities: ActivitySummary[];
	total: number;
	page: number;
	timestamp: number;
};

class ActivityCache {
	// Keyed by preserve name
	private _data = $state<Record<string, PreserveCache>>({});

	get(preserveName: string): PreserveCache | undefined {
		return this._data[preserveName];
	}

	set(preserveName: string, activities: ActivitySummary[], total: number, page: number) {
		this._data[preserveName] = {
			activities,
			total,
			page,
			timestamp: Date.now()
		};
	}

	append(preserveName: string, newActivities: ActivitySummary[], total: number, page: number) {
		const current = this._data[preserveName];
		if (!current) {
			this.set(preserveName, newActivities, total, page);
			return;
		}

		this._data[preserveName] = {
			activities: [...current.activities, ...newActivities],
			total,
			page,
			timestamp: Date.now()
		};
	}

	clear() {
		this._data = {};
	}
}

export const activityCache = new ActivityCache();
