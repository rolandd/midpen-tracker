<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright 2026 Roland Dreier <roland@kernel.org> -->

<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { fetchMe, clearToken } from '$lib/api';

	onMount(() => {
		// Poll every 2 seconds
		const interval = setInterval(async () => {
			try {
				const user = await fetchMe();
				if (!user.deletion_requested_at) {
					// Deletion aborted/cleared -> back to dashboard
					goto('/dashboard');
				}
				// Still present and marked for deletion -> wait
			} catch {
				// Failed to fetch (401/404) -> deletion complete
				clearInterval(interval);
				clearToken();
				goto('/');
			}
		}, 2000);

		return () => clearInterval(interval);
	});
</script>

<div class="container">
	<div class="content">
		<div class="spinner"></div>
		<h1>Deleting Account</h1>
		<p>We are securely removing your data.</p>
		<p class="sub">You will be redirected automatically when finished.</p>
	</div>
</div>

<style>
	.container {
		display: flex;
		align-items: center;
		justify-content: center;
		min-height: 60vh;
		text-align: center;
	}

	.content {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 1.5rem;
	}

	h1 {
		font-size: 1.5rem;
		font-weight: 600;
		color: var(--color-text);
		margin: 0;
	}

	p {
		color: var(--color-text-muted);
		margin: 0;
	}

	.sub {
		font-size: 0.875rem;
		opacity: 0.8;
	}

	.spinner {
		width: 48px;
		height: 48px;
		border: 4px solid var(--color-border);
		border-top-color: var(--color-danger, #ef4444);
		border-radius: 50%;
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
