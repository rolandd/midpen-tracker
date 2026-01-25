<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright 2026 Roland Dreier <roland@kernel.org> -->

<script lang="ts">
	import { fetchActivities, type ActivitySummary } from '$lib/api';

	let { preserveName } = $props();

	let activities = $state<ActivitySummary[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let page = $state(1);
	let total = $state(0);

	$effect(() => {
		// Reset state
		activities = [];
		page = 1;
		total = 0;
		loadActivities(1);
	});

	async function loadActivities(pageNum: number) {
		loading = true;
		error = null;

		try {
			const data = await fetchActivities(preserveName, pageNum);
			if (pageNum === 1) {
				activities = data.activities;
			} else {
				activities = [...activities, ...data.activities];
			}
			total = data.total;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load activities';
		} finally {
			loading = false;
		}
	}

	function handleLoadMore(event: MouseEvent) {
		event.stopPropagation();
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
</script>

<div class="activity-list-container">
	{#if activities.length === 0 && loading}
		<div class="loading">
			<div class="spinner"></div>
		</div>
	{:else if error}
		<div class="error">{error}</div>
	{:else if activities.length === 0}
		<div class="empty">No activities specific to this preserve found.</div>
	{:else}
		<div class="activity-list">
			{#each activities as activity (activity.id)}
				<a
					href="https://www.strava.com/activities/{activity.id}"
					target="_blank"
					rel="noopener"
					class="activity"
					onclick={(e) => e.stopPropagation()}
				>
					<span class="emoji" title={activity.sport_type}>{getEmoji(activity.sport_type)}</span>
					<span class="date">{formatDate(activity.start_date)}</span>
					<span class="name">{activity.name}</span>
					<span class="link">↗</span>
				</a>
			{/each}
		</div>

		{#if activities.length < total}
			<div class="load-more-container">
				{#if loading}
					<div class="spinner"></div>
				{:else}
					<button class="load-more-btn" onclick={handleLoadMore}>Load more</button>
				{/if}
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

	.load-more-btn {
		background: var(--color-surface);
		border: 1px solid var(--color-border);
		color: var(--color-text);
		padding: 0.5rem 1rem;
		border-radius: var(--radius-sm);
		font-size: 0.875rem;
		cursor: pointer;
		transition: all 0.2s;
	}

	.load-more-btn:hover {
		background: var(--color-surface-hover);
		border-color: var(--color-primary);
	}

	.loading {
		padding: 1rem;
		display: flex;
		justify-content: center;
	}

	.spinner {
		width: 20px;
		height: 20px;
		border: 2px solid var(--color-border);
		border-top-color: var(--color-primary);
		border-radius: 50%;
		animation: spin 1s linear infinite;
	}

	.error {
		color: var(--color-danger);
		padding: 0.5rem;
		font-size: 0.875rem;
	}

	.empty {
		color: var(--color-text-muted);
		font-style: italic;
		padding: 0.5rem;
		font-size: 0.875rem;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
