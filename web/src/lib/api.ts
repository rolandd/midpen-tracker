// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

import { DEMO_MODE, mockUser, mockPreserveStats, getMockActivitiesForPreserve } from './mockData';

// API configuration
export const API_BASE_URL = import.meta.env.PUBLIC_API_URL || 'http://localhost:8080';

// Auth state helpers
// Deprecated: We now use HttpOnly cookies, so we can't synchronously check login status
// without an API call. Components should try to fetch user or handle 401s.
export async function checkAuth(): Promise<boolean> {
	if (DEMO_MODE) return true;
	try {
		await fetchMe();
		return true;
	} catch {
		return false;
	}
}

// API fetch wrapper with auth
export async function apiFetch<T>(path: string, options: RequestInit = {}): Promise<T> {
	const headers: HeadersInit = {
		'Content-Type': 'application/json',
		...options.headers
	};

	// We no longer manually inject Authorization header.
	// The browser sends the HttpOnly cookie automatically.

	const response = await fetch(`${API_BASE_URL}${path}`, {
		...options,
		headers,
		credentials: 'include' // Important: sends cookies with request
	});

	if (!response.ok) {
		if (response.status === 401) {
			// Session expired or invalid
			if (typeof window !== 'undefined' && !window.location.pathname.startsWith('/')) {
				// Optionally handle logout redirect here or let caller handle it
			}
			throw new Error('Session expired');
		}
		throw new Error(`API error: ${response.status}`);
	}

	return response.json();
}

// Types - re-exported from generated bindings

export type {
	UserResponse,
	PreserveActivity,
	PreserveSummary,
	PreserveStatsResponse,
	ActivitySummary,
	ActivitiesResponse,
	DeleteAccountResponse,
	HealthResponse
} from './generated';

import type {
	UserResponse,
	PreserveStatsResponse,
	ActivitiesResponse,
	DeleteAccountResponse,
	HealthResponse
} from './generated';

// API methods

export async function fetchMe(): Promise<UserResponse> {
	if (DEMO_MODE) return mockUser;

	return apiFetch<UserResponse>('/api/me');
}

export async function fetchPreserveStats(showUnvisited = false): Promise<PreserveStatsResponse> {
	if (DEMO_MODE) {
		const stats = { ...mockPreserveStats };

		if (!showUnvisited) {
			stats.preserves = stats.preserves.filter((p) => p.count > 0);
		}

		return stats;
	}

	return apiFetch<PreserveStatsResponse>(`/api/stats/preserves?show_unvisited=${showUnvisited}`);
}

export async function logout(): Promise<void> {
	await apiFetch('/auth/logout', { method: 'POST' });
}

export async function fetchActivities(
	preserve: string,

	page = 1
): Promise<ActivitiesResponse> {
	if (DEMO_MODE) return getMockActivitiesForPreserve(preserve, page);

	return apiFetch<ActivitiesResponse>(
		`/api/activities?preserve=${encodeURIComponent(preserve)}&page=${page}`
	);
}

export async function deleteAccount(): Promise<DeleteAccountResponse> {
	return apiFetch<DeleteAccountResponse>('/api/account', { method: 'DELETE' });
}

export async function fetchHealth(): Promise<HealthResponse> {
	if (DEMO_MODE) return { status: 'ok', build_id: 'demo' };

	return apiFetch<HealthResponse>('/health');
}
