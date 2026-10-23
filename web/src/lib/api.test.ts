// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	API_BASE_URL,
	ApiError,
	apiFetch,
	deleteAccount,
	fetchActivities,
	fetchHealth,
	fetchMe,
	fetchPreserveStats,
	logout
} from './api';

function jsonResponse(body: unknown, init: ResponseInit = {}): Response {
	return new Response(JSON.stringify(body), {
		status: 200,
		headers: { 'Content-Type': 'application/json' },
		...init
	});
}

let fetchMock: ReturnType<typeof vi.fn>;

beforeEach(() => {
	fetchMock = vi.fn();
	vi.stubGlobal('fetch', fetchMock);
});

afterEach(() => {
	vi.unstubAllGlobals();
});

describe('API_BASE_URL', () => {
	it('comes from PUBLIC_API_URL (deterministic via .env.test)', () => {
		// Guards the testing setup as much as the code: if env plumbing breaks,
		// every other assertion below would silently hit localhost.
		expect(API_BASE_URL).toBe('http://api.test');
	});
});

describe('apiFetch', () => {
	it('sends credentials and JSON headers, and returns parsed JSON', async () => {
		fetchMock.mockResolvedValue(jsonResponse({ hello: 'world' }));

		const result = await apiFetch<{ hello: string }>('/api/thing');

		expect(result).toEqual({ hello: 'world' });
		expect(fetchMock).toHaveBeenCalledTimes(1);
		const [url, init] = fetchMock.mock.calls[0];
		expect(url).toBe('http://api.test/api/thing');
		expect(init.credentials).toBe('include');
		expect(init.headers).toEqual({ 'Content-Type': 'application/json' });
	});

	it('merges caller headers', async () => {
		fetchMock.mockResolvedValue(jsonResponse({}));

		await apiFetch('/api/thing', { method: 'PUT', headers: { 'X-Trace': 'abc' } });

		const [, init] = fetchMock.mock.calls[0];
		expect(init.method).toBe('PUT');
		expect(init.headers).toEqual({ 'Content-Type': 'application/json', 'X-Trace': 'abc' });
	});

	it('throws ApiError with status for non-2xx responses', async () => {
		fetchMock.mockResolvedValue(jsonResponse({ error: 'boom' }, { status: 500 }));

		const promise = apiFetch('/api/thing');
		await expect(promise).rejects.toBeInstanceOf(ApiError);
		await expect(promise).rejects.toMatchObject({ message: 'API error: 500', status: 500 });
	});

	it('maps 401 to a "Session expired" ApiError', async () => {
		fetchMock.mockResolvedValue(jsonResponse({}, { status: 401 }));

		await expect(apiFetch('/api/thing')).rejects.toMatchObject({
			name: 'ApiError',
			message: 'Session expired',
			status: 401
		});
	});
});

describe('endpoint helpers', () => {
	it('fetchMe requests /api/me', async () => {
		fetchMock.mockResolvedValue(jsonResponse({ athlete_id: 1 }));

		const user = await fetchMe();

		expect(user).toEqual({ athlete_id: 1 });
		expect(fetchMock.mock.calls[0][0]).toBe('http://api.test/api/me');
	});

	it('fetchPreserveStats passes show_unvisited through the query string', async () => {
		// Fresh Response per call: a body can only be consumed once.
		fetchMock.mockImplementation(() => Promise.resolve(jsonResponse({ preserves: [] })));

		await fetchPreserveStats(false);
		expect(fetchMock.mock.calls[0][0]).toBe(
			'http://api.test/api/stats/preserves?show_unvisited=false'
		);

		await fetchPreserveStats(true);
		expect(fetchMock.mock.calls[1][0]).toBe(
			'http://api.test/api/stats/preserves?show_unvisited=true'
		);
	});

	it('fetchActivities URL-encodes the preserve and page', async () => {
		fetchMock.mockResolvedValue(jsonResponse({ activities: [] }));

		await fetchActivities('El Corte de Madera Creek', 3);

		expect(fetchMock.mock.calls[0][0]).toBe(
			'http://api.test/api/activities?preserve=El%20Corte%20de%20Madera%20Creek&page=3'
		);
	});

	it('fetchHealth requests /health', async () => {
		fetchMock.mockResolvedValue(jsonResponse({ status: 'ok', build_id: 'xyz' }));

		expect(await fetchHealth()).toEqual({ status: 'ok', build_id: 'xyz' });
		expect(fetchMock.mock.calls[0][0]).toBe('http://api.test/health');
	});

	it('deleteAccount issues a DELETE to /api/account', async () => {
		fetchMock.mockResolvedValue(jsonResponse({ status: 'deleted' }));

		await deleteAccount();

		const [url, init] = fetchMock.mock.calls[0];
		expect(url).toBe('http://api.test/api/account');
		expect(init.method).toBe('DELETE');
	});

	it('logout POSTs to /auth/logout with credentials and does not parse a body', async () => {
		fetchMock.mockResolvedValue(new Response(null, { status: 204 }));

		await expect(logout()).resolves.toBeUndefined();

		const [url, init] = fetchMock.mock.calls[0];
		expect(url).toBe('http://api.test/auth/logout');
		expect(init).toMatchObject({ method: 'POST', credentials: 'include' });
	});
});
