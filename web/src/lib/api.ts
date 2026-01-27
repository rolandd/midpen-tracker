// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@kernel.org>

import { DEMO_MODE, mockUser, mockPreserveStats, getMockActivitiesForPreserve } from './mockData';

// API configuration
export const API_BASE_URL = import.meta.env.PUBLIC_API_URL || 'http://localhost:8080';

// Auth state helpers
export function getToken(): string | null {
	if (typeof window === 'undefined') return null;
	return localStorage.getItem('midpen_token');
}

export function setToken(token: string): void {
	localStorage.setItem('midpen_token', token);
}

export function clearToken(): void {
	localStorage.removeItem('midpen_token');
}

export function isLoggedIn(): boolean {
	if (DEMO_MODE) return true;
	return getToken() !== null;
}

// API fetch wrapper with auth
export async function apiFetch<T>(path: string, options: RequestInit = {}): Promise<T> {
	const token = getToken();

	const headers: HeadersInit = {
		'Content-Type': 'application/json',
		...options.headers
	};

	if (token) {
		(headers as Record<string, string>)['Authorization'] = `Bearer ${token}`;
	}

	const response = await fetch(`${API_BASE_URL}${path}`, {
		...options,
		headers
	});

	if (!response.ok) {
		if (response.status === 401) {
			clearToken();
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

	clearToken();
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
	const response = await apiFetch<DeleteAccountResponse>('/api/account', { method: 'DELETE' });

	clearToken();

	return response;
}

export async function fetchHealth(): Promise<HealthResponse> {
	if (DEMO_MODE) return { status: 'ok', build_id: 'demo' };

	return apiFetch<HealthResponse>('/health');
}
