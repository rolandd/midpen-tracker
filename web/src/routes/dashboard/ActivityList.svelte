<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright 2026 Roland Dreier <roland@rolandd.dev> -->

<script lang="ts">
	import { fetchActivities, type ActivitySummary } from '$lib/api';
	import { Spinner, Button, EmptyState } from '$lib/components';
	import { activityCache } from '$lib/cache.svelte';

	let { preserveName } = $props();

	let activities = $state<ActivitySummary[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let page = $state(1);
	let total = $state(0);

	$effect(() => {
		const cached = activityCache.get(preserveName);
		if (cached) {
			activities = cached.activities;
			total = cached.total;
			page = cached.page;
			loading = false;
			error = null;
		} else {
			// Reset state
			activities = [];
			page = 1;
			total = 0;
			loadActivities(1);
		}
	});

	async function loadActivities(pageNum: number) {
		loading = true;
		error = null;

		try {
			const data = await fetchActivities(preserveName, pageNum);
			if (pageNum === 1) {
				activities = data.activities;
				activityCache.set(preserveName, data.activities, data.total, 1);
			} else {
				activities = [...activities, ...data.activities];
				activityCache.append(preserveName, data.activities, data.total, pageNum);
			}
			total = data.total;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load activities';
		} finally {
			loading = false;
		}
	}

	function handleLoadMore() {
		page++;
		loadActivities(page);
	}

	function getEmoji(sportType: string): string {
		const type = sportType.toLowerCase();
		if (type.includes('ride')) return '🚴';
		if (type.includes('run') || type.includes('run')) return '🏃'; // Handle TrailRun etc
		if (type.includes('hike') || type.includes('walk')) return '🚶';
		return '❓'; // Unknown
	}

	function formatDate(iso: string): string {
		return new Date(iso).toLocaleDateString(undefined, {
			year: 'numeric',
			month: 'short',
			day: 'numeric'
		});
	}

	function getAriaLabel(activity: ActivitySummary): string {
		return `${activity.sport_type} on ${formatDate(activity.start_date)}: ${activity.name}, opens in new tab`;
	}
</script>

<div class="activity-list-container">
	{#if activities.length === 0 && loading}
		<div class="loading">
			<Spinner size="sm" />
		</div>
	{:else if error}
		<div class="error">{error}</div>
	{:else if activities.length === 0}
		<EmptyState
			title="No activities found"
			description="We couldn't find any activities for this preserve."
			variant="compact"
		/>
	{:else}
		<ul class="activity-list">
			{#each activities as activity (activity.id)}
				<li>
					<a
						href="https://www.strava.com/activities/{activity.id}"
						target="_blank"
						rel="noopener"
						class="activity"
						onclick={(e) => e.stopPropagation()}
						aria-label={getAriaLabel(activity)}
					>
						<span class="emoji" title={activity.sport_type}>{getEmoji(activity.sport_type)}</span>
						<span class="date">{formatDate(activity.start_date)}</span>
						<span class="name">{activity.name}</span>
						<span class="link">↗</span>
					</a>
				</li>
			{/each}
		</ul>

		{#if activities.length < total}
			<div class="load-more-container">
				<Button variant="secondary" size="sm" onclick={handleLoadMore} isLoading={loading}>
					Load more
				</Button>
			</div>
		{/if}
	{/if}
</div>

<style>
	.activity-list-container {
		margin-top: 1rem;
		border-top: 1px solid var(--color-border);
		padding-top: 1rem;
	}

	.activity-list {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		list-style: none;
		padding: 0;
		margin: 0;
	}

	.activity {
		display: grid;
		grid-template-columns: auto auto 1fr auto;
		gap: 0.75rem;
		padding: 0.5rem;
		border-radius: var(--radius-sm);
		color: var(--color-text);
		text-decoration: none;
		font-size: 0.875rem;
		align-items: center;
		transition: background-color 0.2s;
	}

	.activity:hover {
		background: var(--color-bg);
	}

	.emoji {
		font-size: 1.1rem;
		line-height: 1;
	}

	.date {
		color: var(--color-text-muted);
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}

	.name {
		font-weight: 500;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.link {
		color: var(--color-text-muted);
		opacity: 0.5;
	}

	.activity:hover .link {
		opacity: 1;
	}

	.load-more-container {
		display: flex;
		justify-content: center;
		padding-top: 1rem;
	}

	.loading {
		padding: 1rem;
		display: flex;
		justify-content: center;
	}

	.error {
		color: var(--color-danger);
		padding: 0.5rem;
		font-size: 0.875rem;
	}
</style>
