<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright 2026 Roland Dreier <roland@rolandd.dev> -->

<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import {
		checkAuth,
		fetchPreserveStats,
		logout as apiLogout,
		fetchMe,
		type PreserveSummary,
		type PreserveStatsResponse,
		type UserResponse
	} from '$lib/api';
	import ActivityList from './ActivityList.svelte';
	import ProfileDropdown from '$lib/components/ProfileDropdown.svelte';
	import { Spinner } from '$lib/components';
	import { uiState } from '$lib/state.svelte';
	import Toggle from 'svelte-switcher';

	let loading = $state(true);
	let error = $state<string | null>(null);
	let user = $state<UserResponse | null>(null);
	let allTimePreserves = $state<PreserveSummary[]>([]);
	let preservesByYear = $state<PreserveStatsResponse['preserves_by_year']>({});
	let availableYears = $state<string[]>([]);
	let selectedYear = $state<string | null>(null); // null = "All Time"
	let totalPreserves = $state(0);
	let pendingActivities = $state(0);
	let showUnvisited = $state(false);
	let expandedPreserve = $state<string | null>(null);
	let isLoggingOut = $state(false);

	// Computed: get preserves filtered by selected year
	let preserves = $derived.by(() => {
		if (!selectedYear) {
			// All time
			return showUnvisited ? allTimePreserves : allTimePreserves.filter((p) => p.count > 0);
		}
		// Filter to selected year
		const yearData = preservesByYear[selectedYear] ?? {};
		const yearPreserves: PreserveSummary[] = Object.entries(yearData)
			.filter(([, count]) => count !== undefined)
			.map(([name, count]) => ({ name, count: count ?? 0, activities: [] }))
			.sort((a, b) => b.count - a.count || a.name.localeCompare(b.name));

		if (showUnvisited) {
			// Add unvisited preserves with count 0
			const visitedNames = new Set(yearPreserves.map((p) => p.name));
			const allNames = allTimePreserves.map((p) => p.name);
			for (const name of allNames) {
				if (!visitedNames.has(name)) {
					yearPreserves.push({ name, count: 0, activities: [] });
				}
			}
			yearPreserves.sort((a, b) => b.count - a.count || a.name.localeCompare(b.name));
		}
		return yearPreserves;
	});

	let totalVisited = $derived(preserves.filter((p) => p.count > 0).length);

	onMount(() => {
		(async () => {
			if (!(await checkAuth())) {
				goto('/');
				return;
			}
		})();

		loadStats();
		fetchUser();

		// Auto-refresh while backfill is in progress
		const interval = setInterval(() => {
			if (pendingActivities > 0) {
				loadStats();
			}
		}, 10000);

		return () => clearInterval(interval);
	});

	async function loadStats() {
		loading = allTimePreserves.length === 0;
		error = null;

		try {
			// Always fetch all preserves (visited and unvisited)
			const data = await fetchPreserveStats(true);
			allTimePreserves = data.preserves.sort((a, b) => b.count - a.count);
			preservesByYear = data.preserves_by_year;
			availableYears = data.available_years;
			totalPreserves = data.total_preserves;
			pendingActivities = data.pending_activities;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load data';
		} finally {
			loading = false;
		}
	}

	async function fetchUser() {
		try {
			user = await fetchMe();
			if (user.deletion_requested_at) {
				goto('/account-deletion-in-progress');
			}
		} catch (e) {
			console.error('Failed to fetch user profile', e);

			// If we get a 404, the user record is gone (e.g. revoked/deleted),
			// but we still have a valid JWT. We should log them out locally.
			const msg = e instanceof Error ? e.message : String(e);
			if (msg.includes('404')) {
				await apiLogout(); // This clears token and hits logout endpoint
				goto('/');
			}
		}
	}

	function togglePreserve(name: string) {
		expandedPreserve = expandedPreserve === name ? null : name;
	}

	async function handleLogout() {
		isLoggingOut = true;
		await apiLogout();
		await goto('/');
	}
</script>

