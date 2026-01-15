<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { isLoggedIn, fetchPreserveStats, logout as apiLogout, type PreserveSummary } from '$lib/api';
	
	let loading = $state(true);
	let error = $state<string | null>(null);
	let preserves = $state<PreserveSummary[]>([]);
	let totalVisited = $state(0);
	let totalPreserves = $state(0);
	let pendingActivities = $state(0);
	let showUnvisited = $state(false);
	let expandedPreserve = $state<string | null>(null);
	
	onMount(async () => {
		if (!isLoggedIn()) {
			await goto('/');
			return;
		}
		
		loadStats();
		
		// Auto-refresh while backfill is in progress
		const interval = setInterval(() => {
			if (pendingActivities > 0) {
				loadStats();
			}
		}, 10000); // Refresh every 10 seconds
		
		return () => clearInterval(interval);
	});
	
	async function loadStats() {
		loading = preserves.length === 0; // Only show spinner on initial load
		error = null;
		
		try {
			const data = await fetchPreserveStats(showUnvisited);
			preserves = data.preserves.sort((a, b) => b.count - a.count);
			totalVisited = data.total_preserves_visited;
			totalPreserves = data.total_preserves;
			pendingActivities = data.pending_activities;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load data';
		} finally {
			loading = false;
		}
	}
	
	function togglePreserve(name: string) {
		expandedPreserve = expandedPreserve === name ? null : name;
	}
	
	async function toggleShowUnvisited() {
		// showUnvisited is updated by bind:checked
		await loadStats();
	}
	
	async function handleLogout() {
		await apiLogout();
		await goto('/');
	}
</script>

<div class="dashboard">
	<header>
		<div class="header-content">
			<h1>🌲 Midpen Tracker</h1>
			<button class="btn btn-secondary" onclick={handleLogout}>Log out</button>
		</div>
	</header>
	
	<main>
		<div class="stats-header">
			<div class="progress-card card">
				<div class="progress-text">
					<span class="progress-count">{totalVisited}</span>
					<span class="progress-total">/ {totalPreserves}</span>
				</div>
				<p class="progress-label">Preserves Visited</p>
				<div class="progress-bar">
					<div class="progress-fill" style="width: {totalPreserves > 0 ? (totalVisited / totalPreserves) * 100 : 0}%"></div>
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
			<label class="toggle">
				<input type="checkbox" bind:checked={showUnvisited} onchange={toggleShowUnvisited} />
				<span>Show unvisited preserves</span>
			</label>
		</div>
		
		{#if loading}
			<div class="loading">
				<div class="spinner"></div>
			</div>
		{:else if error}
			<div class="card error">{error}</div>
		{:else}
			<div class="preserve-list">
				{#each preserves as preserve (preserve.name)}
					<div 
						class="preserve-card card" 
						class:unvisited={preserve.count === 0}
						role="button"
						tabindex="0"
						onclick={() => togglePreserve(preserve.name)}
						onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); togglePreserve(preserve.name); } }}
					>
						<div class="preserve-header">
							<span class="preserve-name">{preserve.name}</span>
							<span class="preserve-count">{preserve.count}</span>
						</div>
						
						{#if expandedPreserve === preserve.name && preserve.activities.length > 0}
							<div class="activity-list">
								{#each preserve.activities as activity (activity.id)}
									<a 
										href="https://www.strava.com/activities/{activity.id}" 
										target="_blank"
										rel="noopener"
										class="activity"
										onclick={(e) => e.stopPropagation()}
									>
										<span class="activity-date">{activity.date}</span>
										<span class="activity-type">{activity.sport_type}</span>
										<span class="activity-name">{activity.name}</span>
										<span class="activity-link">↗</span>
									</a>
								{/each}
							</div>
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
		0% { box-shadow: 0 0 0 0 rgba(var(--color-primary-rgb), 0.1); }
		70% { box-shadow: 0 0 0 10px rgba(var(--color-primary-rgb), 0); }
		100% { box-shadow: 0 0 0 0 rgba(var(--color-primary-rgb), 0); }
	}
	
	.controls {
		margin-bottom: 1rem;
	}
	
	.toggle {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		cursor: pointer;
		color: var(--color-text-muted);
		font-size: 0.875rem;
	}
	
	.toggle input {
		accent-color: var(--color-primary);
	}
	
	.preserve-list {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}
	
	.preserve-card {
		cursor: pointer;
		transition: all 0.2s;
	}
	
	.preserve-card:hover {
		background: var(--color-surface-hover);
	}
	
	.preserve-card.unvisited {
		opacity: 0.5;
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
	
	.activity-list {
		margin-top: 1rem;
		border-top: 1px solid var(--color-border);
		padding-top: 1rem;
	}
	
	.activity {
		display: grid;
		grid-template-columns: auto auto 1fr auto;
		gap: 1rem;
		padding: 0.5rem;
		border-radius: var(--radius-sm);
		color: var(--color-text);
		text-decoration: none;
		font-size: 0.875rem;
	}
	
	.activity:hover {
		background: var(--color-bg);
	}
	
	.activity-date {
		color: var(--color-text-muted);
	}
	
	.activity-type {
		color: var(--color-primary);
		font-weight: 500;
	}
	
	.activity-link {
		color: var(--color-text-muted);
	}
	
	.loading {
		text-align: center;
		padding: 3rem;
	}
	
	.spinner {
		width: 32px;
		height: 32px;
		border: 3px solid var(--color-border);
		border-top-color: var(--color-primary);
		border-radius: 50%;
		animation: spin 1s linear infinite;
		margin: 0 auto;
	}
	
	@keyframes spin {
		to { transform: rotate(360deg); }
	}
	
	.error {
		color: #ef4444;
		text-align: center;
	}
</style>
