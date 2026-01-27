<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright 2026 Roland Dreier <roland@rolandd.dev> -->

<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { setToken } from '$lib/api';

	let error = $state<string | null>(null);

	onMount(async () => {
		const token = $page.url.searchParams.get('token');
		const errorParam = $page.url.searchParams.get('error');

		if (errorParam) {
			error = errorParam;
			return;
		}

		if (token) {
			setToken(token);
			await goto('/dashboard');
		} else {
			error = 'No token received';
		}
	});
</script>

<div class="callback">
	{#if error}
		<div class="card error-card">
			<h2>Authentication Failed</h2>
			<p>{error}</p>
			<button class="btn btn-primary" onclick={async () => await goto('/')}>Back to Home</button>
		</div>
	{:else}
		<div class="loading">
			<div class="spinner"></div>
			<p>Connecting your account...</p>
		</div>
	{/if}
</div>

<style>
	.callback {
		min-height: 100vh;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 2rem;
	}

	.loading {
		text-align: center;
	}

	.spinner {
		width: 48px;
		height: 48px;
		border: 3px solid var(--color-border);
		border-top-color: var(--color-primary);
		border-radius: 50%;
		animation: spin 1s linear infinite;
		margin: 0 auto 1rem;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.error-card {
		text-align: center;
		max-width: 400px;
	}

	.error-card h2 {
		margin-bottom: 1rem;
		color: #ef4444;
	}

	.error-card p {
		margin-bottom: 1.5rem;
		color: var(--color-text-muted);
	}
</style>
