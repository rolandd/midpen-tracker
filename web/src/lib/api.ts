// API configuration
export const API_BASE_URL = import.meta.env.VITE_API_URL || 'http://localhost:8080';

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

// Types
export interface User {
	athlete_id: number;
	firstname: string;
	lastname: string;
	profile_picture: string | null;
}

export interface PreserveActivity {
	id: number;
	date: string;
	sport_type: string;
	name: string;
}

export interface PreserveSummary {
	name: string;
	count: number;
	activities: PreserveActivity[];
}

export interface PreserveStatsResponse {
	preserves: PreserveSummary[];
	preserves_by_year: Record<string, Record<string, number>>;
	total_preserves_visited: number;
	total_preserves: number;
	pending_activities: number;
	available_years: string[];
}

// API methods
export async function fetchMe(): Promise<User> {
	return apiFetch<User>('/api/me');
}

export async function fetchPreserveStats(showUnvisited = false): Promise<PreserveStatsResponse> {
	return apiFetch<PreserveStatsResponse>(`/api/stats/preserves?show_unvisited=${showUnvisited}`);
}

export async function logout(): Promise<void> {
	await apiFetch('/auth/logout', { method: 'POST' });
	clearToken();
}

export interface ActivitySummary {
	id: number;
	name: string;
	sport_type: string;
	start_date: string;
	preserves: string[];
}

export interface ActivitiesResponse {
	activities: ActivitySummary[];
	total: number;
	page: number;
	per_page: number;
}

export async function fetchActivities(preserve: string): Promise<ActivitiesResponse> {
	return apiFetch<ActivitiesResponse>(`/api/activities?preserve=${encodeURIComponent(preserve)}`);
}