<div class="dashboard">
	<header>
		<div class="header-content">
			<div class="header-left">
				<h1>🌲 Midpen Tracker</h1>
			</div>
			<div class="header-right">
				<button
					class="about-trigger"
					onclick={() => (uiState.isAboutOpen = true)}
					aria-label="About Midpen Tracker"
				>
					<span class="icon" aria-hidden="true">ⓘ</span>
					<span>About</span>
				</button>
				{#if user}
					<ProfileDropdown {user} onLogout={handleLogout} {isLoggingOut} />
				{:else}
					<!-- Fallback/Skeleton while loading user -->
					<div class="header-placeholder"></div>
				{/if}
			</div>
		</div>
	</header>

	<main>
		<div class="stats-header">
			<div class="progress-card card">
				<div class="progress-text">
					<span class="progress-count">{totalVisited}</span>
					<span class="progress-total">/ {totalPreserves}</span>
				</div>
				<p class="progress-label">Preserves Visited{selectedYear ? ` in ${selectedYear}` : ''}</p>
				<div
					class="progress-bar"
					role="progressbar"
					aria-valuenow={totalVisited}
					aria-valuemin={0}
					aria-valuemax={totalPreserves}
					aria-label="{totalVisited} of {totalPreserves} preserves visited{selectedYear
						? ` in ${selectedYear}`
						: ''}"
				>
					<div
						class="progress-fill"
						style="width: {totalPreserves > 0 ? (totalVisited / totalPreserves) * 100 : 0}%"
					></div>
				</div>
			</div>
		</div>

		{#if pendingActivities > 0}
			<div class="processing-banner card">
				<div class="processing-icon">⏳</div>
				<div class="processing-text">
					<strong>Processing {pendingActivities} activities</strong>
					<span>Your data will update automatically</span>
				</div>
			</div>
		{/if}

		<div class="controls">
			{#if availableYears.length > 0}
				<div class="year-filter" role="group" aria-label="Filter preserves by year">
					<button
						class="year-pill"
						class:active={selectedYear === null}
						onclick={() => (selectedYear = null)}
						aria-pressed={selectedYear === null}
					>
						All Time
					</button>
					{#each availableYears as year (year)}
						<button
							class="year-pill"
							class:active={selectedYear === year}
							onclick={() => (selectedYear = year)}
							aria-pressed={selectedYear === year}
						>
							{year}
						</button>
					{/each}
				</div>
			{/if}
			<div class="toggle-wrapper">
				<Toggle bind:checked={showUnvisited} />
				<button class="toggle-label" onclick={() => (showUnvisited = !showUnvisited)}>
					Show unvisited preserves
				</button>
			</div>
		</div>

		{#if loading}
			<div class="loading">
				<Spinner size="md" />
			</div>
		{:else if error}
			<div class="card error">{error}</div>
		{:else}
			<div class="preserve-list">
				{#each preserves as preserve (preserve.name)}
					<div class="preserve-card card" class:unvisited={preserve.count === 0}>
						<button
							type="button"
							class="preserve-header-btn"
							aria-expanded={expandedPreserve === preserve.name}
							onclick={() => togglePreserve(preserve.name)}
						>
							<div class="preserve-header">
								<span class="preserve-name">{preserve.name}</span>
								<span class="preserve-count">{preserve.count}</span>
							</div>
						</button>

						{#if expandedPreserve === preserve.name}
							<ActivityList preserveName={preserve.name} />
						{/if}
					</div>
				{/each}
			</div>
		{/if}
	</main>
</div>

<style>
	.dashboard {
		min-height: 100vh;
	}

	header {
		background: var(--color-surface);
		border-bottom: 1px solid var(--color-border);
		padding: 1rem 1.5rem;
		position: sticky;
		top: 0;
		z-index: 10;
	}

	.header-content {
		max-width: 800px;
		margin: 0 auto;
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.header-left {
		display: flex;
		align-items: center;
	}

	.header-right {
		display: flex;
		align-items: center;
		gap: 1rem;
	}

	.about-trigger {
		background: none;
		border: 1px solid var(--color-border);
		color: var(--color-text-muted);
		cursor: pointer;
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.35rem 0.75rem;
		border-radius: var(--radius-sm);
		font-size: 0.875rem;
		transition: all 0.2s;
	}

	.about-trigger:hover {
		background: var(--color-surface-hover);
		color: var(--color-text);
		border-color: var(--color-primary);
	}

	.about-trigger .icon {
		font-style: normal;
	}

	h1 {
		font-size: 1.25rem;
		font-weight: 600;
	}

	main {
		max-width: 800px;
		margin: 0 auto;
		padding: 1.5rem;
	}

	.progress-card {
		text-align: center;
		margin-bottom: 1.5rem;
	}

	.progress-text {
		margin-bottom: 0.25rem;
	}

	.progress-count {
		font-size: 3rem;
		font-weight: 700;
		color: var(--color-primary);
	}

	.progress-total {
		font-size: 1.5rem;
		color: var(--color-text-muted);
	}

	.progress-label {
		color: var(--color-text-muted);
		margin-bottom: 1rem;
	}

	.progress-bar {
		height: 8px;
		background: var(--color-border);
		border-radius: 4px;
		overflow: hidden;
	}

	.progress-fill {
		height: 100%;
		background: linear-gradient(90deg, var(--color-primary), var(--color-primary-hover));
		transition: width 0.5s ease;
	}

	.processing-banner {
		display: flex;
		align-items: center;
		gap: 1rem;
		padding: 1rem;
		margin-bottom: 2rem;
		background: var(--color-surface);
		border-left: 4px solid var(--color-primary);
		animation: pulse 2s infinite;
	}

	.processing-icon {
		font-size: 1.5rem;
		animation: spin 3s linear infinite;
	}

	.processing-text {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}

	.processing-text strong {
		color: var(--color-text);
		font-weight: 600;
	}

	.processing-text span {
		color: var(--color-text-muted);
		font-size: 0.875rem;
	}

	@keyframes pulse {
		0% {
			box-shadow: 0 0 0 0 rgba(var(--color-primary-rgb), 0.1);
		}
		70% {
			box-shadow: 0 0 0 10px rgba(var(--color-primary-rgb), 0);
		}
		100% {
			box-shadow: 0 0 0 0 rgba(var(--color-primary-rgb), 0);
		}
	}

	.controls {
		display: flex;
		flex-direction: column;
		gap: 1rem;
		margin-bottom: 1.5rem;
	}

	.year-filter {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
	}

	.year-pill {
		padding: 0.5rem 1rem;
		border-radius: 999px;
		border: 1px solid var(--color-border);
		background: var(--color-surface);
		color: var(--color-text-muted);
		font-size: 0.875rem;
		font-weight: 500;
		cursor: pointer;
		transition: all 0.2s ease;
	}

	.year-pill:hover {
		border-color: var(--color-primary);
		color: var(--color-text);
	}

	.year-pill.active {
		background: var(--color-primary);
		border-color: var(--color-primary);
		color: white;
	}

	.toggle-wrapper {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		margin-top: 0.5rem;
	}

	.toggle-label {
		background: none;
		border: none;
		padding: 0;
		color: var(--color-text-muted);
		font-size: 0.875rem;
		cursor: pointer;
		font-family: inherit;
	}

	.toggle-label:hover {
		color: var(--color-text);
	}

	/* Override svelte-switcher styles - scoped to wrapper */
	.toggle-wrapper :global(.svelte-toggle) {
		font-size: 0; /* Fix vertical alignment issues */
	}

	.toggle-wrapper :global(.svelte-toggle .svelte-toggle--track) {
		background-color: var(--color-border) !important;
		width: 44px !important;
		height: 24px !important;
	}

	.toggle-wrapper
		:global(.svelte-toggle:hover:not(.svelte-toggle--disabled) .svelte-toggle--track) {
		background-color: var(--color-surface-hover) !important;
	}

	.toggle-wrapper :global(.svelte-toggle.svelte-toggle--checked .svelte-toggle--track) {
		background-color: var(--color-primary) !important;
	}

	.toggle-wrapper
		:global(
			.svelte-toggle.svelte-toggle--checked:hover:not(.svelte-toggle--disabled)
				.svelte-toggle--track
		) {
		background-color: var(--color-primary-hover) !important;
	}

	.toggle-wrapper :global(.svelte-toggle .svelte-toggle--thumb) {
		border-color: transparent !important;
		background-color: white !important;
		box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
		width: 20px !important;
		height: 20px !important;
		top: 2px !important;
		left: 2px !important;
	}

	.toggle-wrapper :global(.svelte-toggle.svelte-toggle--checked .svelte-toggle--thumb) {
		left: 22px !important;
		border-color: transparent !important;
	}

	.toggle-wrapper :global(.svelte-toggle--focus .svelte-toggle--thumb) {
		box-shadow:
			0 0 0 2px var(--color-surface),
			0 0 0 4px var(--color-primary) !important;
	}

	.preserve-list {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.preserve-card {
		transition: all 0.2s;
	}

	.preserve-card:hover {
		background: var(--color-surface-hover);
	}

	.preserve-card.unvisited {
		opacity: 0.5;
	}

	.preserve-header-btn {
		width: 100%;
		background: none;
		border: none;
		padding: 0;
		text-align: left;
		cursor: pointer;
		color: inherit;
	}

	.preserve-header-btn:focus-visible {
		outline: 2px solid var(--color-primary);
		outline-offset: 4px;
		border-radius: var(--radius-sm);
	}

	.preserve-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.preserve-name {
		font-weight: 500;
	}

	.preserve-count {
		background: var(--color-bg);
		padding: 0.25rem 0.75rem;
		border-radius: 999px;
		font-size: 0.875rem;
		font-weight: 600;
	}

	/* Activity styles moved to ActivityList.svelte */

	.loading {
		display: flex;
		justify-content: center;
		padding: 3rem;
	}

	.error {
		color: #ef4444;
		text-align: center;
	}
</style>
