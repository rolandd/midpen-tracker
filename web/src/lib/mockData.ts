// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@kernel.org>

/**
 * Mock data for frontend development without backend authentication.
 * Enable by setting PUBLIC_DEMO_MODE=true in .env or environment.
 */

import type { UserResponse, PreserveStatsResponse, ActivitiesResponse } from './generated';

export const DEMO_MODE = import.meta.env.PUBLIC_DEMO_MODE === 'true';

export const mockUser: UserResponse = {
	athlete_id: 12345678,
	firstname: 'Demo',
	lastname: 'User',
	// Realistic placeholder avatar
	profile_picture: 'https://ui-avatars.com/api/?name=Demo+User&background=random'
};

// Sample preserves with realistic data
const preserveNames = [
	'Rancho San Antonio',
	'Windy Hill',
	'Russian Ridge',
	'Monte Bello',
	'Los Trancos',
	'Skyline Ridge',
	'Coal Creek',
	'El Corte de Madera Creek',
	'Purisima Creek Redwoods',
	'Thornewood',
	'Long Ridge',
	'Saratoga Gap',
	'Sierra Azul',
	"St. Joseph's Hill",
	'Fremont Older',
	'Picchetti Ranch',
	'Stevens Creek Shoreline Nature Study Area',
	'Ravenswood',
	'Bear Creek Redwoods',
	'Teague Hill',
	'La Honda Creek',
	'Tunitas Creek',
	'Miramontes Ridge',
	"Devil's Slide"
];

// Generate mock activities for a preserve
function generateMockActivities(
	preserveName: string,
	count: number
): { id: number; date: string; sport_type: string; name: string }[] {
	const sportTypes = ['Run', 'Ride', 'Hike', 'Walk', 'TrailRun'];
	const activities = [];

	for (let i = 0; i < count; i++) {
		const date = new Date();
		date.setDate(date.getDate() - Math.floor(Math.random() * 365 * 2)); // Random date within 2 years

		activities.push({
			id: 1000000000 + Math.floor(Math.random() * 999999999),
			date: date.toISOString(),
			sport_type: sportTypes[Math.floor(Math.random() * sportTypes.length)],
			name: `${preserveName} ${sportTypes[Math.floor(Math.random() * sportTypes.length)]}`
		});
	}

	return activities.sort((a, b) => new Date(b.date).getTime() - new Date(a.date).getTime());
}

// Generate preserves with varying visit counts
function generatePreserves() {
	return preserveNames.map((name, idx) => {
		// Give earlier preserves more visits for realistic distribution
		// Increase max visits to allow testing pagination (e.g., > 50)
		const maxVisits = Math.max(1, 120 - idx * 5);
		const count = idx < 15 ? Math.floor(Math.random() * maxVisits) + 1 : 0;
		// Keep up to 100 activities in mock data storage
		return {
			name,
			count,
			activities: generateMockActivities(name, Math.min(count, 100))
		};
	});
}

// Generate year-by-year data
function generatePreservesByYear(): Record<string, Record<string, number>> {
	const years: Record<string, Record<string, number>> = {};
	const currentYear = new Date().getFullYear();

	for (let year = currentYear; year >= currentYear - 2; year--) {
		years[year.toString()] = {};
		preserveNames.slice(0, 15).forEach((name, idx) => {
			const visits = Math.floor(Math.random() * (10 - idx / 2)) + (idx < 8 ? 1 : 0);
			if (visits > 0) {
				years[year.toString()][name] = visits;
			}
		});
	}

	return years;
}

const mockPreserves = generatePreserves();

export const mockPreserveStats: PreserveStatsResponse = {
	preserves: mockPreserves,
	preserves_by_year: generatePreservesByYear(),
	total_preserves_visited: mockPreserves.filter((p) => p.count > 0).length,
	total_preserves: preserveNames.length,
	pending_activities: 0,
	available_years: [
		new Date().getFullYear().toString(),
		(new Date().getFullYear() - 1).toString(),
		(new Date().getFullYear() - 2).toString()
	]
};

export function getMockActivitiesForPreserve(
	preserveName: string,
	page = 1,
	per_page = 50
): ActivitiesResponse {
	const preserve = mockPreserves.find((p) => p.name === preserveName);
	const allActivities = preserve
		? preserve.activities.map((a) => ({
				...a,
				start_date: a.date,
				preserves: [preserveName]
			}))
		: [];

	const start = (page - 1) * per_page;
	const end = start + per_page;
	const slicedActivities = allActivities.slice(start, end);

	return {
		activities: slicedActivities,
		total: allActivities.length,
		page,
		per_page
	};
}
